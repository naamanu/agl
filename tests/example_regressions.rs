use agl::context::ExecutionContext;
use agl::stdlib::default_registry;
use agl::{check_program, execute_pipeline, parse_program};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;

#[derive(Deserialize)]
struct Example {
    source: String,
    pipeline: String,
    inputs: BTreeMap<String, Value>,
    expected: Value,
}

#[test]
fn deterministic_examples_match_expected_results() {
    let cases: Vec<Example> =
        serde_json::from_str(include_str!("fixtures/example_results.json")).unwrap();
    for case in cases {
        let source = std::fs::read_to_string(&case.source).unwrap();
        let program = parse_program(&source).unwrap();
        check_program(&program).unwrap();
        let actual = execute_pipeline(
            &program,
            &case.pipeline,
            case.inputs,
            &default_registry(&program),
            &ExecutionContext::default(),
        )
        .unwrap();
        assert_eq!(actual, case.expected, "regression in {}", case.source);
    }
}

#[test]
fn showcase_tests_run_without_external_executables() {
    let output = Command::new(env!("CARGO_BIN_EXE_agl"))
        .args(["examples/showcase_all_features.agent", "--test"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn removed_plugin_option_is_rejected_by_cli_and_repl() {
    for args in [
        vec!["examples/blog.agent", "--plugin", "legacy"],
        vec!["repl", "--plugin", "legacy"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_agl"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument '--plugin'"));
    }
    for shell in ["bash", "zsh", "fish"] {
        assert!(
            !agl::tooling::shell_completion(shell)
                .unwrap()
                .contains("--plugin")
        );
    }
}
