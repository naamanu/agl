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
pub mod formatter;
pub mod lexer;
pub mod parser;
pub mod plugins;
pub mod runtime;
pub mod stdlib;

pub use checker::{
    AnalysisWarning, CheckError, analyze_program, check_program, infer_program_effects,
};
pub use formatter::format_pipeline;
pub use parser::{ParseError, parse_program};
pub use runtime::{ExecutionError, Registry, execute_pipeline, run_tests};
