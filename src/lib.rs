//! AGL is a small, typed language for composing agentic applications.
//!
//! The crate exposes each compiler phase separately so applications can parse,
//! check, inspect, and execute AGL without shelling out to the CLI.

pub mod adapters;
pub mod ast;
pub mod checker;
pub mod context;
pub mod deployment;
pub mod diagnostic;
pub mod documentation;
pub mod evaluation;
pub mod event_store;
pub mod extension;
pub mod formatter;
pub mod lexer;
pub mod modules;
pub mod package;
pub mod parser;
pub mod policy;
pub mod runtime;
pub mod stdlib;
pub mod tooling;

pub use checker::{
    AnalysisWarning, CheckError, analyze_program, check_program, infer_program_effects,
};
pub use formatter::{format_pipeline, format_program};
pub use parser::{ParseError, parse_program};
pub use runtime::{
    CancellationToken, ExecutionError, HandlerFailure, Invocation, Registry, TaskOutput, Usage,
    execute_pipeline, execute_pipeline_async, run_tests,
};
