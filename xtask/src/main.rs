//! Project automation: `cargo xtask <ci|layers|bless|i18n-check|schema-hash>`.

use std::collections::BTreeMap;
use std::process::Command;

use anyhow::{Context, Result, bail};

/// Allowed workspace dependencies per crate (ADR-0003). Anything else is an upward edge.
const LAYERS: &[(&str, &[&str])] = &[
    ("labely-protocol", &[]),
    ("labely-core", &[]),
    ("labely-i18n", &[]),
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
            "labely-i18n",
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
            "labely-i18n",
        ],
    ),
];

/// External crates restricted to specific workspace crates (dev-dependencies are exempt).
const RESTRICTED: &[(&str, &[&str])] = &[
    ("btleplug", &["labely-transport"]),
    ("serialport", &["labely-transport"]),
    ("egui", &["labely-studio"]),
    ("eframe", &["labely-studio"]),
    ("winit", &["labely-studio"]),
    ("rfd", &["labely-studio"]),
    ("rmcp", &["labely-mcp"]),
    ("fluent-bundle", &["labely-i18n"]),
];

/// Message ids of every `.ftl` file under `dir`, keyed by file name.
fn ftl_keys(dir: &std::path::Path) -> Result<BTreeMap<String, std::collections::BTreeSet<String>>> {
    let mut out = BTreeMap::new();
    for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
        let path = entry?.path();
        if path.extension().is_none_or(|e| e != "ftl") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let keys = text
            .lines()
            .filter(|l| l.starts_with(|c: char| c.is_ascii_lowercase()))
            .filter_map(|l| l.split_once('=').map(|(k, _)| k.trim().to_owned()))
            .collect();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        out.insert(name, keys);
    }
    Ok(out)
}

/// EN and HU must define the same message ids (ADR-0009). Mobile string sets join in
/// Phases 4 and 5.
fn i18n_check() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/i18n/locales");
    let en = ftl_keys(&root.join("en"))?;
    let hu = ftl_keys(&root.join("hu"))?;
    let mut errors = Vec::new();
    for (file, keys) in &en {
        let other = hu.get(file).cloned().unwrap_or_default();
        for k in keys.difference(&other) {
            errors.push(format!("hu/{file}: missing `{k}`"));
        }
        for k in other.difference(keys) {
            errors.push(format!("hu/{file}: `{k}` is not in en"));
        }
    }
    for file in hu.keys().filter(|f| !en.contains_key(*f)) {
        errors.push(format!("hu/{file}: no en counterpart"));
    }
    if errors.is_empty() {
        eprintln!("i18n-check: ok ({} files)", en.len());
        Ok(())
    } else {
        bail!("i18n key mismatch:\n  {}", errors.join("\n  "))
    }
}

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
            if !is_dev
                && let Some((_, owners)) = RESTRICTED.iter().find(|(c, _)| *c == dep_name)
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
    i18n_check()?;
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
        "i18n-check" => i18n_check(),
        // Prints the current schema hash (pinned with API_VERSION in core's schema test).
        "schema-hash" => cargo(
            &[
                "test",
                "-p",
                "labely-core",
                "--test",
                "it",
                "api_version_matches_schema_hash",
                "--",
                "--nocapture",
            ],
            &[],
        ),
        _ => {
            eprintln!("usage: cargo xtask <ci|layers|bless|i18n-check|schema-hash>");
            std::process::exit(2)
        }
    }
}
