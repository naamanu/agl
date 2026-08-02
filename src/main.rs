use agl::adapters::tools::default_tool_registry;
use agl::context::ExecutionContext;
use agl::diagnostic::{RenderedDiagnostic, render_diagnostic};
use agl::plugins::load_python_plugin_with_tools;
use agl::stdlib::{AdapterMode, registry_for_with_tools};
use agl::{
    analyze_program, check_program, execute_pipeline, format_pipeline, infer_program_effects,
    parse_program, run_tests,
};
use clap::Parser;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::PathBuf;
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
    #[arg(long = "plugin")]
    plugins: Vec<String>,
}

#[derive(Parser)]
#[command(name = "agl repl", about = "Interactive AGL session")]
struct ReplCli {
    #[arg(long, default_value = "mock", value_parser = ["mock", "live", "openai", "anthropic"])]
    adapter: String,
    #[arg(long)]
    trace_live: bool,
    #[arg(long = "plugin")]
    plugins: Vec<String>,
}

fn main() {
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
            "load" => match load_program(rest.trim()) {
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
                    let registry = build_registry(program, mode, args.trace_live, &args.plugins)?;
                    Ok(execute_pipeline(
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

fn load_program(path: &str) -> Result<agl::ast::Program, Box<dyn std::error::Error>> {
    if path.is_empty() {
        return Err("usage: load <path>".into());
    }
    let source = std::fs::read_to_string(path)?;
    let program = parse_program(&source)
        .map_err(|error| RenderedDiagnostic(render_diagnostic(path, &source, &error)))?;
    check_program(&program)
        .map_err(|error| RenderedDiagnostic(render_diagnostic(path, &source, &error)))?;
    Ok(program)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let source = std::fs::read_to_string(&cli.source)?;
    let program = parse_program(&source).map_err(|error| {
        RenderedDiagnostic(render_diagnostic(cli.source.display(), &source, &error))
    })?;
    check_program(&program).map_err(|error| {
        RenderedDiagnostic(render_diagnostic(cli.source.display(), &source, &error))
    })?;
    for warning in analyze_program(&program) {
        eprintln!(
            "{}",
            render_diagnostic(cli.source.display(), &source, &warning)
        );
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
        &cli.plugins,
    )?;
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

fn build_registry(
    program: &agl::ast::Program,
    mode: AdapterMode,
    trace_live: bool,
    plugins: &[String],
) -> Result<agl::Registry, Box<dyn std::error::Error>> {
    let mut tools = default_tool_registry(Duration::from_secs(15))?;
    let mut plugin_tasks = agl::Registry::default();
    for plugin in plugins {
        load_python_plugin_with_tools(&mut plugin_tasks, Some(&mut tools), plugin.clone())?;
    }
    let mut registry = registry_for_with_tools(program, mode, trace_live, tools)?;
    registry.extend(plugin_tasks);
    Ok(registry)
}
