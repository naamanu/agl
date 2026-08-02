use agl::context::ExecutionContext;
use agl::stdlib::default_registry;
use agl::{check_program, execute_pipeline, parse_program};
use serde_json::Value;
use std::collections::BTreeMap;
use std::process::Command;

const PYTHON_ORACLE: &str = r#"
import json
import sys
from agentlang import check_program, default_task_registry, execute_pipeline, parse_program

path, pipeline, raw_inputs = sys.argv[1:]
program = parse_program(open(path, encoding='utf-8').read())
check_program(program)
result = execute_pipeline(
    program=program,
    pipeline_name=pipeline,
    inputs=json.loads(raw_inputs),
    task_registry=default_task_registry(program),
)
print(json.dumps(result, separators=(',', ':')))
"#;

#[test]
fn deterministic_examples_match_python_oracle() {
    // The published Rust crate intentionally excludes the Python reference.
    if !std::path::Path::new("agentlang").exists() {
        return;
    }
    let cases = [
        (
            "examples/blog.agent",
            "blog_post",
            serde_json::json!({"topic":"Rust agents"}),
        ),
        (
            "examples/support.agent",
            "support_reply",
            serde_json::json!({"message":"urgent refund please"}),
        ),
        (
            "examples/compare.agent",
            "compare_options",
            serde_json::json!({"query":"vector database"}),
        ),
        (
            "examples/reliability.agent",
            "resilient_brief",
            serde_json::json!({"topic":"retries","fail_count":1}),
        ),
    ];
    for (path, pipeline, inputs) in cases {
        let source = std::fs::read_to_string(path).unwrap();
        let program = parse_program(&source).unwrap();
        check_program(&program).unwrap();
        let rust_inputs: BTreeMap<_, _> = inputs
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let rust_result = execute_pipeline(
            &program,
            pipeline,
            rust_inputs,
            &default_registry(&program),
            &ExecutionContext::default(),
        )
        .unwrap();

        let output = Command::new("python3")
            .args(["-c", PYTHON_ORACLE, path, pipeline, &inputs.to_string()])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Python oracle failed for {path}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let last_line = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .last()
            .unwrap()
            .to_owned();
        let python_result: Value = serde_json::from_str(&last_line).unwrap();
        assert_eq!(rust_result, python_result, "parity mismatch for {path}");
    }
}
