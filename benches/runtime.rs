use agl::context::ExecutionContext;
use agl::{Registry, execute_pipeline, parse_program};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const SOURCE: &str = r#"
task delay(value: String) -> Obj{value: String} {}
pipeline sequential(value: String) -> String {
  let a = delay(value);
  let b = delay(value);
  return b.value;
}
pipeline concurrent(value: String) -> String {
  parallel {
    let a = delay(value);
    let b = delay(value);
  } join;
  return b.value;
}
"#;

fn main() {
    let program = parse_program(SOURCE).unwrap();
    let mut registry = Registry::default();
    registry.register("delay", |args, _| {
        std::thread::sleep(Duration::from_millis(25));
        Ok(json!({"value":args["value"]}))
    });
    let iterations = 12;
    let sequential = measure(&program, &registry, "sequential", iterations);
    let concurrent = measure(&program, &registry, "concurrent", iterations);
    println!(
        "sequential_ms={:.2} concurrent_ms={:.2} speedup={:.2}x iterations={iterations}",
        sequential.as_secs_f64() * 1000.0,
        concurrent.as_secs_f64() * 1000.0,
        sequential.as_secs_f64() / concurrent.as_secs_f64(),
    );
}

fn measure(
    program: &agl::ast::Program,
    registry: &Registry,
    pipeline: &str,
    iterations: u32,
) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        execute_pipeline(
            program,
            pipeline,
            BTreeMap::from([("value".into(), Value::String("benchmark".into()))]),
            registry,
            &ExecutionContext::default(),
        )
        .unwrap();
    }
    start.elapsed() / iterations
}
