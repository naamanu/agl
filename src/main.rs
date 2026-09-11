use agl::adapters::tools::default_tool_registry;
use agl::context::ExecutionContext;
use agl::deployment::DeploymentConfig;
use agl::diagnostic::{RenderedDiagnostic, render_diagnostic};
use agl::evaluation::{EvalBaseline, run_evaluation};
use agl::event_store::SqliteEventStore;
use agl::stdlib::{AdapterMode, registry_for_with_tools};
use agl::{
    analyze_program, check_program, execute_pipeline_async, format_pipeline, infer_program_effects,
    parse_program, run_tests,
};
use clap::Parser;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

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
    #[arg(long, default_value = "mock", value_parser = ["mock", "live", "openai", "anthropic"])]
    adapter: String,
    #[arg(long)]
    trace_live: bool,
    #[arg(long)]
    lower: bool,
    #[arg(long)]
    effects: bool,
    #[arg(long)]
    deployment: Option<PathBuf>,
    #[arg(long, value_name = "NAME")]
    eval: Option<String>,
    #[arg(long, requires = "eval")]
    update_baseline: bool,
    #[arg(long)]
    event_store: Option<PathBuf>,
    #[arg(long)]
    execution_id: Option<String>,
    #[arg(long, requires = "event_store")]
    resume: bool,
    #[arg(long = "approval", value_name = "NAME=BOOL")]
    approvals: Vec<String>,
    #[arg(long)]
    docs: Option<PathBuf>,
    #[arg(long)]
    api: Option<PathBuf>,
    #[arg(long)]
    format: bool,
    #[arg(long)]
    policy: Option<PathBuf>,
    #[arg(long)]
    summary: bool,
}

#[derive(Parser)]
#[command(name = "agl repl", about = "Interactive AGL session")]
struct ReplCli {
    #[arg(long, default_value = "mock", value_parser = ["mock", "live", "openai", "anthropic"])]
    adapter: String,
    #[arg(long)]
    trace_live: bool,
    #[arg(long)]
    deployment: Option<PathBuf>,
}

fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("lsp") => return exit_result(agl::tooling::serve_lsp()),
        Some("protocol") => return exit_result(agl::tooling::serve_json_lines()),
        Some("completions") => {
            let shell = std::env::args().nth(2).unwrap_or_default();
            match agl::tooling::shell_completion(&shell) {
                Ok(value) => print!("{value}"),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(2);
                }
            }
            return;
        }
        Some("package") => return exit_result(package_command()),
        Some("api-compare") => return exit_result(api_compare_command()),
        _ => {}
    }
    if std::env::args().nth(1).as_deref() == Some("repl") {
        if let Err(e) = repl() {
            eprintln!("REPL error: {e}");
            std::process::exit(1)
        }
        return;
    }
    if let Err(e) = run() {
        eprintln!("Execution error: {e}");
        std::process::exit(1)
    }
}

fn exit_result(result: Result<(), Box<dyn std::error::Error>>) {
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn package_command() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    if args.first().map(String::as_str) != Some("lock") {
        return Err("usage: agl package lock <agl.json> [agl.lock]".into());
    }
    let manifest = args.get(1).ok_or("manifest path is required")?;
    let lock = args.get(2).cloned().unwrap_or_else(|| {
        std::path::Path::new(manifest)
            .with_file_name("agl.lock")
            .display()
            .to_string()
    });
    let value = agl::package::write_lock(manifest, &lock)?;
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}

fn api_compare_command() -> Result<(), Box<dyn std::error::Error>> {
    let previous = std::env::args()
        .nth(2)
        .ok_or("previous API JSON is required")?;
    let current = std::env::args()
        .nth(3)
        .ok_or("current API JSON is required")?;
    let previous = serde_json::from_str(&std::fs::read_to_string(previous)?)?;
    let current = serde_json::from_str(&std::fs::read_to_string(current)?)?;
    let report = agl::documentation::compare_api(&previous, &current);
    println!("{}", serde_json::to_string_pretty(&report)?);
    if !report.compatible {
        return Err("public API contains breaking changes".into());
    }
    Ok(())
}

