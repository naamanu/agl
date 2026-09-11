use agl::context::ExecutionContext;
use agl::stdlib::default_registry;
use agl::{analyze_program, check_program, execute_pipeline, parse_program};
use serde_json::{Value, json};

fn execute(source: &str, inputs: Value) -> Value {
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    execute_pipeline(
        &program,
        "main",
        inputs
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
        &default_registry(&program),
        &ExecutionContext::default(),
    )
    .unwrap()
}

fn rejects(source: &str, message: &str) {
    let error = check_program(&parse_program(source).unwrap()).unwrap_err();
    assert!(error.message.contains(message), "{error}");
}

#[test]
fn try_propagates_return_through_nested_handlers() {
    assert_eq!(
        execute(
            r#"
        pipeline main() -> Number {
          try {
            try { return 1; } catch inner { return 2; }
          } catch outer { return 3; }
          return 4;
        }
    "#,
            json!({})
        ),
        1
    );
}

#[test]
fn loops_consume_break_and_continue_through_try_and_optional_scope() {
    let source = r#"
        task countdown(current: Number) -> Obj{next: Number, done: Bool} {}
        pipeline main(saved: String, item: Option[String]) -> String {
          let state = countdown(4);
          while state.done == false {
            let state = countdown(state.next);
            try {
              if let saved = item {
                if state.next == 1 { break; }
                continue;
              }
            } catch err { return "caught"; }
            return "control was swallowed";
          }
          return saved;
        }
    "#;
    assert_eq!(
        execute(source, json!({"saved":"original", "item":"temporary"})),
        "original"
    );
}

#[test]
fn inner_loop_consumes_only_its_own_break() {
    assert_eq!(
        execute(
            r#"
        task countdown(current: Number) -> Obj{next: Number, done: Bool} {}
        pipeline main() -> Number {
          let state = countdown(3);
          while state.done == false {
            while true { try { break; } catch err { return 99; } }
            let state = countdown(state.next);
          }
          return state.next;
        }
    "#,
            json!({})
        ),
        0
    );
}

#[test]
fn optional_binding_restores_on_normal_and_error_exits() {
    for body in ["", "assert false, \"failure\";"] {
        let source = format!(
            r#"
            pipeline main(saved: String, item: Option[String]) -> String {{
              try {{ if let saved = item {{ {body} }} }}
              catch err {{ return saved; }}
              return saved;
            }}
        "#
        );
        for item in [json!("temporary"), Value::Null] {
            assert_eq!(
                execute(&source, json!({"saved":"original", "item":item})),
                "original"
            );
        }
    }
}

#[test]
fn optional_return_preserves_the_evaluated_value() {
    assert_eq!(
        execute(
            r#"
        pipeline main(saved: String, item: Option[String]) -> String {
          if let saved = item { return saved; } else { return "none"; }
        }
    "#,
            json!({"saved":"original","item":"temporary"})
        ),
        "temporary"
    );
}

#[test]
fn match_restores_payload_names_on_normal_and_error_exits() {
    for body in ["", "assert false, \"failure\";"] {
        let source = format!(
            r#"
            language "0.3";
            union Choice {{ Some {{ saved: String }}, None }};
            pipeline main(saved: String, choice: Choice) -> String {{
              try {{
                match choice {{
                  Choice::Some {{ saved }} => {{ {body} }}
                  Choice::None => {{}}
                }}
              }} catch err {{ return saved; }}
              return saved;
            }}
        "#
        );
        assert_eq!(
            execute(
                &source,
                json!({"saved":"original","choice":{"$type":"Choice","$variant":"Some","saved":"temporary"}})
            ),
            "original"
        );
    }
}

