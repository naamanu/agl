//! Stable compiler diagnostic codes and source rendering.

use crate::ast::Span;
use crate::{AnalysisWarning, CheckError, ExecutionError, ParseError};
use std::fmt::{self, Display};

pub trait AglDiagnostic {
    fn code(&self) -> &'static str;
    fn summary(&self) -> String;
    fn span(&self) -> Option<Span>;
    fn severity(&self) -> &'static str {
        "error"
    }
}

impl AglDiagnostic for ParseError {
    fn code(&self) -> &'static str {
        self.code()
    }

    fn summary(&self) -> String {
        match self {
            Self::Lex(error) => error.message.clone(),
            Self::Syntax { message, .. } => message.clone(),
            Self::Semantic(message) => message.clone(),
            Self::UnsupportedVersion {
                version, supported, ..
            } => format!(
                "unsupported language version {version:?}; supported versions are {supported}"
            ),
        }
    }

    fn span(&self) -> Option<Span> {
        match self {
            Self::Lex(error) => Some(Span {
                line: error.line,
                col: error.col,
            }),
            Self::Syntax { line, col, .. } | Self::UnsupportedVersion { line, col, .. } => {
                Some(Span {
                    line: *line,
                    col: *col,
                })
            }
            Self::Semantic(_) => None,
        }
    }
}

impl AglDiagnostic for CheckError {
    fn code(&self) -> &'static str {
        self.code()
    }

    fn summary(&self) -> String {
        self.message.clone()
    }

    fn span(&self) -> Option<Span> {
        Some(Span {
            line: self.line,
            col: self.col,
        })
    }
}

impl AglDiagnostic for ExecutionError {
    fn code(&self) -> &'static str {
        self.code()
    }

    fn summary(&self) -> String {
        self.message.clone()
    }

    fn span(&self) -> Option<Span> {
        None
    }
}

impl AglDiagnostic for AnalysisWarning {
    fn code(&self) -> &'static str {
        self.code
    }

    fn summary(&self) -> String {
        self.message.clone()
    }

    fn span(&self) -> Option<Span> {
        Some(Span {
            line: self.line,
            col: self.col,
        })
    }

    fn severity(&self) -> &'static str {
        "warning"
    }
}

pub fn render_diagnostic(
    filename: impl Display,
    source: &str,
    diagnostic: &impl AglDiagnostic,
) -> String {
    let filename = filename.to_string();
    let Some(span) = diagnostic.span() else {
        return format!(
            "[{}] {filename}\n{}: {}",
            diagnostic.code(),
            diagnostic.severity(),
            diagnostic.summary()
        );
    };
    let source_line = source
        .lines()
        .nth(span.line.saturating_sub(1))
        .unwrap_or("");
    let gutter = span.line.to_string().len();
    let caret_padding = " ".repeat(span.col.saturating_sub(1));
    format!(
        "[{}] {filename}:{}:{}\n{}: {}\n{:gutter$} |\n{} | {}\n{:gutter$} | {}^",
        diagnostic.code(),
        span.line,
        span.col,
        diagnostic.severity(),
        diagnostic.summary(),
        "",
        span.line,
        source_line,
        "",
        caret_padding,
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedDiagnostic(pub String);

impl Display for RenderedDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RenderedDiagnostic {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_program;

    #[test]
    fn renders_a_stable_source_diagnostic() {
        let source = "pipeline main() -> String {\n  return 7;\n}\n";
        let program = parse_program(source).unwrap();
        let error = crate::check_program(&program).unwrap_err();
        assert_eq!(
            render_diagnostic("example.agent", source, &error),
            "[AGL2001] example.agent:2:10\nerror: type mismatch: expected String, got Number\n  |\n2 |   return 7;\n  |          ^"
        );
    }
}
