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


_SIMPLE_ESCAPES: dict[str, str] = {
    "n": "\n",
    "t": "\t",
    "r": "\r",
    "\\": "\\",
    '"': '"',
    "'": "'",
    "0": "\0",
}


def _decode_string(token: str) -> str:
    body = token[1:-1]
    result: list[str] = []
    i = 0
    while i < len(body):
        ch = body[i]
        if ch != "\\":
            result.append(ch)
            i += 1
            continue

        i += 1
        if i >= len(body):
            raise LexError("Unterminated escape sequence in string literal.")

        esc = body[i]
        if esc in _SIMPLE_ESCAPES:
            result.append(_SIMPLE_ESCAPES[esc])
            i += 1
            continue

        if esc == "u":
            if i + 4 >= len(body):
                raise LexError("Incomplete \\uXXXX escape sequence.")
            hex_digits = body[i + 1 : i + 5]
            if not all(c in "0123456789abcdefABCDEF" for c in hex_digits):
                raise LexError(f"Invalid \\uXXXX escape: \\u{hex_digits}")
            result.append(_decode_code_point(hex_digits, "\\uXXXX"))
            i += 5
            continue

        if esc == "U":
            if i + 8 >= len(body):
                raise LexError("Incomplete \\UXXXXXXXX escape sequence.")
            hex_digits = body[i + 1 : i + 9]
            if not all(c in "0123456789abcdefABCDEF" for c in hex_digits):
                raise LexError(f"Invalid \\UXXXXXXXX escape: \\U{hex_digits}")
            result.append(_decode_code_point(hex_digits, "\\UXXXXXXXX"))
            i += 9
            continue

        raise LexError(f"Unknown escape sequence: \\{esc}")

    return "".join(result)


def _decode_code_point(hex_digits: str, form: str) -> str:
    value = int(hex_digits, 16)
    if value > 0x10FFFF or 0xD800 <= value <= 0xDFFF:
        raise LexError(f"Invalid {form} escape: out-of-range code point U+{hex_digits.upper()}")
    return chr(value)


def _advance_position(text: str, line: int, col: int) -> tuple[int, int]:
    parts = text.split("\n")
    if len(parts) == 1:
        return line, col + len(text)
    return line + (len(parts) - 1), len(parts[-1]) + 1