#[test]
fn match_restores_before_break_and_keeps_returned_payload() {
    let declarations = r#"language "0.3"; union Choice { Some { saved: String }, None };"#;
    let input = json!({"saved":"original","choice":{"$type":"Choice","$variant":"Some","saved":"temporary"}});
    let source = format!(
        r#"{declarations}
        pipeline main(saved: String, choice: Choice) -> String {{
          while true {{
            match choice {{
              Choice::Some {{ saved }} => {{ break; }}
              Choice::None => {{ break; }}
            }}
          }}
          return saved;
        }}
    "#
    );
    assert_eq!(execute(&source, input.clone()), "original");
    let source = format!(
        r#"{declarations}
        pipeline main(saved: String, choice: Choice) -> String {{
          match choice {{
            Choice::Some {{ saved }} => {{ return saved; }}
            Choice::None => {{ return saved; }}
          }}
        }}
    "#
    );
    assert_eq!(execute(&source, input), "temporary");
}

#[test]
fn catch_restores_shadowing_even_when_its_handler_fails() {
    for body in ["", "assert false, \"second failure\";"] {
        let source = format!(
            r#"
            pipeline main(saved: String) -> String {{
              try {{
                try {{ assert false, "first failure"; }} catch saved {{ {body} }}
              }} catch outer {{ return saved; }}
              return saved;
            }}
        "#
        );
        assert_eq!(execute(&source, json!({"saved":"original"})), "original");
    }
}

#[test]
fn catch_restores_structured_shadowing_on_break() {
    assert_eq!(
        execute(
            r#"
        language "0.3";
        pipeline main(saved: String) -> String {
          while true {
            try { assert false, "failure"; } catch saved: Failure { break; }
          }
          return saved;
        }
    "#,
            json!({"saved":"original"})
        ),
        "original"
    );
}

#[test]
fn temporary_names_cannot_escape_scopes() {
    for body in [
        "if let local = item {} else { let local = identity(\"other\"); }",
        "try { assert false; } catch local {}",
        "match Ok(\"payload\") { Result::Ok { value } => {} Result::Err { error } => {} }",
    ] {
        let name = if body.starts_with("match") {
            "value"
        } else {
            "local"
        };
        rejects(
            &format!(
                r#"
            language "0.3";
            pipeline identity(x: String) -> String {{ return x; }}
            pipeline main(item: Option[String]) -> String {{ {body} return {name}; }}
        "#
            ),
            "unknown reference",
        );
    }
}

#[test]
fn every_return_is_checked_in_every_language_version() {
    for version in ["0.2", "0.3", "0.4", "0.5", "0.6"] {
        let source = format!(
            r#"language "{version}";
            pipeline main(flag: Bool) -> Number {{
              if flag {{ return "wrong"; }} else {{ return 1; }}
            }}"#
        );
        let error = check_program(&parse_program(&source).unwrap()).unwrap_err();
        assert!(error.message.contains("expected Number, got String"));
        assert_eq!(error.line, 3);
    }
}

#[test]
fn only_normally_completing_branches_feed_the_tail() {
    let source = r#"
        language "0.3";
        pipeline identity(x: String) -> String { return x; }
        pipeline main(flag: Bool) -> String {
          if flag { return "early"; } else { let answer = identity("late"); }
          return answer;
        }
    "#;
    assert_eq!(execute(source, json!({"flag":true})), "early");
    assert_eq!(execute(source, json!({"flag":false})), "late");
}