fn repl() -> Result<(), Box<dyn std::error::Error>> {
    let args = ReplCli::parse_from(
        std::iter::once("agl repl".to_string()).chain(std::env::args().skip(2)),
    );
    let mode = args.adapter.parse::<AdapterMode>()?;
    let mut current = None;
    println!(
        "AGL REPL (adapter={}). Type 'help' for commands, 'exit' to quit.",
        args.adapter
    );
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            println!();
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (command, rest) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        match command.to_ascii_lowercase().as_str() {
            "exit" | "quit" => break,
            "help" => println!(
                "Commands:\n  load <path>                Load and check an .agent file\n  run <pipeline> [json]      Execute a loaded pipeline\n  lower <pipeline>           Print lowered pipeline IR\n  list                       List loaded definitions\n  clear                      Clear session state\n  exit                       Quit"
            ),
            "clear" => {
                current = None;
                println!("Session cleared.");
            }
            "load" => match load_program(
                rest.trim(),
                args.deployment.as_deref(),
                provider_name(&args.adapter),
            ) {
                Ok(program) => {
                    println!(
                        "Loaded '{}': {} agents, {} tasks, {} pipelines.",
                        rest.trim(),
                        program.agents.len(),
                        program.tasks.len(),
                        program.pipelines.len()
                    );
                    current = Some(program);
                }
                Err(e) => eprintln!("Load error: {e}"),
            },
            "list" => match &current {
                None => eprintln!("No program loaded. Use 'load <path>' first."),
                Some(program) => {
                    for name in program.agents.keys() {
                        println!("  Agent: {name}");
                    }
                    for name in program.tasks.keys() {
                        println!("  Task: {name}");
                    }
                    for name in program.pipelines.keys() {
                        println!("  Pipeline: {name}");
                    }
                }
            },
            "lower" => match &current {
                None => eprintln!("No program loaded. Use 'load <path>' first."),
                Some(program) => match program.pipelines.get(rest.trim()) {
                    Some(pipeline) => println!("{}", format_pipeline(pipeline)),
                    None => eprintln!("Unknown pipeline or workflow '{}'.", rest.trim()),
                },
            },
            "run" => {
                let Some(program) = &current else {
                    eprintln!("No program loaded. Use 'load <path>' first.");
                    continue;
                };
                let (name, raw_input) = rest
                    .trim()
                    .split_once(char::is_whitespace)
                    .unwrap_or((rest.trim(), "{}"));
                let result = (|| -> Result<Value, Box<dyn std::error::Error>> {
                    let raw: Value = serde_json::from_str(raw_input.trim())?;
                    let object = raw.as_object().ok_or("input JSON must be an object")?;
                    let inputs = object.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                    let registry = build_registry(program, mode, args.trace_live, None)?;
                    Ok(execute_async(
                        program,
                        name,
                        inputs,
                        &registry,
                        &ExecutionContext::default(),
                    )?)
                })();
                match result {
                    Ok(value) => println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({"result":value}))?
                    ),
                    Err(e) => eprintln!("Execution error: {e}"),
                }
            }
            unknown => eprintln!("Unknown command '{unknown}'. Type 'help' for commands."),
        }
    }
    Ok(())
}

