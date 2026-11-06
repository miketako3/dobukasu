//! What the shipped binary actually contains, checked from the outside.
//!
//! A plug-in is only in the binary because some `use ... as _;` keeps the
//! linker from dropping it, so this is the test that notices when an engine
//! crate is added but never linked.

use std::process::Command;

use serde_json::Value;

fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_dobukasu"))
        .args(args)
        .output()
        .expect("the binary runs")
}

#[test]
fn the_binary_ships_the_free_engines_and_makes_them_the_default() {
    let output = run(&["engines"]);
    assert!(
        output.status.success(),
        "engines exited with {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let body: Value = serde_json::from_slice(&output.stdout).expect("engines prints JSON");
    let entries = body["engines"].as_array().expect("engines array");
    let ids: Vec<&str> = entries
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();

    assert_eq!(ids, vec!["bing", "duckduckgo", "google"]);
    assert!(
        entries.iter().all(|entry| entry["default"] == true),
        "the default set must be the keyless engines: {body}"
    );
    assert_eq!(
        body.get("unavailable"),
        None,
        "nothing should be missing in a stock build"
    );
}

#[test]
fn the_binary_refuses_a_flag_it_does_not_know() {
    let output = run(&["--nope"]);

    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, &[] as &[u8]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown flag"));
}
