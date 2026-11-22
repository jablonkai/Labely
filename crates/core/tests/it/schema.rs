use serde_json::json;

use crate::ok;

fn has_ref(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(m) => m.contains_key("$ref") || m.values().any(has_ref),
        serde_json::Value::Array(a) => a.iter().any(has_ref),
        _ => false,
    }
}

#[test]
fn command_list_schemas_snapshot() {
    let mut s = labely_testkit::test_session();
    let r = ok(&mut s, "command.list", json!({}));
    let commands = r["commands"].as_array().unwrap();
    assert_eq!(commands.len(), s.registry().entries().len());
    for c in commands {
        assert!(!has_ref(&c["params_schema"]), "{} params has $ref", c["id"]);
        assert!(!has_ref(&c["output_schema"]), "{} output has $ref", c["id"]);
        assert_eq!(c["params_schema"]["type"], "object", "{}", c["id"]);
        assert!(!c["description"].as_str().unwrap().is_empty());
    }
    insta::assert_json_snapshot!("command_list", r);
}

#[test]
fn command_list_prefix() {
    let mut s = labely_testkit::test_session();
    let r = ok(&mut s, "command.list", json!({"prefix": "history."}));
    let ids: Vec<_> = r["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["history.undo", "history.redo", "history.journal"]);
}

/// API version and the schema hash it was released with. When this fails, the command
/// schemas changed: bump `API_VERSION` in crates/core/src/lib.rs (if it was not bumped yet in
/// this release cycle) and paste the new hash here.
const PINNED: (u32, &str) = (
    3,
    "79772b3e1aaa45cbe013d1419a7167387bfc75c76ae22e13a89afb5803ad97ed",
);

#[test]
fn api_version_matches_schema_hash() {
    let hash = labely_core::Registry::builtin().schema_hash();
    println!("schema hash: {hash}");
    assert_eq!(
        (labely_core::API_VERSION, hash.as_str()),
        PINNED,
        "command schemas changed: bump API_VERSION if needed and update PINNED"
    );
}
