//! Project automation:
//! `cargo xtask <ci|layers|bless|i18n-check|schema-hash|bindings|xcframework>`.

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
    ("uniffi", &["labely-ffi"]),
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

/// `name` attributes of every `<string>`/`<plurals>` in the Android `strings*.xml` files of
/// `dir` (a `res/values*` directory), keyed by file name. Empty when `dir` does not exist.
fn android_string_keys(
    dir: &std::path::Path,
) -> Result<BTreeMap<String, std::collections::BTreeSet<String>>> {
    let mut out = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Ok(out);
    };
    for entry in entries {
        let path = entry?.path();
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !name.starts_with("strings") || !name.ends_with(".xml") {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let keys = text
            .split('<')
            .filter(|t| t.starts_with("string ") || t.starts_with("plurals "))
            .filter_map(|t| {
                t.split_once("name=\"")?
                    .1
                    .split_once('"')
                    .map(|(k, _)| k.to_owned())
            })
            .collect();
        out.insert(name, keys);
    }
    Ok(out)
}

/// Differences between two key sets per file, reported against `label` (the second set).
fn key_diff(
    label: &str,
    en: &BTreeMap<String, std::collections::BTreeSet<String>>,
    hu: &BTreeMap<String, std::collections::BTreeSet<String>>,
    errors: &mut Vec<String>,
) {
    for (file, keys) in en {
        let other = hu.get(file).cloned().unwrap_or_default();
        for k in keys.difference(&other) {
            errors.push(format!("{label}/{file}: missing `{k}`"));
        }
        for k in other.difference(keys) {
            errors.push(format!("{label}/{file}: `{k}` is not in en"));
        }
    }
    for file in hu.keys().filter(|f| !en.contains_key(*f)) {
        errors.push(format!("{label}/{file}: no en counterpart"));
    }
}

/// Android string resource directories checked by `i18n-check`.
const ANDROID_RES: &[&str] = &["mobile/android/app/src/main/res"];

/// EN and HU must define the same message ids (ADR-0009): the Fluent files and the Android
/// string resources. iOS joins in Phase 5.
fn i18n_check() -> Result<()> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/i18n/locales");
    let en = ftl_keys(&root.join("en"))?;
    let hu = ftl_keys(&root.join("hu"))?;
    let mut errors = Vec::new();
    key_diff("hu", &en, &hu, &mut errors);
    let mut files = en.len();
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for res in ANDROID_RES {
        let en = android_string_keys(&repo.join(res).join("values"))?;
        let hu = android_string_keys(&repo.join(res).join("values-hu"))?;
        key_diff(&format!("{res}/values-hu"), &en, &hu, &mut errors);
        files += en.len();
    }
    if errors.is_empty() {
        eprintln!("i18n-check: ok ({files} files)");
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

/// Generate the UniFFI sources for `language` (kotlin or swift) from a host build of
/// `labely-ffi` into `out` (default `target/bindings/<language>`).
fn bindings(language: &str, out: Option<String>) -> Result<()> {
    if !matches!(language, "kotlin" | "swift") {
        bail!("bindings: language must be kotlin or swift, not '{language}'");
    }
    cargo(&["build", "-p", "labely-ffi", "--lib"], &[])?;
    let lib = format!(
        "target/debug/{}labely_ffi{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let out = out.unwrap_or_else(|| format!("target/bindings/{language}"));
    cargo(
        &[
            "run",
            "-q",
            "-p",
            "labely-ffi",
            "--features",
            "bindgen",
            "--bin",
            "uniffi-bindgen",
            "--",
            "generate",
            "--library",
            &lib,
            "--language",
            language,
            "--out-dir",
            &out,
        ],
        &[],
    )?;
    eprintln!("bindings: {language} → {out}");
    Ok(())
}

/// Swift package that wraps the XCFramework (ADR-0030).
const IOS_PACKAGE: &str = "mobile/ios/LabelyFFI";
/// Rust targets in the XCFramework: device, Apple-silicon simulator, and macOS for
/// `swift test` on the host.
const APPLE_TARGETS: &[&str] = &[
    "aarch64-apple-ios",
    "aarch64-apple-ios-sim",
    "aarch64-apple-darwin",
];

/// Build `labely-ffi` as a static library for [`APPLE_TARGETS`], wrap it in
/// `LabelyFFIFFI.xcframework` and put it, with the generated Swift sources, into
/// [`IOS_PACKAGE`]. `--debug` skips `--release`.
fn xcframework(debug: bool) -> Result<()> {
    if std::env::consts::OS != "macos" {
        bail!("xcframework: needs macOS and Xcode");
    }
    let profile = if debug { "debug" } else { "release" };
    let envs = [
        ("IPHONEOS_DEPLOYMENT_TARGET", "18.0"),
        ("MACOSX_DEPLOYMENT_TARGET", "15.0"),
    ];
    for target in APPLE_TARGETS {
        let mut args = vec![
            "rustc",
            "-p",
            "labely-ffi",
            "--lib",
            "--crate-type",
            "staticlib",
            "--target",
            target,
        ];
        if !debug {
            args.push("--release");
        }
        cargo(&args, &envs)?;
    }

    let bindings_dir = "target/bindings/swift";
    bindings("swift", Some(bindings_dir.into()))?;
    let headers = std::path::Path::new("target/ios/headers");
    if headers.exists() {
        std::fs::remove_dir_all(headers)?;
    }
    std::fs::create_dir_all(headers)?;
    std::fs::copy(
        format!("{bindings_dir}/LabelyFFIFFI.h"),
        headers.join("LabelyFFIFFI.h"),
    )?;
    std::fs::copy(
        format!("{bindings_dir}/LabelyFFIFFI.modulemap"),
        headers.join("module.modulemap"),
    )?;

    let out = format!("{IOS_PACKAGE}/LabelyFFIFFI.xcframework");
    if std::path::Path::new(&out).exists() {
        std::fs::remove_dir_all(&out)?;
    }
    let mut args: Vec<String> = vec!["-create-xcframework".into()];
    for target in APPLE_TARGETS {
        args.extend([
            "-library".into(),
            format!("target/{target}/{profile}/liblabely_ffi.a"),
            "-headers".into(),
            headers.display().to_string(),
        ]);
    }
    args.extend(["-output".into(), out.clone()]);
    eprintln!("$ xcodebuild {}", args.join(" "));
    let status = Command::new("xcodebuild")
        .args(&args)
        .stdout(std::process::Stdio::null())
        .status()
        .context("running xcodebuild")?;
    if !status.success() {
        bail!("xcodebuild -create-xcframework failed");
    }

    let sources = format!("{IOS_PACKAGE}/Sources/LabelyFFI");
    std::fs::create_dir_all(&sources)?;
    std::fs::copy(
        format!("{bindings_dir}/LabelyFFI.swift"),
        format!("{sources}/LabelyFFI.swift"),
    )?;
    eprintln!("xcframework: {out} ({profile})");
    Ok(())
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
        "bindings" => bindings(
            &std::env::args().nth(2).unwrap_or_else(|| "kotlin".into()),
            std::env::args().nth(3),
        ),
        "xcframework" => xcframework(std::env::args().nth(2).as_deref() == Some("--debug")),
        _ => {
            eprintln!(
                "usage: cargo xtask <ci|layers|bless|i18n-check|schema-hash|bindings [kotlin|swift] [out-dir]|xcframework [--debug]>"
            );
            std::process::exit(2)
        }
    }
}
