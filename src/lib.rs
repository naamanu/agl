//! AGL is a small, typed language for composing agentic applications.
//!
//! The crate exposes each compiler phase separately so applications can parse,
//! check, inspect, and execute AGL without shelling out to the CLI.

pub mod ast;
pub mod checker;
pub mod context;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod stdlib;

pub use checker::{CheckError, check_program};
pub use parser::{ParseError, parse_program};
pub use runtime::{ExecutionError, Registry, execute_pipeline, run_tests};
