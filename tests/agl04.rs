use agl::deployment::{DeploymentConfig, DeploymentError};
use agl::{check_program, format_pipeline, infer_program_effects, parse_program};
use std::collections::BTreeSet;

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
