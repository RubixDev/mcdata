use std::{
    collections::VecDeque,
    env,
    fmt::Write,
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

macro_rules! log {
    (raw, task, $($arg:tt)*) => {{
        let msg = format!($($arg)*);
        let msg_len = msg.chars().count();
        format!("\x1b[1;32m{0}\x1b[0m\n\x1b[1;32m>>> {msg} <<<\x1b[0m\n\x1b[1;32m{0}\x1b[0m", "-".repeat(msg_len + 8))
    }};
    (raw, warn, $($arg:tt)*) => { format_args!("\x1b[1;33m>>>> WARNING: {}\x1b[0m", format_args!($($arg)*)) };
    (raw, step, $($arg:tt)*) => { format_args!("\x1b[1;36m>>>> {}\x1b[0m", format_args!($($arg)*)) };
    (raw, info, $($arg:tt)*) => { format_args!("\x1b[36m>>> {}\x1b[0m", format_args!($($arg)*)) };
    (raw, trace, $($arg:tt)*) => { format_args!("\x1b[34m>> {}\x1b[0m", format_args!($($arg)*)) };
    ($level:ident, $($arg:tt)*) => { println!("{}", log!(raw, $level, $($arg)*)) };
}

#[tokio::main]
async fn main() -> Result<()> {
    match env::args().nth(1).unwrap_or_default().as_str() {
        "--help" | "-h" | "" => println!(
            "{}",
            r###"
Usage: Run with `cargo xtask <task>`, eg. `cargo xtask codegen`.

    Tasks:
        codegen:                              Run all codegen subtasks
        codegen features:                     Generate the features list in Cargo.toml
        codegen combined-mod:                 Generate the `combined` module
        codegen block-states:                 Generate the `list.rs` file for block states
        codegen entities:                     Generate the `list.rs` file for entities
        codegen block-entities:               Generate the `list.rs` file for block entities
            "###
            .trim(),
        ),
        "codegen" => {
            let arg = env::args().nth(2);
            let arg = arg.as_deref();

            // read the versions
            log!(step, "reading `versions.json`");
            let root = WORKSPACE_DIR.join("data-extractor");
            let versions: FeaturesJson =
                serde_json::from_reader(BufReader::new(File::open(root.join("versions.json"))?))?;

            if arg.map_or(true, |arg| arg == "features") {
                codegen_features_list(&versions)?;
            }

            if arg.map_or(true, |arg| arg == "combined-mod") {
                codegen_combined_mod(&versions)?;
            }

            if arg.map_or(true, |arg| {
                ["block-states", "entities", "block-entities"].contains(&arg)
            }) {
                let outputs = codegen_extract(&versions).await?;

                if arg.map_or(true, |arg| arg == "block-states") {
                    codegen_block_states(&versions, &outputs.block_lists)?;
                }

                if arg.map_or(true, |arg| ["entities", "block-entities"].contains(&arg)) {
                    let outputs = codegen_class_analysis(&versions, outputs)?;

                    if arg.map_or(true, |arg| arg == "entities") {
                        codegen_entities(&versions, &outputs.0)?;
                    }

                    if arg.map_or(true, |arg| arg == "block-entities") {
                        codegen_block_entities(&versions, &outputs.1)?;
                    }
                }
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
    be_list_paths: Vec<PathBuf>,
}

fn codegen_features_list(versions: &FeaturesJson) -> Result<()> {
    log!(task, "generating Cargo.toml features list");
    log!(step, "building string");
    log!(trace, "`all` and `latest` features");
    let mut cargo_features = format!(
        r###"
## Enable all Minecraft version features
mc-all = [{all}
]
## Enable lists for the latest supported Minecraft version. Currently {latest}
latest = ["{latest}"]
"###,
        all = versions.iter().fold(String::new(), |mut acc, v| {
            let _ = write!(acc, "\n    \"{}\",", v.name);
            acc
        }),
        latest = versions.last().unwrap().name,
    )
    .trim_start()
    .to_owned();
    for Feature { name, mc, .. } in versions {
        log!(trace, "`{name}` with Minecraft `{mc}`");
        cargo_features += &format!("## Enable lists for Minecraft {name}, extracted from Minecraft {mc}\n\"{name}\" = []\n");
    }

    log!(step, "writing file");
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

fn codegen_combined_mod(versions: &FeaturesJson) -> Result<()> {
    log!(task, "generating `combined` module");
    log!(step, "building string");
    log!(trace, "'latest' feature");
    let mut rs = r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.

//! Combined re-exports of all Minecraft version submodules in the type-specific modules.

/// Re-exports for all items of the latest Minecraft version.
#[cfg(feature = "latest")]
#[rustfmt::skip]
pub mod latest {
    #[cfg(feature = "block-states")]
    pub use crate::block_state::list::latest::*;

    #[cfg(feature = "entities")]
    pub use crate::entity::list::latest::Entity;
    #[cfg(feature = "entities")]
    pub use crate::entity::list::latest::types as entity_types;
    #[cfg(feature = "entities")]
    pub use crate::entity::list::latest::compounds as entity_compounds;

    #[cfg(feature = "block-entities")]
    pub use crate::block_entity::list::latest::BlockEntity;
    #[cfg(feature = "block-entities")]
    pub use crate::block_entity::list::latest::types as block_entity_types;
    #[cfg(feature = "block-entities")]
    pub use crate::block_entity::list::latest::compounds as block_entity_compounds;
}
"###
    .trim_start()
    .to_owned();
    for Feature { name, .. } in versions {
        log!(trace, "version '{name}'");
        let mod_name = name.replace('.', "_").replace('-', "_mc");
        rs += &format!(
            r###"
/// Types for Minecraft {name}.
#[cfg(feature = "{name}")]
#[rustfmt::skip]
pub mod mc{mod_name} {{
    #[cfg(feature = "block-states")]
    pub use crate::block_state::list::mc{mod_name}::*;

    #[cfg(feature = "entities")]
    pub use crate::entity::list::mc{mod_name}::Entity;
    #[cfg(feature = "entities")]
    pub use crate::entity::list::mc{mod_name}::types as entity_types;
    #[cfg(feature = "entities")]
    pub use crate::entity::list::mc{mod_name}::compounds as entity_compounds;

    #[cfg(feature = "block-entities")]
    pub use crate::block_entity::list::mc{mod_name}::BlockEntity;
    #[cfg(feature = "block-entities")]
    pub use crate::block_entity::list::mc{mod_name}::types as block_entity_types;
    #[cfg(feature = "block-entities")]
    pub use crate::block_entity::list::mc{mod_name}::compounds as block_entity_compounds;
}}
"###
        );
    }

    log!(step, "writing file");
    fs::write(WORKSPACE_DIR.join("src/combined.rs"), rs)?;

    Ok(())
}

async fn codegen_extract(versions: &FeaturesJson) -> Result<ExtractOutput> {
    log!(task, "extracting data from Minecraft at runtime");
    let root = WORKSPACE_DIR.join("data-extractor");

    // prepare directories
    log!(step, "preparing the tmp directory");
    let tmp_dir = root.join("tmp");
    let clone_dir = tmp_dir.join("fabricmc.net");
    let mods_dir = tmp_dir.join("mods");
    fs::create_dir_all(&tmp_dir)?;

    // clone the fabricmc.net source
    log!(step, "cloning the fabricmc.net source");
    if clone_dir.is_dir() {
        log!(info, "directory already exists, skipping clone");
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
    log!(step, "editing the vite config");
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
    log!(step, "building the generator lib");
    run_command(
        Command::new("deno")
            .args(["task", "--node-modules-dir=auto", "buildLib"])
            .current_dir(&vite_root),
    )
    .with_context(|| "failed to build generator lib")?;

    // for each mod:
    let mut logs: Vec<Vec<String>> = vec![];
    let mut block_lists: Vec<BlocksJson> = vec![];
    let mut mc_jar_paths: Vec<PathBuf> = vec![];
    let mut entity_list_paths: Vec<PathBuf> = vec![];
    let mut be_list_paths: Vec<PathBuf> = vec![];
    for Feature {
        name: feature,
        mc,
        extractor,
    } in versions
    {
        log!(step, "version '{feature}'");
        let mod_dir = mods_dir.join(feature);

        // generate the mod template if it doesn't already exist
        log!(step, "generating the mod template");
        if mod_dir.is_dir() {
            log!(info, "directory already exists, skipping mod generation");
        } else {
            run_command(
                Command::new("deno")
                    .args(["run", "-A"])
                    .arg(root.join("gen_template_mods.ts"))
                    .arg(feature)
                    .current_dir(&root),
            )
            .with_context(|| "failed to generate template mods")?;
        }

        if mod_dir.join("run/blocks.json").is_file()
            && mod_dir.join("run/entities.json").is_file()
            && mod_dir.join("run/block_entities.json").is_file()
        {
            log!(info, "output files already exist, skipping extraction");
        } else {
            // accept the EULA
            log!(info, "accepting the EULA");
            fs::create_dir_all(mod_dir.join("run"))?;
            fs::write(mod_dir.join("run/eula.txt"), "eula=true")?;

            // remove Fabric API dependency and save gradle user home
            log!(info, "removing dependency on Fabric API");
            modify_file(mod_dir.join("build.gradle"), |str| {
                let str = regex_replace_all!(
                    r#"^([ \t]*)(modImplementation "net\.fabricmc\.fabric-api:.*")"#m,
                    &str,
                    |_, space, dep| format!("{space}// {dep}")
                );
                let str = regex_replace_all!(r#"^(dependencies \{)$"#m, &str, |_, deps| format!(
                    r#"project.file("gradle_user_home").write(project.gradle.gradleUserHomeDir.toString()); {deps}"#
                ));
                Ok(str.into_owned())
            })?;

            // disable all mixins and dependencies, and set the entrypoint
            log!(info, "setting up `fabric.mod.json`");
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
            log!(info, "copying the matching DataExtractor class");
            fs::copy(
                root.join(format!("DataExtractor_{extractor}.java")),
                mod_dir.join("src/main/java/com/example/DataExtractor.java"),
            )?;

            // run the extraction
            log!(info, "running the extraction");
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
        log!(info, "saving the outputs");
        let mut blocks_json: BlocksJson =
            serde_json::from_reader(BufReader::new(File::open(mod_dir.join("run/blocks.json"))?))?;
        blocks_json.blocks.sort_unstable_by_key(|b| b.id.clone());
        blocks_json.enums.sort_unstable_by_key(|e| e.name.clone());
        block_lists.push(blocks_json);
        let gradle_user_home = PathBuf::from(
            fs::read_to_string(mod_dir.join("gradle_user_home"))
                .with_context(|| format!("failed to locate Gradle user home for {mc}"))?,
        );
        mc_jar_paths.push(
            glob::glob(
                gradle_user_home.join(format!("caches/fabric-loom/minecraftMaven/net/minecraft/minecraft-merged/{mc}-loom.mappings.*/*.jar"))
                    .to_str()
                    .with_context(|| format!("failed to locate Minecraft jar for {mc}"))?,
            )
                .with_context(|| format!("failed to locate Minecraft jar for {mc}"))?
                .next()
                .with_context(|| format!("failed to locate Minecraft jar for {mc}"))?
                .with_context(|| format!("failed to locate Minecraft jar for {mc}"))?,
        );
        entity_list_paths.push(mod_dir.join("run/entities.json"));
        be_list_paths.push(mod_dir.join("run/block_entities.json"));
    }

    for (log, Feature { name: feature, .. }) in logs.into_iter().zip(versions) {
        if !log.is_empty() {
            log!(warn, "data-extractor for {feature} logged non-info:");
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
        be_list_paths,
    })
}

fn codegen_block_states(versions: &FeaturesJson, block_lists: &[BlocksJson]) -> Result<()> {
    log!(
        task,
        "generating block-states list at `src/block_state/list.rs`"
    );
    let latest_mod_name = versions
        .last()
        .unwrap()
        .name
        .replace('.', "_")
        .replace('-', "_mc");
    let mut block_state_list_rs = format!(
        r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.

/// Re-exports for all items of the latest Minecraft version.
#[cfg(feature = "latest")]
pub mod latest {{
    pub use super::mc{latest_mod_name}::*;
}}
"###
    )
    .trim_start()
    .to_owned();
    for (
        BlocksJson {
            blocks,
            enums,
            map_colors,
        },
        Feature { name: feature, .. },
    ) in block_lists.iter().zip(versions)
    {
        log!(step, "version '{feature}'");
        let mod_name = feature.replace('.', "_").replace('-', "_mc");
        block_state_list_rs += &format!(
            r###"
/// Accompanying types for block states in Minecraft {feature}.
#[cfg(feature = "{feature}")]
pub mod mc{mod_name} {{
    blocks! {{
        "{feature}";
"###
        );
        log!(info, "blocks");
        for (i, block) in blocks.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {}", i + 1, blocks.len(), block.id)
            );
            let Some(name) = block.id.strip_prefix("minecraft:") else {
                continue;
            };
            block_state_list_rs += "        ";
            if block.experimental {
                block_state_list_rs += "experimental "
            }
            block_state_list_rs += &format!("\"{}\", ", block.id);
            block_state_list_rs += &name.to_pascal_case();
            block_state_list_rs += ", ";
            block_state_list_rs += &block.map_color.to_pascal_case();

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
        println!();
        block_state_list_rs += &format!(
            r###"    }}

    prop_enums! {{
        "{feature}";
"###
        );

        log!(info, "enums");
        for (i, Enum { name, values }) in enums.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {name}", i + 1, enums.len())
            );
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
        println!();

        block_state_list_rs += &format!(
            r###"    }}

    map_colors! {{
        "{feature}";
"###
        );
        for (i, (name, color)) in map_colors.iter().enumerate() {
            if name == "NONE" {
                continue;
            }
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {name}", i + 1, enums.len())
            );
            let r = color >> 16 & 0xff;
            let g = color >> 8 & 0xff;
            let b = color & 0xff;
            block_state_list_rs += &format!(
                "        {} => {} ({r}, {g}, {b}),\n",
                name.to_pascal_case(),
                *color as u32
            );
        }
        println!();

        log!(info, "map colors");

        block_state_list_rs += "    }\n}\n";
    }

    log!(step, "writing file");
    fs::write(
        WORKSPACE_DIR.join("src/block_state/list.rs"),
        block_state_list_rs,
    )?;

    Ok(())
}

fn codegen_class_analysis(
    versions: &FeaturesJson,
    outputs: ExtractOutput,
) -> Result<(Vec<EntitiesJson>, Vec<EntitiesJson>)> {
    log!(task, "extracting data from Minecraft jars");
    let class_parser_dir = WORKSPACE_DIR.join("class-parser");

    log!(step, "building class-parser");
    run_command(
        Command::new(class_parser_dir.join("gradlew"))
            .arg("installDist")
            .current_dir(&class_parser_dir),
    )?;
    #[cfg(windows)]
    let class_parser_bin = class_parser_dir.join("build/install/class-parser/bin/class-parser.bat");
    #[cfg(unix)]
    let class_parser_bin = class_parser_dir.join("build/install/class-parser/bin/class-parser");

    log!(step, "running class-parser on extracted data");
    let mut entity_lists: Vec<EntitiesJson> = vec![];
    let mut be_lists: Vec<EntitiesJson> = vec![];
    for (
        Feature {
            name: feature, mc, ..
        },
        ((jar_path, entity_json_path), be_json_path),
    ) in versions.iter().zip(
        outputs
            .mc_jar_paths
            .into_iter()
            .zip(outputs.entity_list_paths)
            .zip(outputs.be_list_paths),
    ) {
        log!(info, "version '{feature}'");
        run_command(
            Command::new(&class_parser_bin)
                .arg(jar_path)
                .arg(mc)
                .arg(entity_json_path)
                .arg(be_json_path)
                .current_dir(&class_parser_dir),
        )?;
        entity_lists.push(serde_json::from_reader(BufReader::new(File::open(
            class_parser_dir.join(format!("out/{mc}/entities.json")),
        )?))?);
        be_lists.push(serde_json::from_reader(BufReader::new(File::open(
            class_parser_dir.join(format!("out/{mc}/block_entities.json")),
        )?))?);
    }

    Ok((entity_lists, be_lists))
}

fn codegen_entities(versions: &FeaturesJson, entity_lists: &[EntitiesJson]) -> Result<()> {
    log!(task, "generating entities list at `src/entity/list.rs`");
    let latest_mod_name = versions
        .last()
        .unwrap()
        .name
        .replace('.', "_")
        .replace('-', "_mc");
    let mut entity_list_rs = format!(
        r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.

/// Re-exports for all items of the latest Minecraft version.
#[cfg(feature = "latest")]
pub mod latest {{
    pub use super::mc{latest_mod_name}::*;
}}
"###
    )
    .trim_start()
    .to_owned();
    for (
        EntitiesJson {
            entities,
            types,
            compound_types,
        },
        Feature { name: feature, .. },
    ) in entity_lists.iter().zip(versions)
    {
        log!(step, "version '{feature}'");
        let mod_name = feature.replace('.', "_").replace('-', "_mc");
        entity_list_rs += &format!(
            r###"
/// Accompanying types for entities in Minecraft {feature}.
#[cfg(feature = "{feature}")]
pub mod mc{mod_name} {{
    entities! {{
        "{feature}", mc{mod_name};
"###
        );
        log!(info, "entities");
        for (i, entity) in entities.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {}", i + 1, entities.len(), entity.id)
            );
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
        println!();
        entity_list_rs += &format!(
            r###"    }}

    entity_types! {{
        "{feature}";
"###
        );

        fn write_type(
            writer: &mut String,
            name: &str,
            parent: Option<&str>,
            compound: &NbtCompound,
        ) {
            *writer += &format!("        {name}");
            if let Some(parent) = parent {
                *writer += &format!(" > {parent}");
            }
            if let Some(extras) = &compound.unknown_keys {
                *writer += &format!(", with extras as {}", extras.as_rust_type());
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

        log!(info, "class types");
        for (i, EntityType { name, parent, nbt }) in types.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {name}", i + 1, types.len())
            );
            write_type(&mut entity_list_rs, name, parent.as_deref(), nbt);
        }
        println!();

        entity_list_rs += &format!(
            r###"    }}

    entity_compound_types! {{
        "{feature}";
"###
        );

        log!(info, "other compound types");
        for (i, CompoundType { name, compound }) in compound_types.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {name}", i + 1, compound_types.len())
            );
            write_type(&mut entity_list_rs, name, None, compound);
        }
        println!();

        entity_list_rs += "    }\n}\n";
    }
    log!(step, "writing file");
    fs::write(WORKSPACE_DIR.join("src/entity/list.rs"), entity_list_rs)?;

    Ok(())
}

fn codegen_block_entities(versions: &FeaturesJson, entity_lists: &[EntitiesJson]) -> Result<()> {
    log!(
        task,
        "generating block entities list at `src/block_entity/list.rs`"
    );
    let latest_mod_name = versions
        .last()
        .unwrap()
        .name
        .replace('.', "_")
        .replace('-', "_mc");
    let mut block_entity_list_rs = format!(
        r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.

/// Re-exports for all items of the latest Minecraft version.
#[cfg(feature = "latest")]
pub mod latest {{
    pub use super::mc{latest_mod_name}::*;
}}
"###
    )
    .trim_start()
    .to_owned();
    for (
        EntitiesJson {
            entities,
            types,
            compound_types,
        },
        Feature { name: feature, .. },
    ) in entity_lists.iter().zip(versions)
    {
        log!(step, "version '{feature}'");
        let mod_name = feature.replace('.', "_").replace('-', "_mc");
        block_entity_list_rs += &format!(
            r###"
/// Accompanying types for block entities in Minecraft {feature}.
#[cfg(feature = "{feature}")]
pub mod mc{mod_name} {{
    block_entities! {{
        "{feature}", mc{mod_name};
"###
        );
        log!(info, "entities");
        for (i, entity) in entities.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {}", i + 1, entities.len(), entity.id)
            );
            let Some(name) = entity.id.strip_prefix("minecraft:") else {
                continue;
            };
            block_entity_list_rs += "        ";
            block_entity_list_rs += &format!("\"{}\", ", entity.id);
            block_entity_list_rs += &name.to_pascal_case();
            block_entity_list_rs += ": ";
            block_entity_list_rs += &entity.type_;
            block_entity_list_rs += " ";

            let mut indirection = 0;
            let mut curr = &entity.type_;
            while let Some(parent) = &types.iter().find(|t| &t.name == curr).unwrap().parent {
                indirection += 1;
                curr = parent;
            }
            block_entity_list_rs += &format!("({}BlockEntity)", "> ".repeat(indirection));

            let mut all_fields = vec![];
            let mut search = VecDeque::from([&entity.type_]);
            while let Some(curr) = search.pop_front() {
                let c = types.iter().find(|t| &t.name == curr).unwrap();
                if c.name != "BlockEntity" {
                    all_fields.extend(c.nbt.entries.keys().map(|k| format!("\"{k}\"")))
                }

                // TODO: this should also include `c.nbt.flattened` and `c.nbt.unknown_keys` somehow
                let Some(parent) = &c.parent else { break };
                search.push_back(parent);
            }
            block_entity_list_rs += ", [";
            block_entity_list_rs += &all_fields.join(", ");
            block_entity_list_rs += "]";

            block_entity_list_rs += ";\n";
        }
        println!();
        block_entity_list_rs += &format!(
            r###"    }}

    block_entity_types! {{
        "{feature}";
"###
        );

        fn write_type(
            writer: &mut String,
            name: &str,
            parent: Option<&str>,
            compound: &NbtCompound,
        ) {
            *writer += &format!("        {name}");
            if let Some(parent) = parent {
                *writer += &format!(" > {parent}");
            }
            if let Some(extras) = &compound.unknown_keys {
                *writer += &format!(", with extras as {}", extras.as_rust_type());
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

        log!(info, "class types");
        for (i, EntityType { name, parent, nbt }) in types.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {name}", i + 1, types.len())
            );
            write_type(&mut block_entity_list_rs, name, parent.as_deref(), nbt);
        }
        println!();

        block_entity_list_rs += &format!(
            r###"    }}

    block_entity_compound_types! {{
        "{feature}";
"###
        );

        log!(info, "other compound types");
        for (i, CompoundType { name, compound }) in compound_types.iter().enumerate() {
            print!(
                "{}\x1b[0K\r",
                log!(raw, trace, "{}/{}: {name}", i + 1, compound_types.len())
            );
            write_type(&mut block_entity_list_rs, name, None, compound);
        }
        println!();

        block_entity_list_rs += "    }\n}\n";
    }
    log!(step, "writing file");
    fs::write(
        WORKSPACE_DIR.join("src/block_entity/list.rs"),
        block_entity_list_rs,
    )?;

    Ok(())
}
