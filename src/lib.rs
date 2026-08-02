//! AGL is a small, typed language for composing agentic applications.
//!
//! The crate exposes each compiler phase separately so applications can parse,
//! check, inspect, and execute AGL without shelling out to the CLI.

pub mod adapters;
pub mod ast;
pub mod checker;
pub mod context;
pub mod formatter;
pub mod lexer;
pub mod parser;
pub mod plugins;
pub mod runtime;
pub mod stdlib;

pub use checker::{CheckError, check_program};
pub use formatter::format_pipeline;
pub use parser::{ParseError, parse_program};
pub use runtime::{ExecutionError, Registry, execute_pipeline, run_tests};
