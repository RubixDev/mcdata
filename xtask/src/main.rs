use std::{
    env,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{bail, Context, Result};
use heck::ToPascalCase;
use lazy_regex::regex_replace_all;
use once_cell::sync::Lazy;
use serde_json::{json, Map, Value};
use tokio::{
    io::{AsyncBufReadExt, BufReader as TokioBufReader},
    process::Command as TokioCommand,
};

use crate::schema::{BlocksJson, Enum, Feature, FeaturesJson, Property};

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
        codegen:                              Run all codegen subtasks
        codegen block-states:                 Generate the `list.rs` file for block states
            "###
            .trim(),
        ),
        "codegen" => match env::args().nth(2).as_deref() {
            Some("block-states") => codegen_block_states().await?,
            Some(subtask) => {
                eprintln!("unknown codegen subtask '{subtask}', run `cargo xtask --help` to see a list of available tasks");
                std::process::exit(1);
            }
            None => codegen_block_states().await?,
        },
        task => {
            eprintln!(
                "unknown task '{task}', run `cargo xtask --help` to see a list of available tasks"
            );
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn codegen_block_states() -> Result<()> {
    // TODO: use xshell?
    // TODO: separate data extraction and reuse across codegen tasks

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

    // read the versions
    println!("\x1b[1;36m>>> reading `versions.json`\x1b[0m");
    let versions: FeaturesJson =
        serde_json::from_reader(BufReader::new(File::open(root.join("versions.json"))?))?;

    // for each mod:
    let mut logs: Vec<Vec<String>> = vec![];
    let mut outputs: Vec<BlocksJson> = vec![];
    for Feature {
        name: feature,
        mc: _,
        extractor,
    } in &versions
    {
        let mod_dir = mods_dir.join(feature);
        // accept the EULA
        println!("\x1b[1;36m>>> version '{feature}': accepting the EULA\x1b[0m");
        fs::create_dir_all(mod_dir.join("run"))?;
        fs::write(mod_dir.join("run/eula.txt"), "eula=true")?;

        // use the official Mojang mappings
        println!("\x1b[1;36m>>> version '{feature}': switching to Mojang mappings\x1b[0m");
        modify_file(mod_dir.join("build.gradle"), |str| {
            Ok(
                regex_replace_all!(r"(^\s*mappings\s*).*$"m, &str, |_, start| format!(
                    "{start}loom.officialMojangMappings()"
                ))
                .into_owned(),
            )
        })?;

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

        // set world type to void world for faster generation
        println!("\x1b[1;36m>>> version '{feature}': setting void world type\x1b[0m");
        fs::write(
            mod_dir.join("run/server.properties"),
            r#"
generator-settings={"lakes"\:false,"features"\:false,"layers"\:[{"block"\:"minecraft\:air","height"\:1}],"structures"\:{"structures"\:{}}}
level-type=flat
"#,
        )?;

        // copy the appropriate DataExtractor class
        println!("\x1b[1;36m>>> version '{feature}': copying the DataExtractor class\x1b[0m");
        fs::copy(
            root.join(format!("DataExtractor_{extractor}.java")),
            mod_dir.join("src/main/java/com/example/DataExtractor.java"),
        )?;

        // run the extraction
        println!("\x1b[1;36m>>> version '{feature}': running the extraction\x1b[0m");
        if mod_dir.join("run/blocks.json").is_file() {
            println!("\x1b[36m> output file already exists, skipping extraction\x1b[0m");
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
            cmd.wait().await?;
            logs.push(log);
        }

        // save the output
        outputs.push(serde_json::from_reader(BufReader::new(File::open(
            mod_dir.join("run/blocks.json"),
        )?))?);
    }

    println!("\x1b[1;32m>>> Done!\x1b[0m");

    for (log, Feature { name: feature, .. }) in logs.into_iter().zip(&versions) {
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

    // generate `block_state.rs`
    let mut block_state_list_rs = r###"
// IMPORTANT: DO NOT EDIT THIS FILE MANUALLY!
// This file is automatically generated with `cargo xtask codegen`.
// To make any changes, edit the xtask source instead.
"###
    .trim_start()
    .to_owned();
    let mut cargo_features = format!("latest = [\"{}\"]\n", versions.last().unwrap().name);
    for (BlocksJson { blocks, enums }, Feature { name: feature, .. }) in
        outputs.into_iter().zip(&versions)
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
                for (index, prop) in block.properties.into_iter().enumerate() {
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
            for (index, value) in values.into_iter().enumerate() {
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
