use agl::context::ExecutionContext;
use agl::{Registry, check_program, execute_pipeline, parse_program};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

#[test]
fn task_can_return_a_typed_domain_error_without_throwing() {
    let source = r#"
        language "0.3";
        union LookupError { Missing { key: String }, Unavailable };
        task lookup(key: String) -> Result[String, LookupError] {}
        pipeline main(key: String) -> String {
          let result = lookup(key);
          match result {
            Result::Ok { value } => { return value; }
            Result::Err { error } => {
              match error {
                LookupError::Missing { key } => { return "missing: " + key; }
                LookupError::Unavailable => { return "unavailable"; }
              }
            }
          }
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let mut registry = Registry::default();
    registry.register("lookup", |args, _| {
        Ok(json!({
            "$type": "Result",
            "$variant": "Err",
            "error": {
                "$type": "LookupError",
                "$variant": "Missing",
                "key": args["key"],
            }
        }))
    });
    let result = execute_pipeline(
        &program,
        "main",
        BTreeMap::from([("key".into(), json!("profile"))]),
        &registry,
        &ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(result, "missing: profile");
}

#[test]
fn retry_policy_selects_typed_error_variants() {
    let source = r#"
        language "0.3";
        union FetchError { Network, Invalid };
        task fetch() -> Result[String, FetchError] {}
        pipeline main() -> String {
          let result = fetch() retries 2 retry_on [FetchError::Network];
          match result {
            Result::Ok { value } => { return value; }
            Result::Err { error } => {
              match error {
                FetchError::Network => { return "network"; }
                FetchError::Invalid => { return "invalid"; }
              }
            }
          }
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let attempts = Arc::new(Mutex::new(0));
    let shared = attempts.clone();
    let mut registry = Registry::default();
    registry.register("fetch", move |_, _| {
        let mut count = shared.lock().unwrap();
        *count += 1;
        Ok(if *count == 1 {
            json!({
                "$type":"Result",
                "$variant":"Err",
                "error":{"$type":"FetchError","$variant":"Network"}
            })
        } else {
            json!({"$type":"Result","$variant":"Ok","value":"recovered"})
        })
    });
    let result = execute_pipeline(
        &program,
        "main",
        BTreeMap::new(),
        &registry,
        &ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(result, "recovered");
    assert_eq!(*attempts.lock().unwrap(), 2);
}

#[test]
fn structured_catch_exposes_execution_failure_data() {
    let source = r#"
        language "0.3";
        task explode() -> String {}
        pipeline main() -> String {
          try {
            let value = explode();
            return value;
          } catch failure: Failure {
            return failure.kind + ": " + failure.message;
          }
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let mut registry = Registry::default();
    registry.register("explode", |_, _| Err("provider unavailable".into()));
    let result = execute_pipeline(
        &program,
        "main",
        BTreeMap::new(),
        &registry,
        &ExecutionContext::default(),
    )
    .unwrap();
    assert_eq!(
        result,
        "task: task 'explode' failed after 1 attempt(s): provider unavailable"
    );
}
