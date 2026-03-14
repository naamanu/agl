from __future__ import annotations

import unittest

from agentlang.lexer import LexError, lex


class LexerTests(unittest.TestCase):
    def test_simple_tokens(self) -> None:
        tokens = lex('let x = 42;')
        kinds = [t.kind for t in tokens]
        self.assertEqual(kinds, ["LET", "ID", "EQUAL", "NUMBER", "SEMI", "EOF"])

    def test_string_token(self) -> None:
        tokens = lex('"hello world"')
        self.assertEqual(tokens[0].kind, "STRING")
        self.assertEqual(tokens[0].value, "hello world")

    def test_string_escape_sequences(self) -> None:
        tokens = lex(r'"line\nbreak\ttab"')
        self.assertEqual(tokens[0].value, "line\nbreak\ttab")

    def test_keywords_recognized(self) -> None:
        src = "agent task pipeline workflow let run with by return if else while break continue parallel join"
        tokens = lex(src)
        expected = [
            "AGENT", "TASK", "PIPELINE", "WORKFLOW", "LET", "RUN",
            "WITH", "BY", "RETURN", "IF", "ELSE", "WHILE", "BREAK",
            "CONTINUE", "PARALLEL", "JOIN", "EOF",
        ]
        self.assertEqual([t.kind for t in tokens], expected)

    def test_timeout_keyword(self) -> None:
        tokens = lex("timeout")
        self.assertEqual(tokens[0].kind, "TIMEOUT")

    def test_operators(self) -> None:
        tokens = lex("== != + -> = .")
        kinds = [t.kind for t in tokens[:-1]]
        self.assertEqual(kinds, ["EQEQ", "NEQ", "PLUS", "ARROW", "EQUAL", "DOT"])

    def test_number_tokens(self) -> None:
        tokens = lex("42 3.14")
        self.assertEqual(tokens[0].value, "42")
        self.assertEqual(tokens[1].value, "3.14")

    def test_comments_are_skipped(self) -> None:
        tokens = lex("let x -- this is a comment\nlet y")
        kinds = [t.kind for t in tokens]
        self.assertEqual(kinds, ["LET", "ID", "LET", "ID", "EOF"])

    def test_line_and_col_tracking(self) -> None:
        tokens = lex("let\n  x")
        # 'let' at 1:1, 'x' at 2:3
        self.assertEqual(tokens[0].line, 1)
        self.assertEqual(tokens[0].col, 1)
        self.assertEqual(tokens[1].line, 2)
        self.assertEqual(tokens[1].col, 3)

    def test_unterminated_string_raises_lex_error(self) -> None:
        with self.assertRaises(LexError):
            lex('"unterminated')

    def test_invalid_escape_raises_lex_error(self) -> None:
        with self.assertRaises(LexError):
            lex(r'"bad\z"')

    def test_unexpected_character_raises_lex_error(self) -> None:
        with self.assertRaises(LexError):
            lex("let x = @;")

    def test_true_false_null(self) -> None:
        tokens = lex("true false null")
        kinds = [t.kind for t in tokens[:-1]]
        self.assertEqual(kinds, ["TRUE", "FALSE", "NULL"])

    def test_braces_brackets_parens(self) -> None:
        tokens = lex("{}[]()")
        kinds = [t.kind for t in tokens[:-1]]
        self.assertEqual(kinds, ["LBRACE", "RBRACE", "LBRACKET", "RBRACKET", "LPAREN", "RPAREN"])

    def test_unicode_escape(self) -> None:
        tokens = lex(r'"\u0041"')
        self.assertEqual(tokens[0].value, "A")

    def test_empty_source(self) -> None:
        tokens = lex("")
        self.assertEqual(len(tokens), 1)
        self.assertEqual(tokens[0].kind, "EOF")


if __name__ == "__main__":
    unittest.main()
