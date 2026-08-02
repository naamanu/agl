use agl::context::ExecutionContext;
use agl::deployment::{DeploymentConfig, DeploymentError};
use agl::evaluation::run_evaluation;
use agl::{
    Registry, TaskOutput, Usage, check_program, execute_pipeline, format_pipeline,
    infer_program_effects, parse_program,
};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn effects_propagate_through_agents_tools_and_pipeline_calls() {
    let source = r#"
        language "0.4";
        tool search(query: String) -> String
          effects [network, external_read]
          idempotency idempotent {}
        agent researcher { tools: [search] }
        task investigate(query: String) -> String by agent {}
        pipeline leaf(query: String) -> String
          effects [model, network, external_read] {
          let result = investigate(query) by researcher;
          return result;
        }
        pipeline main(query: String) -> String {
          let result = leaf(query);
          return result;
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let inferred = infer_program_effects(&program);
    let expected = BTreeSet::from(["external_read".into(), "model".into(), "network".into()]);
    assert_eq!(inferred["leaf"], expected);
    assert_eq!(inferred["main"], expected);
    assert!(
        format_pipeline(&program.pipelines["leaf"])
            .contains("effects [external_read, model, network]")
    );
}

#[test]
fn retries_have_deterministic_backoff_and_stable_keys() {
    let source = r#"
        language "0.4";
        task publish(key: String) -> String effects [external_write] idempotency keyed_by key {}
        pipeline main(key: String) -> String budget { retries: 2, tool_calls: 3 } {
          let value = publish(key) retries 2 backoff { initial_ms: 10, max_ms: 100, multiplier: 2, jitter: 0.25 };
          return value;
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let attempts = Arc::new(AtomicUsize::new(0));
    let seen = attempts.clone();
    let mut registry = Registry::default();
    registry.register("publish", move |_, _| {
        if seen.fetch_add(1, Ordering::SeqCst) < 2 {
            Err("transient".into())
        } else {
            Ok("done".into())
        }
    });
    let context = ExecutionContext::deterministic(7);
    let value = execute_pipeline(
        &program,
        "main",
        BTreeSet::<(String, serde_json::Value)>::new()
            .into_iter()
            .collect(),
        &registry,
        &context,
    );
    assert!(value.is_err(), "a String input is still required");
    let value = execute_pipeline(
        &program,
        "main",
        [("key".into(), "order-1".into())].into_iter().collect(),
        &registry,
        &context,
    )
    .unwrap();
    assert_eq!(value, "done");
    let retries: Vec<_> = context
        .events()
        .into_iter()
        .filter(|event| event.kind == "task_retry")
        .collect();
    assert_eq!(retries.len(), 2);
    assert!(
        retries
            .iter()
            .all(|event| event.fields["idempotency_key"] == "order-1")
    );
    assert_ne!(retries[0].fields["delay_ms"], retries[1].fields["delay_ms"]);
}

#[test]
fn usage_exhausts_a_typed_pipeline_budget() {
    let source = r#"
        language "0.4";
        task generate() -> String effects [model] idempotency idempotent {}
        pipeline main() -> String budget { tokens: 5, cost_usd: 0.01, tool_calls: 1 } {
          let value = generate();
          return value;
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let mut registry = Registry::default();
    registry.register_contextual("generate", |_, _, _| {
        Ok(TaskOutput {
            value: "answer".into(),
            usage: Usage {
                input_tokens: 4,
                output_tokens: 3,
                cost_usd: 0.005,
                estimated: false,
            },
        })
    });
    let error = execute_pipeline(
        &program,
        "main",
        Default::default(),
        &registry,
        &ExecutionContext::deterministic(1),
    )
    .unwrap_err();
    assert_eq!(error.kind, "budget");
    assert_eq!(error.operation.as_deref(), Some("tokens"));
}

#[test]
fn dataset_evaluations_repeat_and_replay_deterministically() {
    let source = r#"
        language "0.4";
        task echo(value: String) -> String idempotency pure {}
        pipeline main(value: String) -> String {
          let output = echo(value);
          return output;
        }
        eval regression {
          pipeline: main,
          dataset: "cases.jsonl",
          trials: 3,
          assert_schema: true,
          assert_expected: true,
          max_latency_ms: 1000,
          max_cost_usd: 0.01
        }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let directory = std::env::temp_dir().join(format!("agl-eval-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("cases.jsonl"),
        "{\"input\":{\"value\":\"hello\"},\"expected\":\"hello\"}\n",
    )
    .unwrap();
    let mut registry = Registry::default();
    registry.register("echo", |args, _| Ok(args["value"].clone()));
    let report = run_evaluation(
        &program,
        &program.evals["regression"],
        &directory,
        &registry,
    )
    .unwrap();
    assert_eq!(report.trials, 3);
    assert_eq!(report.pass_rate, 1.0);

    let context = ExecutionContext::deterministic(9);
    execute_pipeline(
        &program,
        "main",
        [("value".into(), "replayed".into())].into_iter().collect(),
        &registry,
        &context,
    )
    .unwrap();
    let replay = Registry::from_trace(&context.events());
    let replayed = execute_pipeline(
        &program,
        "main",
        [("value".into(), "ignored".into())].into_iter().collect(),
        &replay,
        &ExecutionContext::deterministic(9),
    )
    .unwrap();
    assert_eq!(replayed, "replayed");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn declared_effect_ceiling_rejects_missing_capabilities() {
    let source = r#"
        language "0.4";
        task fetch() -> String effects [network] idempotency idempotent {}
        pipeline main() -> String effects [] {
          let value = fetch();
          return value;
        }
    "#;
    let program = parse_program(source).unwrap();
    let error = check_program(&program).unwrap_err();
    assert!(error.message.contains("does not permit inferred effects"));
}

#[test]
fn external_writes_require_retry_safe_idempotency() {
    let unsafe_source = r#"
        language "0.4";
        task publish(id: String) -> String
          effects [external_write]
          idempotency non_idempotent {}
        pipeline main(id: String) -> String {
          let value = publish(id) retries 1;
          return value;
        }
    "#;
    let program = parse_program(unsafe_source).unwrap();
    assert!(
        check_program(&program)
            .unwrap_err()
            .message
            .contains("cannot be retried safely")
    );

    let safe_source = unsafe_source.replace("non_idempotent", "keyed_by id");
    let program = parse_program(&safe_source).unwrap();
    check_program(&program).unwrap();
}

#[test]
fn deployment_bindings_validate_agent_requirements() {
    let source = r#"
        language "0.4";
        agent researcher {
          tools: [],
          requires: [reasoning, tool_calling],
          min_context: 100000,
          max_latency_ms: 5000,
          quality: "high"
        }
    "#;
    let mut program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let config = DeploymentConfig::from_json(
        r#"{
          "agents": {
            "researcher": {
              "provider": "openai",
              "model": "production-model",
              "reasoning_effort": "medium",
              "capabilities": ["reasoning", "tool_calling"],
              "context_window": 200000,
              "expected_latency_ms": 3000,
              "quality": "frontier"
            }
          }
        }"#,
    )
    .unwrap();
    config.apply(&mut program, Some("openai")).unwrap();
    let binding = program.agents["researcher"].deployment.as_ref().unwrap();
    assert_eq!(binding.model, "production-model");
    assert_eq!(binding.reasoning_effort.as_deref(), Some("medium"));

    let mut program = parse_program(source).unwrap();
    let missing = DeploymentConfig::from_json(
        r#"{"agents":{"researcher":{"provider":"openai","model":"small","capabilities":[]}}}"#,
    )
    .unwrap();
    assert!(matches!(
        missing.apply(&mut program, Some("openai")),
        Err(DeploymentError::Capabilities { .. })
    ));
}
