//! `--data -` reads CSV from stdin.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json::Value;

const TEMPLATE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../core/templates/name-badge.labely"
);

/// Run `labely --json <args>` with `stdin` piped in; return the parsed envelope.
fn labely(args: &[&str], stdin: &str) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_labely"))
        .arg("--json")
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn labely");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin.as_bytes())
        .expect("write stdin");
    let out = child.wait_with_output().expect("wait labely");
    assert!(
        out.status.success(),
        "labely failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("JSON envelope")
}

fn path_str(p: &Path) -> &str {
    p.to_str().expect("UTF-8 temp path")
}

#[test]
fn export_pdf_reads_csv_from_stdin() {
    let dir = tempfile::tempdir().expect("tempdir");
    let pdf = dir.path().join("out.pdf");
    let env = labely(
        &["export", TEMPLATE, "-o", path_str(&pdf), "--data", "-"],
        "name,role\nAda,Eng\nBob,Ops\nCy,QA\n",
    );
    assert_eq!(env["result"]["pages"], 3, "{env}");
    assert!(pdf.is_file());
}

#[test]
fn print_reads_csv_from_stdin() {
    let dir = tempfile::tempdir().expect("tempdir");
    let printer = format!("fake:{}", path_str(dir.path()));
    let env = labely(
        &["print", TEMPLATE, "-p", &printer, "--data", "-"],
        "name\nAda\nBob\n",
    );
    assert_eq!(env["result"]["labels"], 2, "{env}");
    assert_eq!(env["result"]["state"], "done", "{env}");
}