#[test]
fn unreachable_returns_do_not_pollute_typing_but_still_warn() {
    let program =
        parse_program(r#"pipeline main() -> Number { return 1; return "unreachable"; }"#).unwrap();
    check_program(&program).unwrap();
    assert!(
        analyze_program(&program)
            .iter()
            .any(|warning| warning.code == "AGLW2002")
    );
}

#[test]
fn fallthrough_is_allowed_only_in_02() {
    for version in ["0.2", "0.3", "0.4", "0.5", "0.6"] {
        let source = format!(
            r#"language "{version}"; pipeline main(flag: Bool) -> Number {{ if flag {{ return 1; }} }}"#
        );
        let checked = check_program(&parse_program(&source).unwrap());
        assert_eq!(checked.is_ok(), version == "0.2");
    }
}

#[test]
fn parallel_branches_share_only_the_entry_scope() {
    for width in [1, 2] {
        rejects(
            &format!(
                r#"
            pipeline identity(x: Number) -> Number {{ return x; }}
            pipeline main() -> Number {{
              parallel max_concurrency {width} {{
                let a = identity(1);
                let b = identity(a);
              }} join;
              return b;
            }}
        "#
            ),
            "unknown reference",
        );
    }
    assert_eq!(
        execute(
            r#"
        pipeline identity(x: Number) -> Number { return x; }
        pipeline main() -> Number {
          parallel { let a = identity(1); let b = identity(2); } join;
          return a + b;
        }
    "#,
            json!({})
        ),
        3
    );
}

#[test]
fn loop_back_edges_and_break_exits_cannot_keep_stale_types() {
    for ending in ["", "continue;", "break;"] {
        rejects(
            &format!(
                r#"
            pipeline text() -> String {{ return "bad"; }}
            pipeline main(x: Number) -> Number {{
              while true {{ let x = text(); {ending} }}
              return x;
            }}
        "#
            ),
            if ending == "break;" {
                "unknown reference"
            } else {
                "expected Number, got String"
            },
        );
    }
}

#[test]
fn zero_iteration_path_and_safe_break_rebinding_remain_valid() {
    let source = r#"
        pipeline text() -> String { return "changed"; }
        pipeline main(x: Number, flag: Bool) -> Number {
          while flag { let x = text(); break; }
          return 7;
        }
    "#;
    for flag in [false, true] {
        assert_eq!(execute(source, json!({"x":1,"flag":flag})), 7);
    }
    rejects(
        r#"
        pipeline identity(x: Number) -> Number { return x; }
        pipeline main(flag: Bool) -> Number {
          while flag { let local = identity(1); break; }
          return local;
        }
    "#,
        "unknown reference",
    );
}

#[test]
fn catch_checks_every_failure_prefix_without_rolling_back_values() {
    rejects(
        r#"
        pipeline text() -> String { return "changed"; }
        pipeline main(x: Number) -> Number {
          try { let x = text(); assert false; } catch err { return x; }
          return 1;
        }
    "#,
        "unknown reference",
    );
    assert_eq!(
        execute(
            r#"
        pipeline text() -> String { return "changed"; }
        pipeline main(x: String) -> String {
          try { let x = text(); assert false; } catch err { return x; }
          return x;
        }
    "#,
            json!({"x":"original"})
        ),
        "changed"
    );
}

#[test]
fn typed_err_is_a_value_not_an_execution_error() {
    assert_eq!(
        execute(
            r#"
        language "0.3";
        pipeline main() -> Result[String, String] {
          try { return Err("domain"); } catch err { return Ok("caught"); }
        }
    "#,
            json!({})
        ),
        json!({"$type":"Result", "$variant":"Err", "error":"domain"})
    );
}

#[test]
fn catch_restores_before_continue() {
    assert_eq!(
        execute(
            r#"
        language "0.3";
        task countdown(current: Number) -> Obj{next: Number, done: Bool} {}
        pipeline main(saved: String) -> String {
          let state = countdown(3);
          while state.done == false {
            let state = countdown(state.next);
            try { assert false; } catch saved: Failure { continue; }
            return "continue was swallowed";
          }
          return saved;
        }
    "#,
            json!({"saved":"original"})
        ),
        "original"
    );
}

#[test]
fn unreachable_loop_control_still_requires_an_enclosing_loop() {
    for body in [
        "break;",
        "if true { continue; }",
        "try {} catch err { break; }",
    ] {
        rejects(
            &format!("pipeline main() -> Number {{ return 1; {body} }}"),
            "outside while loop",
        );
    }
}
