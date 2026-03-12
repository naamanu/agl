from __future__ import annotations

from dataclasses import dataclass

from .ast import (
    AgentDef,
    BinaryExpr,
    BreakStmt,
    ContinueStmt,
    Expr,
    IfLetStmt,
    IfStmt,
    ListExpr,
    ListType,
    LiteralExpr,
    ObjExpr,
    ObjType,
    OptionType,
    ParallelStmt,
    Param,
    PipelineDef,
    PrimitiveType,
    Program,
    RefExpr,
    ReturnStmt,
    RunStmt,
    Stmt,
    TaskDef,
    ToolDef,
    TypeExpr,
    WhileStmt,
)
from .lexer import Token, lex


class ParseError(ValueError):
    pass


@dataclass
class Parser:
    tokens: list[Token]
    pos: int = 0

    def current(self) -> Token:
        return self.tokens[self.pos]

    def advance(self) -> Token:
        token = self.current()
        self.pos += 1
        return token

    def expect(self, kind: str) -> Token:
        token = self.current()
        if token.kind != kind:
            raise ParseError(
                f"Expected {kind}, got {token.kind} at {token.line}:{token.col}"
            )
        self.pos += 1
        return token

    def match(self, kind: str) -> bool:
        if self.current().kind == kind:
            self.pos += 1
            return True
        return False

    def parse_program(self) -> Program:
        agents: dict[str, AgentDef] = {}
        tools: dict[str, ToolDef] = {}
        tasks: dict[str, TaskDef] = {}
        pipelines: dict[str, PipelineDef] = {}

        while self.current().kind != "EOF":
            token_kind = self.current().kind
            if token_kind == "AGENT":
                agent = self.parse_agent()
                if agent.name in agents:
                    raise ParseError(f"Duplicate agent: {agent.name}")
                agents[agent.name] = agent
            elif token_kind == "TOOL":
                tool = self.parse_tool()
                if tool.name in tools:
                    raise ParseError(f"Duplicate tool: {tool.name}")
                tools[tool.name] = tool
            elif token_kind == "TASK":
                task = self.parse_task()
                if task.name in tasks:
                    raise ParseError(f"Duplicate task: {task.name}")
                tasks[task.name] = task
            elif token_kind == "PIPELINE":
                pipeline = self.parse_pipeline()
                if pipeline.name in pipelines:
                    raise ParseError(f"Duplicate pipeline: {pipeline.name}")
                pipelines[pipeline.name] = pipeline
            else:
                token = self.current()
                raise ParseError(
                    f"Unexpected token {token.kind} at {token.line}:{token.col}"
                )

        return Program(agents=agents, tools=tools, tasks=tasks, pipelines=pipelines)

    def parse_agent(self) -> AgentDef:
        self.expect("AGENT")
        name = self.expect("ID").value
        self.expect("LBRACE")
        self.expect("MODEL")
        self.expect("COLON")
        model = self.expect("STRING").value
        self.expect("COMMA")
        self.expect("TOOLS")
        self.expect("COLON")
        tools = self.parse_tool_list()
        self.expect("RBRACE")
        return AgentDef(name=name, model=model, tools=tools)

    def parse_tool_list(self) -> list[str]:
        self.expect("LBRACKET")
        tools: list[str] = []
        seen: set[str] = set()
        if self.current().kind != "RBRACKET":
            first = self.expect("ID").value
            tools.append(first)
            seen.add(first)
            while self.match("COMMA"):
                tool = self.expect("ID").value
                if tool in seen:
                    raise ParseError(f"Duplicate tool '{tool}' in tools list.")
                tools.append(tool)
                seen.add(tool)
        self.expect("RBRACKET")
        return tools

    def parse_task(self) -> TaskDef:
        self.expect("TASK")
        name = self.expect("ID").value
        self.expect("LPAREN")
        params = self.parse_params()
        self.expect("RPAREN")
        self.expect("ARROW")
        return_type = self.parse_type()
        execution_mode = "handler"
        if self.match("BY"):
            self.expect("AGENT")
            execution_mode = "agent"
        self.expect("LBRACE")
        self.expect("RBRACE")
        return TaskDef(
            name=name,
            params=params,
            return_type=return_type,
            execution_mode=execution_mode,
        )

    def parse_tool(self) -> ToolDef:
        self.expect("TOOL")
        name = self.expect("ID").value
        self.expect("LPAREN")
        params = self.parse_params()
        self.expect("RPAREN")
        self.expect("ARROW")
        return_type = self.parse_type()
        self.expect("LBRACE")
        self.expect("RBRACE")
        return ToolDef(name=name, params=params, return_type=return_type)

    def parse_pipeline(self) -> PipelineDef:
        self.expect("PIPELINE")
        name = self.expect("ID").value
        self.expect("LPAREN")
        params = self.parse_params()
        self.expect("RPAREN")
        self.expect("ARROW")
        return_type = self.parse_type()
        statements = self.parse_block()
        return PipelineDef(
            name=name,
            params=params,
            return_type=return_type,
            statements=statements,
        )

    def parse_params(self) -> list[Param]:
        params: list[Param] = []
        seen: set[str] = set()
        if self.current().kind == "RPAREN":
            return params
        while True:
            name = self.expect("ID").value
            if name in seen:
                raise ParseError(f"Duplicate parameter '{name}'.")
            self.expect("COLON")
            type_expr = self.parse_type()
            params.append(Param(name=name, type_expr=type_expr))
            seen.add(name)
            if not self.match("COMMA"):
                break
        return params

    def parse_type(self) -> TypeExpr:
        token = self.current()
        if token.kind == "ID" and token.value in {"String", "Number", "Bool"}:
            self.advance()
            return PrimitiveType(token.value)

        if token.kind == "ID" and token.value == "Obj":
            self.advance()
            self.expect("LBRACE")
            fields: dict[str, TypeExpr] = {}
            if self.current().kind != "RBRACE":
                while True:
                    field_name = self.expect("ID").value
                    if field_name in fields:
                        raise ParseError(f"Duplicate object type field '{field_name}'.")
                    self.expect("COLON")
                    fields[field_name] = self.parse_type()
                    if not self.match("COMMA"):
                        break
            self.expect("RBRACE")
            return ObjType(fields=fields)

        if token.kind == "ID" and token.value == "List":
            self.advance()
            self.expect("LBRACKET")
            item_type = self.parse_type()
            self.expect("RBRACKET")
            return ListType(item_type=item_type)

        if token.kind == "ID" and token.value == "Option":
            self.advance()
            self.expect("LBRACKET")
            item_type = self.parse_type()
            self.expect("RBRACKET")
            return OptionType(item_type=item_type)

        raise ParseError(f"Invalid type at {token.line}:{token.col}: {token.kind}")

    def parse_block(self) -> list[Stmt]:
        self.expect("LBRACE")
        statements: list[Stmt] = []
        while self.current().kind != "RBRACE":
            statements.append(self.parse_stmt())
        self.expect("RBRACE")
        return statements

    def parse_stmt(self) -> Stmt:
        token = self.current()

        if token.kind == "LET":
            stmt = self.parse_run_stmt()
            self.expect("SEMI")
            return stmt

        if token.kind == "PARALLEL":
            return self.parse_parallel_stmt()

        if token.kind == "IF":
            return self.parse_if_stmt()

        if token.kind == "WHILE":
            return self.parse_while_stmt()

        if token.kind == "RETURN":
            self.advance()
            expr = self.parse_expr()
            self.expect("SEMI")
            return ReturnStmt(expr=expr)

        if token.kind == "BREAK":
            self.advance()
            self.expect("SEMI")
            return BreakStmt()

        if token.kind == "CONTINUE":
            self.advance()
            self.expect("SEMI")
            return ContinueStmt()

        raise ParseError(
            f"Unexpected statement token: {token.kind} at {token.line}:{token.col}"
        )

    def parse_if_stmt(self) -> Stmt:
        self.expect("IF")
        if self.match("LET"):
            binding = self.expect("ID").value
            self.expect("EQUAL")
            option_expr = self.parse_expr()
            then_statements = self.parse_block()
            else_statements: list[Stmt] | None = None
            if self.match("ELSE"):
                else_statements = self.parse_block()
            return IfLetStmt(
                binding=binding,
                option_expr=option_expr,
                then_statements=then_statements,
                else_statements=else_statements,
            )

        condition = self.parse_expr()
        then_statements = self.parse_block()
        else_statements: list[Stmt] | None = None
        if self.match("ELSE"):
            else_statements = self.parse_block()
        return IfStmt(
            condition=condition,
            then_statements=then_statements,
            else_statements=else_statements,
        )

    def parse_while_stmt(self) -> WhileStmt:
        self.expect("WHILE")
        condition = self.parse_expr()
        statements = self.parse_block()
        return WhileStmt(condition=condition, statements=statements)

    def parse_parallel_stmt(self) -> ParallelStmt:
        self.expect("PARALLEL")
        self.expect("LBRACE")
        branches: list[RunStmt] = []
        while self.current().kind != "RBRACE":
            branch = self.parse_run_stmt()
            self.expect("SEMI")
            branches.append(branch)
        self.expect("RBRACE")
        self.expect("JOIN")
        self.expect("SEMI")
        return ParallelStmt(branches=branches)

    def parse_run_stmt(self) -> RunStmt:
        self.expect("LET")
        target = self.expect("ID").value
        self.expect("EQUAL")
        self.expect("RUN")
        task_name = self.expect("ID").value
        self.expect("WITH")
        args = self.parse_arg_map()

        agent_name: str | None = None
        retries = 0
        on_fail = "abort"
        fallback_expr: Expr | None = None

        saw_by = False
        saw_retries = False
        saw_on_fail = False
        while True:
            if self.match("BY"):
                if saw_by:
                    raise ParseError("Duplicate 'by' clause in run statement.")
                agent_name = self.expect("ID").value
                saw_by = True
                continue

            if self.match("RETRIES"):
                if saw_retries:
                    raise ParseError("Duplicate 'retries' clause in run statement.")
                retries_token = self.expect("NUMBER")
                if "." in retries_token.value:
                    raise ParseError("Retries must be an integer.")
                retries = int(retries_token.value)
                if retries < 0:
                    raise ParseError("Retries cannot be negative.")
                saw_retries = True
                continue

            if self.match("ON_FAIL"):
                if saw_on_fail:
                    raise ParseError("Duplicate 'on_fail' clause in run statement.")
                if self.match("ABORT"):
                    on_fail = "abort"
                elif self.match("USE"):
                    on_fail = "use"
                    fallback_expr = self.parse_expr()
                else:
                    token = self.current()
                    raise ParseError(
                        f"Expected 'abort' or 'use' after on_fail at {token.line}:{token.col}"
                    )
                saw_on_fail = True
                continue

            break

        return RunStmt(
            target=target,
            task_name=task_name,
            args=args,
            agent_name=agent_name,
            retries=retries,
            on_fail=on_fail,
            fallback_expr=fallback_expr,
        )

    def parse_arg_map(self) -> dict[str, Expr]:
        self.expect("LBRACE")
        args: dict[str, Expr] = {}
        if self.current().kind != "RBRACE":
            while True:
                key = self.expect("ID").value
                if key in args:
                    raise ParseError(f"Duplicate argument '{key}' in run call.")
                self.expect("COLON")
                args[key] = self.parse_expr()
                if not self.match("COMMA"):
                    break
        self.expect("RBRACE")
        return args

    def parse_expr(self) -> Expr:
        return self.parse_equality()

    def parse_equality(self) -> Expr:
        expr = self.parse_addition()
        while True:
            if self.match("EQEQ"):
                right = self.parse_addition()
                expr = BinaryExpr(op="==", left=expr, right=right)
                continue
            if self.match("NEQ"):
                right = self.parse_addition()
                expr = BinaryExpr(op="!=", left=expr, right=right)
                continue
            return expr

    def parse_addition(self) -> Expr:
        expr = self.parse_primary()
        while self.match("PLUS"):
            right = self.parse_primary()
            expr = BinaryExpr(op="+", left=expr, right=right)
        return expr

    def parse_primary(self) -> Expr:
        token = self.current()

        if token.kind == "STRING":
            self.advance()
            return LiteralExpr(value=token.value)

        if token.kind == "NUMBER":
            self.advance()
            if "." in token.value:
                return LiteralExpr(value=float(token.value))
            return LiteralExpr(value=int(token.value))

        if token.kind == "LPAREN":
            self.advance()
            expr = self.parse_expr()
            self.expect("RPAREN")
            return expr

        if token.kind == "LBRACE":
            return self.parse_object_literal()

        if token.kind == "LBRACKET":
            return self.parse_list_literal()

        if token.kind == "TRUE":
            self.advance()
            return LiteralExpr(value=True)

        if token.kind == "FALSE":
            self.advance()
            return LiteralExpr(value=False)

        if token.kind == "NULL":
            self.advance()
            return LiteralExpr(value=None)

        if token.kind == "ID":
            parts = [self.advance().value]
            while self.match("DOT"):
                parts.append(self.expect("ID").value)
            return RefExpr(parts=parts)

        raise ParseError(
            f"Invalid expression token {token.kind} at {token.line}:{token.col}"
        )

    def parse_object_literal(self) -> ObjExpr:
        self.expect("LBRACE")
        fields: dict[str, Expr] = {}
        if self.current().kind != "RBRACE":
            while True:
                key = self.expect("ID").value
                if key in fields:
                    raise ParseError(f"Duplicate object literal field '{key}'.")
                self.expect("COLON")
                fields[key] = self.parse_expr()
                if not self.match("COMMA"):
                    break
        self.expect("RBRACE")
        return ObjExpr(fields=fields)

    def parse_list_literal(self) -> ListExpr:
        self.expect("LBRACKET")
        items: list[Expr] = []
        if self.current().kind != "RBRACKET":
            while True:
                items.append(self.parse_expr())
                if not self.match("COMMA"):
                    break
        self.expect("RBRACKET")
        return ListExpr(items=items)


def parse_program(source: str) -> Program:
    parser = Parser(tokens=lex(source))
    return parser.parse_program()
