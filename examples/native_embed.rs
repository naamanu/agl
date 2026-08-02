use agl::context::ExecutionContext;
use agl::{Registry, check_program, execute_pipeline, parse_program};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = r#"
        task uppercase(text: String) -> Obj{text: String} {}
        pipeline main(text: String) -> String {
          let result = uppercase(text);
          return result.text;
        }
    "#;
    let program = parse_program(source)?;
    check_program(&program)?;

    let mut registry = Registry::default();
    registry.register("uppercase", |args, _agent| {
        let text = args.get("text").and_then(Value::as_str).unwrap_or_default();
        Ok(json!({"text":text.to_uppercase()}))
    });
    let result = execute_pipeline(
        &program,
        "main",
        BTreeMap::from([("text".into(), Value::String("native AGL".into()))]),
        &registry,
        &ExecutionContext::default(),
    )?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
