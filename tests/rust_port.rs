use agl::context::ExecutionContext;
use agl::stdlib::default_registry;
use agl::{check_program, execute_pipeline, parse_program};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;

#[test]
fn parses_and_checks_every_example() {
    for entry in fs::read_dir("examples").unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|x| x.to_str()) != Some("agent") {
            continue;
        }
        let name = path.display();
        let source = fs::read_to_string(&path).unwrap();
        let program = parse_program(&source).unwrap_or_else(|e| panic!("{name}: {e}"));
        check_program(&program).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn deterministic_blog_matches_reference_result() {
    let program = parse_program(&fs::read_to_string("examples/blog.agent").unwrap()).unwrap();
    let result = execute_pipeline(
        &program,
        "blog_post",
        BTreeMap::from([("topic".into(), Value::String("Rust agents".into()))]),
        &default_registry(&program),
        &ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(
        result,
        Value::String("[writer] Draft article:\n[planner] key points for 'Rust agents'".into())
    );
}

#[test]
fn retry_path_succeeds() {
    let program =
        parse_program(&fs::read_to_string("examples/reliability.agent").unwrap()).unwrap();
    let result = execute_pipeline(
        &program,
        "resilient_brief",
        BTreeMap::from([
            ("topic".into(), Value::String("retries".into())),
            ("fail_count".into(), Value::from(1)),
        ]),
        &default_registry(&program),
        &ExecutionContext::default(),
    )
    .unwrap();
    assert!(result.as_str().unwrap().contains("fetched payload"));
}
