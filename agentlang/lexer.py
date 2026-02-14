from __future__ import annotations

import re
from dataclasses import dataclass


@dataclass(frozen=True)
class Token:
    kind: str
    value: str
    line: int
    col: int


TOKEN_RE = re.compile(
    r"""
    (?P<WS>[ \t\r\n]+)
    |(?P<COMMENT>//[^\n]*)
    |(?P<HASHCOMMENT>\#[^\n]*)
    |(?P<ARROW>->)
    |(?P<EQEQ>==)
    |(?P<NEQ>!=)
    |(?P<STRING>"(?:\\.|[^"\\])*")
    |(?P<NUMBER>\d+(?:\.\d+)?)
    |(?P<LBRACE>\{)
    |(?P<RBRACE>\})
    |(?P<LPAREN>\()
    |(?P<RPAREN>\))
    |(?P<LBRACKET>\[)
    |(?P<RBRACKET>\])
    |(?P<COLON>:)
    |(?P<COMMA>,)
    |(?P<SEMI>;)
    |(?P<EQUAL>=)
    |(?P<PLUS>\+)
    |(?P<DOT>\.)
    |(?P<ID>[A-Za-z_][A-Za-z0-9_]*)
    """,
    re.VERBOSE,
)


KEYWORDS = {
    "agent",
    "task",
    "pipeline",
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
    "else",
    "retries",
    "on_fail",
    "abort",
    "use",
}


class LexError(ValueError):
    pass


def lex(source: str) -> list[Token]:
    tokens: list[Token] = []
    i = 0
    line = 1
    col = 1

    while i < len(source):
        match = TOKEN_RE.match(source, i)
        if not match:
            snippet = source[i : i + 30]
            raise LexError(f"Unexpected token at {line}:{col}: {snippet!r}")

        kind = match.lastgroup
        assert kind is not None
        text = match.group()

        if kind in {"WS", "COMMENT", "HASHCOMMENT"}:
            pass
        elif kind == "ID" and text in KEYWORDS:
            tokens.append(Token(kind=text.upper(), value=text, line=line, col=col))
        elif kind == "STRING":
            tokens.append(Token(kind="STRING", value=_decode_string(text), line=line, col=col))
        elif kind == "NUMBER":
            tokens.append(Token(kind="NUMBER", value=text, line=line, col=col))
        else:
            tokens.append(Token(kind=kind, value=text, line=line, col=col))

        line, col = _advance_position(text, line, col)
        i = match.end()

    tokens.append(Token(kind="EOF", value="", line=line, col=col))
    return tokens


def _decode_string(token: str) -> str:
    body = token[1:-1]
    return bytes(body, "utf-8").decode("unicode_escape")


def _advance_position(text: str, line: int, col: int) -> tuple[int, int]:
    parts = text.split("\n")
    if len(parts) == 1:
        return line, col + len(text)
    return line + (len(parts) - 1), len(parts[-1]) + 1