fn load_program(
    path: &str,
    deployment: Option<&std::path::Path>,
    provider: Option<&str>,
) -> Result<agl::ast::Program, Box<dyn std::error::Error>> {
    if path.is_empty() {
        return Err("usage: load <path>".into());
    }
    let source = std::fs::read_to_string(path)?;
    let mut program = parse_program(&source)
        .map_err(|error| RenderedDiagnostic(render_diagnostic(path, &source, &error)))?;
    if !program.imports.is_empty() {
        program = agl::modules::load_module(path)?;
    }
    check_program(&program)
        .map_err(|error| RenderedDiagnostic(render_diagnostic(path, &source, &error)))?;
    if let Some(path) = deployment {
        DeploymentConfig::load(path)?.apply(&mut program, provider)?;
    }
    Ok(program)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let source = std::fs::read_to_string(&cli.source)?;
    let mut program = parse_program(&source).map_err(|error| {
        RenderedDiagnostic(render_diagnostic(cli.source.display(), &source, &error))
    })?;
    if cli.format {
        print!("{}", agl::format_program(&program));
        return Ok(());
    }
    if !program.imports.is_empty() {
        program = agl::modules::load_module(&cli.source)?;
    }
    check_program(&program).map_err(|error| {
        RenderedDiagnostic(render_diagnostic(cli.source.display(), &source, &error))
    })?;
    if let Some(path) = &cli.deployment {
        DeploymentConfig::load(path)?.apply(&mut program, provider_name(&cli.adapter))?;
    }
    let policy = cli
        .policy
        .as_ref()
        .map(agl::policy::DeploymentPolicy::load)
        .transpose()?
        .map(Arc::new);
    if let Some(policy) = &policy {
        policy.validate_program(&program)?;
    }
    for warning in analyze_program(&program) {
        eprintln!(
            "{}",
            render_diagnostic(cli.source.display(), &source, &warning)
        );
    }
    if cli.summary {
        println!(
            "{}",
            serde_json::to_string_pretty(&agl::policy::static_summary(&program))?
        );
        return Ok(());
    }
    if let Some(path) = &cli.docs {
        std::fs::write(
            path,
            agl::documentation::markdown(&program, &cli.source.display().to_string()),
        )?;
        return Ok(());
    }
    if let Some(path) = &cli.api {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&agl::documentation::api_interface(&program))?,
        )?;
        return Ok(());
    }
    if cli.effects {
        println!(
            "{}",
            serde_json::to_string_pretty(&infer_program_effects(&program))?
        );
        return Ok(());
    }
    if cli.lower {
        let name = cli.pipeline.ok_or("pipeline is required with --lower")?;
        let pipeline = program
            .pipelines
            .get(&name)
            .ok_or_else(|| format!("unknown pipeline or workflow '{name}'"))?;
        println!("{}", format_pipeline(pipeline));
        return Ok(());
    }
    if cli.check {
        println!("OK: {}", cli.source.display());
        return Ok(());
    }
    let registry = build_registry(
        &program,
        cli.adapter.parse::<AdapterMode>()?,
        cli.trace_live,
        policy,
    )?;
    if let Some(name) = &cli.eval {
        let definition = program
            .evals
            .get(name)
            .ok_or_else(|| format!("unknown evaluation '{name}'"))?;
        let root = cli
            .source
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let mut runnable = definition.clone();
        if cli.update_baseline {
            runnable.baseline = None;
        }
        let report = run_evaluation(&program, &runnable, root, &registry)?;
        if cli.update_baseline {
            let path = definition
                .baseline
                .as_ref()
                .ok_or("evaluation has no baseline path")?;
            std::fs::write(
                root.join(path),
                serde_json::to_string_pretty(&EvalBaseline::from_report(&report))?,
            )?;
        }
        println!("{}", serde_json::to_string_pretty(&report)?);
        if report.passed != report.trials {
            return Err(format!(
                "evaluation failed {}/{} trials",
                report.trials - report.passed,
                report.trials
            )
            .into());
        }
        return Ok(());
    }
    let approvals = parse_approvals(&cli.approvals)?;
    let context = if let Some(path) = &cli.event_store {
        let execution_id = cli
            .execution_id
            .clone()
            .unwrap_or_else(|| format!("agl-{}", std::process::id()));
        ExecutionContext::durable(
            execution_id,
            Arc::new(SqliteEventStore::open(path)?),
            cli.resume,
        )?
        .with_approvals(approvals)
    } else {
        if cli.execution_id.is_some() {
            return Err("--execution-id requires --event-store".into());
        }
        ExecutionContext::default().with_approvals(approvals)
    };
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
        let result = execute_async(&program, &name, inputs, &registry, &context)?;
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

fn execute_async(
    program: &agl::ast::Program,
    name: &str,
    inputs: BTreeMap<String, Value>,
    registry: &agl::Registry,
    context: &ExecutionContext,
) -> Result<Value, agl::ExecutionError> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| agl::ExecutionError {
            kind: "runtime",
            message: error.to_string(),
            operation: None,
            retryable: false,
        })?
        .block_on(execute_pipeline_async(
            Arc::new(program.clone()),
            name.into(),
            inputs,
            registry.clone(),
            context.clone(),
        ))
}

fn parse_approvals(
    values: &[String],
) -> Result<BTreeMap<String, bool>, Box<dyn std::error::Error>> {
    values
        .iter()
        .map(|value| {
            let (name, decision) = value.split_once('=').ok_or("approval must use NAME=BOOL")?;
            let decision = decision
                .parse::<bool>()
                .map_err(|_| "approval decision must be true or false")?;
            Ok((name.to_owned(), decision))
        })
        .collect()
}

fn provider_name(adapter: &str) -> Option<&str> {
    match adapter {
        "live" | "openai" => Some("openai"),
        "anthropic" => Some("anthropic"),
        _ => None,
    }
}

fn build_registry(
    program: &agl::ast::Program,
    mode: AdapterMode,
    trace_live: bool,
    policy: Option<Arc<agl::policy::DeploymentPolicy>>,
) -> Result<agl::Registry, Box<dyn std::error::Error>> {
    let mut tools = default_tool_registry(Duration::from_secs(15))?;
    if let Some(policy) = policy {
        tools.set_policy(policy);
    }
    Ok(registry_for_with_tools(program, mode, trace_live, tools)?)
}
