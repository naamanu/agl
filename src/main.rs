use agl::context::ExecutionContext;
use agl::stdlib::default_registry;
use agl::{check_program, execute_pipeline, parse_program, run_tests};
use clap::Parser;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "agl", version, about = "Run typed AGL agent workflows")]
struct Cli {
    source: PathBuf,
    pipeline: Option<String>,
    #[arg(long, default_value = "{}")]
    input: String,
    #[arg(long)]
    test: bool,
    #[arg(long)]
    check: bool,
    #[arg(long)]
    output_trace: Option<PathBuf>,
}
fn main() {
    if let Err(e) = run() {
        eprintln!("Execution error: {e}");
        std::process::exit(1)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let source = std::fs::read_to_string(&cli.source)?;
    let program = parse_program(&source)?;
    check_program(&program)?;
    if cli.check {
        println!("OK: {}", cli.source.display());
        return Ok(());
    }
    let registry = default_registry(&program);
    let context = ExecutionContext::default();
    if cli.test {
        let results = run_tests(&program, &registry, &context);
        let mut failed = 0;
        for (name, result) in results {
            match result {
                Ok(()) => println!("  PASS: {name}"),
                Err(e) => {
                    failed += 1;
                    println!("  FAIL: {name} — {e}")
                }
            }
        }
        if failed > 0 {
            return Err(format!("{failed} test(s) failed").into());
        }
    } else {
        let name = cli
            .pipeline
            .ok_or("pipeline is required unless --test or --check is used")?;
        let raw: Value = serde_json::from_str(&cli.input)?;
        let obj = raw.as_object().ok_or("--input must be a JSON object")?;
        let inputs: BTreeMap<_, _> = obj.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        let result = execute_pipeline(&program, &name, inputs, &registry, &context)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"result":result}))?
        )
    }
    if let Some(path) = cli.output_trace {
        std::fs::write(path, serde_json::to_string_pretty(&context.events())?)?
    }
    Ok(())
}
