//! Project automation: `cargo xtask <ci|layers|bless>`.

use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Allowed workspace dependencies per crate (ADR-0003). Anything else is an upward edge.
const LAYERS: &[(&str, &[&str])] = &[
    ("labely-protocol", &[]),
    ("labely-core", &[]),
    ("labely-render", &["labely-core"]),
    ("labely-transport", &[]),
    ("labely-testkit", &["labely-core"]),
    (
        "labely-printers",
        &[
            "labely-core",
            "labely-protocol",
            "labely-transport",
            "labely-render",
        ],
    ),
    (
        "labely-mcp",
        &["labely-core", "labely-printers", "labely-render"],
    ),
    (
        "labely-ffi",
        &[
            "labely-core",
            "labely-printers",
            "labely-render",
            "labely-transport",
        ],
    ),
    (
        "labely-cli",
        &[
            "labely-core",
            "labely-printers",
            "labely-render",
            "labely-transport",
            "labely-protocol",
            "labely-mcp",
        ],
    ),
    (
        "labely-studio",
        &[
            "labely-core",
            "labely-printers",
            "labely-render",
            "labely-transport",
            "labely-mcp",
        ],
    ),
];

/// External crates restricted to specific workspace crates.
const RESTRICTED: &[(&str, &[&str])] = &[
    ("btleplug", &["labely-transport"]),
    ("serialport", &["labely-transport"]),
    ("egui", &["labely-studio"]),
    ("eframe", &["labely-studio"]),
    ("winit", &["labely-studio"]),
    ("rfd", &["labely-studio"]),
];

fn cargo(args: &[&str], envs: &[(&str, &str)]) -> Result<()> {
    eprintln!("$ cargo {}", args.join(" "));
    let status = Command::new(env!("CARGO"))
        .args(args)
        .envs(envs.iter().copied())
        .status()
        .context("running cargo")?;
    if !status.success() {
        bail!("cargo {} failed", args.join(" "));
    }
    Ok(())
}

fn layers() -> Result<()> {
    let out = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .output()
        .context("cargo metadata")?;
    let meta: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let allowed: BTreeMap<_, _> = LAYERS.iter().copied().collect();
    let mut errors = Vec::new();
    let empty = Vec::new();
    for pkg in meta["packages"].as_array().unwrap_or(&empty) {
        let name = pkg["name"].as_str().unwrap_or_default();
        if name == "xtask" {
            continue;
        }
        let Some(ok) = allowed.get(name) else {
            errors.push(format!("{name}: not listed in xtask LAYERS"));
            continue;
        };
        for dep in pkg["dependencies"].as_array().unwrap_or(&empty) {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            let is_dev = dep["kind"].as_str() == Some("dev");
            if dep_name.starts_with("labely-") && !is_dev && !ok.contains(&dep_name) {
                errors.push(format!(
                    "{name} -> {dep_name}: violates the layer rules (ADR-0003)"
                ));
            }
            if let Some((_, owners)) = RESTRICTED.iter().find(|(c, _)| *c == dep_name)
                && !owners.contains(&name)
            {
                errors.push(format!("{name} -> {dep_name}: only allowed in {owners:?}"));
            }
        }
    }
    if errors.is_empty() {
        eprintln!("layers: ok");
        Ok(())
    } else {
        bail!("layer violations:\n  {}", errors.join("\n  "))
    }
}

fn ci() -> Result<()> {
    cargo(&["fmt", "--all", "--check"], &[])?;
    layers()?;
    cargo(
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )?;
    cargo(
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        &[],
    )?;
    cargo(&["test", "--workspace", "--all-features"], &[])?;
    Ok(())
}

fn main() -> Result<()> {
    let task = std::env::args().nth(1).unwrap_or_default();
    match task.as_str() {
        "ci" => ci(),
        "layers" => layers(),
        "bless" => cargo(&["test", "-p", "labely-render"], &[("LABELY_BLESS", "1")]),
        _ => {
            eprintln!("usage: cargo xtask <ci|layers|bless>");
            std::process::exit(2)
        }
    }
}
