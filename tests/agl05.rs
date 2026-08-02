use agl::context::ExecutionContext;
use agl::event_store::{EventStore, SqliteEventStore};
use agl::{HandlerFailure, Registry, check_program, execute_pipeline, parse_program};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn persisted_approval_resumes_without_repeating_external_write() {
    let source = r#"
        language "0.5";
        task publish(id: String) -> String effects [external_write] idempotency keyed_by id {}
        pipeline release(id: String) -> String effects [external_write, human] {
          let receipt = publish(id);
          let approved = approve production "Release receipt?" expires 3600 delegate operator;
          if approved { return receipt; } else { return "rejected"; }
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let path = std::env::temp_dir().join(format!("agl-events-{}.sqlite", std::process::id()));
    let store = Arc::new(SqliteEventStore::open(&path).unwrap());
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let mut registry = Registry::default();
    registry.register("publish", move |args, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Ok(format!("receipt:{}", args["id"].as_str().unwrap()).into())
    });
    let inputs = || {
        [("id".into(), "order-7".into())]
            .into_iter()
            .collect::<BTreeMap<_, _>>()
    };
    let first = ExecutionContext::durable("release-7", store.clone(), false).unwrap();
    let suspended = execute_pipeline(&program, "release", inputs(), &registry, &first).unwrap_err();
    assert_eq!(suspended.kind, "suspended");
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let resumed = ExecutionContext::durable("release-7", store.clone(), true)
        .unwrap()
        .with_approvals(BTreeMap::from([("production".into(), true)]));
    let value = execute_pipeline(&program, "release", inputs(), &registry, &resumed).unwrap();
    assert_eq!(value, "receipt:order-7");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        1,
        "completed write was replayed"
    );
    assert!(
        store
            .load("release-7")
            .unwrap()
            .iter()
            .any(|event| event.kind == "human_approval")
    );
    let changed = parse_program(&source.replace("Release receipt?", "Changed prompt")).unwrap();
    let incompatible = ExecutionContext::durable("release-7", store.clone(), true).unwrap();
    assert_eq!(
        execute_pipeline(&changed, "release", inputs(), &registry, &incompatible)
            .unwrap_err()
            .kind,
        "resume_incompatible"
    );

    let secure = ExecutionContext::durable("secure", store.clone(), false)
        .unwrap()
        .with_secrets(["top-secret".into()]);
    secure.record(
        "example",
        serde_json::json!({"message":"contains top-secret", "api_key":"never-store-this"}),
    );
    let saved = store.load("secure").unwrap();
    assert_eq!(saved[0].fields["message"], "contains [REDACTED]");
    assert_eq!(saved[0].fields["api_key"], "[REDACTED]");
    assert!(store.prune_before(u128::MAX).unwrap() > 0);
    let _ = std::fs::remove_file(path);
}

#[test]
fn timeout_propagates_cooperative_cancellation() {
    let source = r#"
        language "0.5";
        task billable() -> String effects [model] idempotency idempotent {}
        pipeline main() -> String {
          let value = billable() timeout 0.01;
          return value;
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut registry = Registry::default();
    registry.register_contextual("billable", move |_, _, invocation| {
        while !invocation.cancellation.is_cancelled() {
            std::thread::yield_now();
        }
        sender.send(()).unwrap();
        Err(HandlerFailure::cancelled("cancelled by deadline"))
    });
    let error = execute_pipeline(
        &program,
        "main",
        BTreeMap::new(),
        &registry,
        &ExecutionContext::deterministic(1),
    )
    .unwrap_err();
    assert_eq!(error.kind, "timeout");
    receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("handler observed cancellation");
}

#[test]
fn parallel_map_is_bounded_and_ordered_and_race_cancels_losers() {
    let source = r#"
        language "0.5";
        task transform(value: String) -> String idempotency pure {}
        task fast() -> String idempotency idempotent {}
        task slow() -> String idempotency idempotent {}
        pipeline map_all(values: List[String]) -> List[String] budget { concurrency: 2 } {
          let outputs = parallel map value in values max_concurrency 2 collect_all {
            let output = run transform with { value: value };
          };
          return outputs;
        }
        pipeline first() -> String budget { concurrency: 2 } {
          let winner = race {
            let quick = fast();
            let delayed = slow();
          };
          return winner;
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let mut registry = Registry::default();
    registry.register("transform", |args, _| {
        Ok(format!("{}!", args["value"].as_str().unwrap()).into())
    });
    registry.register("fast", |_, _| Ok("fast".into()));
    let (sender, receiver) = std::sync::mpsc::channel();
    registry.register_contextual("slow", move |_, _, invocation| {
        while !invocation.cancellation.is_cancelled() {
            std::thread::yield_now();
        }
        sender.send(()).unwrap();
        Err(HandlerFailure::cancelled("lost race"))
    });
    let mapped = execute_pipeline(
        &program,
        "map_all",
        [("values".into(), serde_json::json!(["a", "b", "c"]))]
            .into_iter()
            .collect(),
        &registry,
        &ExecutionContext::deterministic(3),
    )
    .unwrap();
    assert_eq!(mapped, serde_json::json!(["a!", "b!", "c!"]));
    let winner = execute_pipeline(
        &program,
        "first",
        BTreeMap::new(),
        &registry,
        &ExecutionContext::deterministic(3),
    )
    .unwrap();
    assert_eq!(winner, "fast");
    receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .unwrap();
}
