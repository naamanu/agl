use crate::ast::Span;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: Kind,
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    Id,
    String,
    Number,
    Symbol,
    Keyword,
    Eof,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("[AGL0001] {message} at {line}:{col}")]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl LexError {
    pub const fn code(&self) -> &'static str {
        "AGL0001"
    }
}

const KEYWORDS: &[&str] = &[
    "language",
    "agent",
    "tool",
    "task",
    "pipeline",
    "workflow",
    "stage",
    "does",
    "review",
    "checks",
    "revise",
    "using",
    "max_rounds",
    "max_concurrency",
    "model",
    "tools",
    "let",
    "run",
    "with",
    "by",
    "parallel",
    "join",
    "return",
    "if",
    "while",
    "else",
    "break",
    "continue",
    "null",
    "retries",
    "on_fail",
    "abort",
    "use",
    "timeout",
    "true",
    "false",
    "type",
    "enum",
    "try",
    "catch",
    "assert",
    "test",
];

pub fn lex(source: &str) -> Result<Vec<Token>, LexError> {
    let chars: Vec<char> = source.chars().collect();
    let (mut i, mut line, mut col) = (0, 1, 1);
    let mut out = Vec::new();
    while i < chars.len() {
        let ch = chars[i];
        if ch.is_whitespace() {
            advance(ch, &mut line, &mut col);
            i += 1;
            continue;
        }
        if ch == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
                col += 1;
            }
            continue;
        }
        let span = Span { line, col };
        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
                col += 1;
            }
            let text: String = chars[start..i].iter().collect();
            let kind = if KEYWORDS.contains(&text.as_str()) {
                Kind::Keyword
            } else {
                Kind::Id
            };
            out.push(Token { kind, text, span });
            continue;
        }
        if ch.is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
                col += 1;
            }
            if chars.get(i) == Some(&'.') && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit()) {
                i += 1;
                col += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                    col += 1;
                }
            }
            out.push(Token {
                kind: Kind::Number,
                text: chars[start..i].iter().collect(),
                span,
            });
            continue;
        }
        if ch == '"' {
            i += 1;
            col += 1;
            let mut value = String::new();
            let mut closed = false;
            while i < chars.len() {
                let c = chars[i];
                if c == '"' {
                    i += 1;
                    col += 1;
                    closed = true;
                    break;
                }
                if c == '\n' {
                    return Err(err("newline in string literal", line, col));
                }
                if c != '\\' {
                    value.push(c);
                    i += 1;
                    col += 1;
                    continue;
                }
                i += 1;
                col += 1;
                let esc = *chars
                    .get(i)
                    .ok_or_else(|| err("unterminated escape sequence", line, col))?;
                match esc {
                    'n' => value.push('\n'),
                    't' => value.push('\t'),
                    'r' => value.push('\r'),
                    '\\' => value.push('\\'),
                    '"' => value.push('"'),
                    '\'' => value.push('\''),
                    '0' => value.push('\0'),
                    'u' | 'U' => {
                        let n = if esc == 'u' { 4 } else { 8 };
                        let start = i + 1;
                        let end = start + n;
                        if end > chars.len() {
                            return Err(err("incomplete unicode escape", line, col));
                        }
                        let hex: String = chars[start..end].iter().collect();
                        let cp = u32::from_str_radix(&hex, 16)
                            .map_err(|_| err("invalid unicode escape", line, col))?;
                        value.push(
                            char::from_u32(cp)
                                .ok_or_else(|| err("invalid unicode code point", line, col))?,
                        );
                        i = end;
                        col += n + 1;
                        continue;
                    }
                    _ => return Err(err(&format!("unknown escape sequence \\{esc}"), line, col)),
                }
                i += 1;
                col += 1;
            }
            if !closed {
                return Err(err("unterminated string literal", span.line, span.col));
            }
            out.push(Token {
                kind: Kind::String,
                text: value,
                span,
            });
            continue;
        }
        let two = chars.get(i + 1).map(|b| format!("{ch}{b}"));
        if matches!(two.as_deref(), Some("->" | "==" | "!=")) {
            out.push(Token {
                kind: Kind::Symbol,
                text: two.unwrap(),
                span,
            });
            i += 2;
            col += 2;
            continue;
        }
        if "{}()[]:,;=+.".contains(ch) {
            out.push(Token {
                kind: Kind::Symbol,
                text: ch.to_string(),
                span,
            });
            i += 1;
            col += 1;
            continue;
        }
        return Err(err(&format!("unexpected character {ch:?}"), line, col));
    }
    out.push(Token {
        kind: Kind::Eof,
        text: String::new(),
        span: Span { line, col },
    });
    Ok(out)
}

fn advance(ch: char, line: &mut usize, col: &mut usize) {
    if ch == '\n' {
        *line += 1;
        *col = 1
    } else {
        *col += 1
    }
}
fn err(message: &str, line: usize, col: usize) -> LexError {
    LexError {
        message: message.into(),
        line,
        col,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comments_and_unicode_strings() {
        let t = lex("-- hi\ntask x() -> String; \"hi\\u0021\"").unwrap();
        assert_eq!(t[0].text, "task");
        assert_eq!(t[7].text, "hi!");
    }
    #[test]
    fn reports_bad_escape() {
        assert!(
            lex("\"\\q\"")
                .unwrap_err()
                .to_string()
                .contains("unknown escape")
        );
    }
}
