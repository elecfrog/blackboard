use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;

#[derive(Debug, Parser)]
#[command(name = "bb-schema")]
#[command(about = "Generate Blackboard schema assets from Rust code.")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Generate {
        #[arg(long)]
        root: Option<PathBuf>,
    },
    Check {
        #[arg(long)]
        root: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Generate { root } => generate(root),
        Command::Check { root } => check(root),
    }
}

fn generate(root: Option<PathBuf>) -> Result<()> {
    let data_root = resolve_data_root(root)?;
    let schema_dir = data_root.join("schemas");
    fs::create_dir_all(&schema_dir)
        .with_context(|| format!("create schema directory {}", schema_dir.display()))?;

    for schema in bb_core::schema::generated_schemas() {
        let path = schema_dir.join(schema.file_name);
        fs::write(&path, schema_text(&schema.value)?)
            .with_context(|| format!("write schema {}", path.display()))?;
        println!("generated {}", path.display());
    }

    Ok(())
}

fn check(root: Option<PathBuf>) -> Result<()> {
    let data_root = resolve_data_root(root)?;
    let schema_dir = data_root.join("schemas");
    let mut stale = Vec::new();

    for schema in bb_core::schema::generated_schemas() {
        let path = schema_dir.join(schema.file_name);
        let expected = schema_text(&schema.value)?;
        let actual =
            fs::read_to_string(&path).with_context(|| format!("read schema {}", path.display()))?;
        if actual != expected {
            stale.push(path);
        }
    }

    if !stale.is_empty() {
        for path in &stale {
            eprintln!("stale schema: {}", path.display());
        }
        bail!(
            "schema files are stale; run `bb-schema generate --root {}`",
            data_root.display()
        );
    }

    println!("schemas up to date in {}", schema_dir.display());
    Ok(())
}

fn schema_text(value: &Value) -> Result<String> {
    let mut text = serde_json::to_string_pretty(value)?;
    text.push('\n');
    Ok(text)
}

fn resolve_data_root(root: Option<PathBuf>) -> Result<PathBuf> {
    match root {
        Some(root) => resolve_explicit_root(&root),
        None => discover_data_root(&std::env::current_dir()?)
            .context("could not find .bb_template, .bb, or blackboard.json from current directory"),
    }
}

fn resolve_explicit_root(root: &Path) -> Result<PathBuf> {
    if root.join("blackboard.json").is_file() {
        return Ok(root.to_path_buf());
    }
    if root.join(".bb_template/blackboard.json").is_file() {
        return Ok(root.join(".bb_template"));
    }
    if root.join(".bb/blackboard.json").is_file() {
        return Ok(root.join(".bb"));
    }
    bail!(
        "`{}` is not a Blackboard data root or repository root",
        root.display()
    )
}

fn discover_data_root(start: &Path) -> Option<PathBuf> {
    for dir in start.ancestors() {
        if dir.join(".bb_template/blackboard.json").is_file() {
            return Some(dir.join(".bb_template"));
        }
        if dir.join(".bb/blackboard.json").is_file() {
            return Some(dir.join(".bb"));
        }
        if dir.join("blackboard.json").is_file() {
            return Some(dir.to_path_buf());
        }
    }
    None
}
