use std::{
    env,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use heck::{ToPascalCase, ToSnakeCase};
use lazy_regex::regex_replace_all;
use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader as TokioBufReader},
    process::Command as TokioCommand,
};

use crate::schema::*;

mod schema;

pub static WORKSPACE_DIR: Lazy<PathBuf> = Lazy::new(|| {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
});

#[tokio::main]
async fn main() -> Result<()> {
    match env::args().nth(1).unwrap_or_default().as_str() {
        "--help" | "-h" | "" => println!(
            "{}",
            r###"
Usage: Run with `cargo xtask <task>`, eg. `cargo xtask codegen`.

    Tasks:
        codegen:                              Run all codegen subtasks. The data-extractor and class-parser are always run.
        codegen block-states:                 Generate the `list.rs` file for block states
        codegen entities:                     Generate the `list.rs` file for entities
            "###
            .trim(),
        ),
        "codegen" => {
            let arg = env::args().nth(2);
            let arg = arg.as_ref();

            // read the versions
            println!("\x1b[1;36m>>> reading `versions.json`\x1b[0m");
            let root = WORKSPACE_DIR.join("data-extractor");
            let versions: FeaturesJson =
                serde_json::from_reader(BufReader::new(File::open(root.join("versions.json"))?))?;

            let outputs = codegen_extract(&versions).await?;

            if arg.map_or(true, |arg| arg == "block-states") {
                codegen_block_states(&versions, &outputs.block_lists)?;
            }

            let outputs = codegen_class_analysis(&versions, outputs)?;

            if arg.map_or(true, |arg| arg == "entities") {
                codegen_entities(&versions, &outputs)?;
            }
        }
        task => {
            eprintln!(
                "unknown task '{task}', run `cargo xtask --help` to see a list of available tasks"
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

fn run_command(cmd: &mut Command) -> Result<()> {
    let status = cmd.status()?;
    if !status.success() {
        bail!("command exited with {status}");
    }
    Ok(())
}

fn modify_file(path: impl AsRef<Path>, func: impl FnOnce(String) -> Result<String>) -> Result<()> {
    let content = fs::read_to_string(&path)
        .with_context(|| format!("could not modify file {}", path.as_ref().display()))?;
    let new_content = func(content)?;
    fs::write(&path, new_content)?;
    Ok(())
}

struct ExtractOutput {
    block_lists: Vec<BlocksJson>,
    mc_jar_paths: Vec<PathBuf>,
    entity_list_paths: Vec<PathBuf>,
}

async fn codegen_extract(versions: &FeaturesJson) -> Result<ExtractOutput> {
    let root = WORKSPACE_DIR.join("data-extractor");

    // prepare directories
    println!("\x1b[1;36m>>> preparing the tmp directory\x1b[0m");
    let tmp_dir = root.join("tmp");
    let clone_dir = tmp_dir.join("fabricmc.net");
    let mods_dir = tmp_dir.join("mods");
    fs::create_dir_all(&tmp_dir)?;

    // clone the fabricmc.net source
    println!("\x1b[1;36m>>> cloning the fabricmc.net source\x1b[0m");
    if clone_dir.is_dir() {
        println!("\x1b[36m> directory already exists, skipping the clone step\x1b[0m");
    } else {
        run_command(
            Command::new("git")
                .args(["clone", "https://github.com/FabricMC/fabricmc.net"])
                .arg(&clone_dir),
        )
        .with_context(|| "failed to `git clone`")?;
    }

    // edit the vite config (we only need the generator lib, overwriting the config avoids
    // downloading unnecessary dependencies)
    println!("\x1b[1;36m>>> editing the vite config\x1b[0m");
    let vite_root = clone_dir.join("scripts");
    fs::write(
        vite_root.join("vite.config.js"),
        r"
export default {
  build: {
    sourcemap: false,
    minify: false,
    outDir: './dist',
    emptyOutDir: true,
    lib: {
      entry: './src/lib.ts',
      fileName: 'fabric-template-generator',
      name: 'fabric-template-generator',
      formats: ['es'],
    },
  },
}",
    )?;

    // build the library
    println!("\x1b[1;36m>>> building the generator lib\x1b[0m");
    run_command(
        Command::new("deno")
            .args(["task", "buildLib"])
            .current_dir(&vite_root),
    )
    .with_context(|| "failed to build generator lib")?;

    // generate the mod templates for all versions
    println!("\x1b[1;36m>>> generating the template mods\x1b[0m");
    if mods_dir.is_dir() {
        println!("\x1b[36m> directory already exists, skipping mod generation\x1b[0m");
    } else {
        run_command(
            Command::new("deno")
                .args(["run", "-A"])
                .arg(root.join("gen_template_mods.ts"))
                .current_dir(&root),
        )
        .with_context(|| "failed to generate template mods")?;
    }

    // for each mod:
    let mut logs: Vec<Vec<String>> = vec![];
    let mut block_lists: Vec<BlocksJson> = vec![];
    let mut mc_jar_paths: Vec<PathBuf> = vec![];
    let mut entity_list_paths: Vec<PathBuf> = vec![];
    for Feature {
        name: feature,
        mc,
        extractor,
    } in versions
    {
        let mod_dir = mods_dir.join(feature);
        // accept the EULA
        println!("\x1b[1;36m>>> version '{feature}': accepting the EULA\x1b[0m");
        fs::create_dir_all(mod_dir.join("run"))?;
        fs::write(mod_dir.join("run/eula.txt"), "eula=true")?;

        // disable all mixins and dependencies, and set the entrypoint
        println!("\x1b[1;36m>>> version '{feature}': setting up `fabric.mod.json`\x1b[0m");
        modify_file(mod_dir.join("src/main/resources/fabric.mod.json"), |str| {
            let mut json: Map<String, Value> = serde_json::from_str(&str)?;
            json["mixins"] = json!([]);
            json["depends"] = json!({});
            json["entrypoints"] = json!({ "main": ["com.example.DataExtractor"] });
            Ok(serde_json::to_string_pretty(&json)?)
        })?;
        fs::write(
            mod_dir.join("src/main/resources/data-extractor.mixins.json"),
            "{}",
        )?;

        // copy the appropriate DataExtractor class
        println!("\x1b[1;36m>>> version '{feature}': copying the DataExtractor class\x1b[0m");
        fs::copy(
            root.join(format!("DataExtractor_{extractor}.java")),
            mod_dir.join("src/main/java/com/example/DataExtractor.java"),
        )?;

        // run the extraction
        println!("\x1b[1;36m>>> version '{feature}': running the extraction\x1b[0m");
        if mod_dir.join("run/blocks.json").is_file() && mod_dir.join("run/entities.json").is_file()
        {
            println!("\x1b[36m> output files already exist, skipping extraction\x1b[0m");
        } else {
            let mut log = vec![];
            let mut cmd = TokioCommand::new(mod_dir.join("gradlew"))
                .arg("runServer")
                .current_dir(&mod_dir)
                .stdout(Stdio::piped())
                .spawn()
                .with_context(|| "failed to run mod")?;
            let stdout = cmd.stdout.take().unwrap();
            let mut lines = TokioBufReader::new(stdout).lines();
            while let Some(line) = lines.next_line().await? {
                if line.contains("(data-extractor)") {
                    // if the incoming line is from the DataExtractor, print it in the appropriate color
                    if line.contains("/INFO]") {
                        println!("\x1b[32m{line}\x1b[0m");
                    } else if line.contains("/WARN]") {
                        println!("\x1b[33m{line}\x1b[0m");
                        // save warnings and errors for later
                        log.push(line);
                    } else if line.contains("/ERROR]") || line.contains("/FATAL]") {
                        println!("\x1b[31m{line}\x1b[0m");
                        // save warnings and errors for later
                        log.push(line);
                    } else {
                        println!("\x1b[36m{line}\x1b[0m");
                    }
                } else {
                    // print all other log output without color
                    println!("{line}");
                }
            }
            let status = cmd.wait().await?;
            if !status.success() {
                bail!("command exited with {status}");
            }
            logs.push(log);
        }

        // save the outputs
        block_lists.push(serde_json::from_reader(BufReader::new(File::open(
            mod_dir.join("run/blocks.json"),
        )?))?);
        mc_jar_paths.push(
            glob::glob(
                mod_dir.join(".gradle/loom-cache/minecraftMaven/net/minecraft/minecraft-merged-*/*-loom.mappings.*/*.jar")
                    .to_str()
                    .with_context(|| format!("failed to find Minecraft jar for {mc}"))?,
            )
                .with_context(|| format!("failed to find Minecraft jar for {mc}"))?
                .next()
                .with_context(|| format!("failed to find Minecraft jar for {mc}"))?
                .with_context(|| format!("failed to find Minecraft jar for {mc}"))?,
        );
        entity_list_paths.push(mod_dir.join("run/entities.json"));
    }

    println!("\x1b[1;32m>>> Done!\x1b[0m");

    for (log, Feature { name: feature, .. }) in logs.into_iter().zip(versions) {
        if !log.is_empty() {
            println!("\x1b[1;33m>>> WARNING: data-extractor for {feature} logged non-info:\x1b[0m");
        }
        for line in &log {
            if line.contains("/WARN]") {
                println!("\x1b[33m{line}\x1b[0m");
            } else {
                println!("\x1b[31m{line}\x1b[0m");
            }
        }
    }

    Ok(ExtractOutput {
        block_lists,
        mc_jar_paths,
        entity_list_paths,
    })
}

fn codegen_block_states(versions: &FeaturesJson, block_lists: &[BlocksJson]) -> Result<()> {
    let mut block_state_list_rs = r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.
"###
    .trim_start()
    .to_owned();
    // TODO: the "latest" feature should also provide a module that's either aliasing the latest or
    //  even provides its own definitions which then change over time and are marked as
    //  non-exhaustive
    // TODO: make feature generation in Cargo.toml its own step
    let mut cargo_features = format!("latest = [\"{}\"]\n", versions.last().unwrap().name);
    for (BlocksJson { blocks, enums }, Feature { name: feature, .. }) in
        block_lists.iter().zip(versions)
    {
        cargo_features += &format!("\"{feature}\" = []\n");

        let mod_name = feature.replace('.', "_").replace('-', "_mc");
        block_state_list_rs += &format!(
            r###"
/// Block states and property types for Minecraft {feature}.
#[cfg(feature = "{feature}")]
pub mod mc{mod_name} {{
    blocks! {{
        "{feature}";
"###
        );
        for block in blocks {
            let Some(name) = block.id.strip_prefix("minecraft:") else {
                continue;
            };
            block_state_list_rs += "        ";
            if block.experimental {
                block_state_list_rs += "experimental "
            }
            block_state_list_rs += &format!("\"{}\", ", block.id);
            block_state_list_rs += &name.to_pascal_case();

            if !block.properties.is_empty() {
                block_state_list_rs += " - ";
                let last_index = block.properties.len() - 1;
                for (index, prop) in block.properties.iter().enumerate() {
                    let name = prop.name();
                    let mut rename = "";
                    if name == "type" {
                        block_state_list_rs += "r#";
                        rename = " as \"type\"";
                    }
                    match prop {
                        Property::Bool { name } => {
                            block_state_list_rs += &format!("{name}: bool{rename}")
                        }
                        Property::Int { name, min, max } => {
                            block_state_list_rs +=
                                &format!("{name}: bounded_integer::BoundedU8<{min}, {max}>{rename}")
                        }
                        Property::Enum { name, enum_name } => {
                            block_state_list_rs += &format!("{name}: props::{enum_name}{rename}")
                        }
                    }
                    if index != last_index {
                        block_state_list_rs += ", ";
                    }
                }
            }

            block_state_list_rs += ";\n";
        }
        block_state_list_rs += &format!(
            r###"    }}

    prop_enums! {{
        "{feature}";
"###
        );

        for Enum { name, values } in enums {
            block_state_list_rs += &format!("        {name} => ");
            let last_index = values.len() - 1;
            for (index, value) in values.iter().enumerate() {
                block_state_list_rs += &value.to_pascal_case();
                if index != last_index {
                    block_state_list_rs += ", ";
                }
            }
            block_state_list_rs += ";\n";
        }

        block_state_list_rs += "    }\n}\n";
    }
    fs::write(
        WORKSPACE_DIR.join("src/block_state/list.rs"),
        block_state_list_rs,
    )?;

    modify_file(WORKSPACE_DIR.join("Cargo.toml"), |str| {
        Ok(regex_replace_all!(
            r"^(### FEATURE AUTOGEN START ###)[\s\S]*(### FEATURE AUTOGEN END ###)$"m,
            &str,
            |_, start_comment, end_comment| format!(
                "{start_comment}\n{cargo_features}{end_comment}"
            )
        )
        .into_owned())
    })?;

    Ok(())
}

fn codegen_class_analysis(
    versions: &FeaturesJson,
    outputs: ExtractOutput,
) -> Result<Vec<EntitiesJson>> {
    let class_parser_dir = WORKSPACE_DIR.join("class-parser");

    println!("\x1b[1;36m>>> building class-parser\x1b[0m");
    run_command(
        Command::new(class_parser_dir.join("gradlew"))
            .arg("installDist")
            .current_dir(&class_parser_dir),
    )?;
    let class_parser_bin = class_parser_dir.join("build/install/class-parser/bin/class-parser");

    println!("\x1b[1;36m>>> running class-parser on extracted data\x1b[0m");
    let mut entity_lists: Vec<EntitiesJson> = vec![];
    for (Feature { name: feature, .. }, (jar_path, entity_json_path)) in versions.iter().zip(
        outputs
            .mc_jar_paths
            .into_iter()
            .zip(outputs.entity_list_paths),
    ) {
        println!("\x1b[36m>> version '{feature}'\x1b[0m\n");
        run_command(
            Command::new(&class_parser_bin)
                .arg(jar_path)
                .arg(entity_json_path)
                .current_dir(&class_parser_dir),
        )?;
        entity_lists.push(serde_json::from_reader(BufReader::new(File::open(
            class_parser_dir.join("out/entities.json"),
        )?))?);
    }
    println!("\x1b[1;32m>>> Done!\x1b[0m");

    Ok(entity_lists)
}

fn codegen_entities(versions: &FeaturesJson, entity_lists: &[EntitiesJson]) -> Result<()> {
    let mut entity_list_rs = r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.
"###
    .trim_start()
    .to_owned();
    // TODO: the "latest" feature should also provide a module that's either aliasing the latest or
    //  even provides its own definitions which then change over time and are marked as
    //  non-exhaustive
    for (
        EntitiesJson {
            entities,
            types,
            compound_types,
        },
        Feature { name: feature, .. },
    ) in entity_lists.iter().zip(versions)
    {
        let mod_name = feature.replace('.', "_").replace('-', "_mc");
        entity_list_rs += &format!(
            r###"
/// Block states and property types for Minecraft {feature}.
#[cfg(feature = "{feature}")]
pub mod mc{mod_name} {{
    entities! {{
        "{feature}";
"###
        );
        for entity in entities {
            let Some(name) = entity.id.strip_prefix("minecraft:") else {
                continue;
            };
            entity_list_rs += "        ";
            if entity.experimental {
                entity_list_rs += "experimental "
            }
            entity_list_rs += &format!("\"{}\", ", entity.id);
            entity_list_rs += &name.to_pascal_case();
            entity_list_rs += ": ";
            entity_list_rs += &entity.type_;
            entity_list_rs += ";\n";
        }
        entity_list_rs += &format!(
            r###"    }}

    entity_types! {{
        "{feature}";
"###
        );

        fn write_type(
            writer: &mut String,
            feature: &str,
            name: &str,
            parent: Option<&str>,
            supports_extras: bool,
            compound: &NbtCompound,
        ) {
            *writer += &format!("        {name}");
            if let Some(parent) = parent {
                *writer += &format!(" > {parent}");
            }
            if let Some(extras) = &compound.unknown_keys {
                if supports_extras {
                    *writer += &format!(", with extras as {}", extras.as_rust_type());
                } else {
                    println!("\x1b[1;33m>>> WARNING: version '{feature}': entity type '{name}' specifies unknown keys as '{}'\x1b[0m", extras.as_rust_type());
                }
            }
            if !compound.flattened.is_empty() {
                *writer += ", flattened [";
                let last_index = compound.flattened.len() - 1;
                for (index, value) in compound.flattened.iter().enumerate() {
                    *writer += &format!("flattened_{index}: {}", value.as_rust_type());
                    if index != last_index {
                        *writer += ", ";
                    }
                }
                *writer += "]";
            }
            *writer += " { ";

            let last_index = compound.entries.len().saturating_sub(1);
            for (index, (name, entry)) in compound.entries.iter().enumerate() {
                if entry.optional {
                    *writer += "optional ";
                }
                let mut ident_name = name.to_snake_case();
                if ident_name == "type" {
                    ident_name = "r#type".to_string();
                }
                *writer += &format!("\"{name}\" as {ident_name}: {}", entry.value.as_rust_type());
                if index != last_index {
                    *writer += ", ";
                }
            }
            *writer += " }\n";
        }

        for EntityType { name, parent, nbt } in types {
            write_type(
                &mut entity_list_rs,
                feature,
                name,
                parent.as_deref(),
                false,
                nbt,
            );
        }

        entity_list_rs += &format!(
            r###"    }}

    entity_compound_types! {{
        "{feature}";
"###
        );

        for CompoundType { name, compound } in compound_types {
            write_type(&mut entity_list_rs, feature, name, None, true, compound);
        }

        entity_list_rs += "    }\n}\n";
    }
    fs::write(WORKSPACE_DIR.join("src/entity/list.rs"), entity_list_rs)?;

    Ok(())
}
