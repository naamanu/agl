from __future__ import annotations

from dataclasses import dataclass

from .ast import (
    AgentDef,
    AssertStmt,
    BinaryExpr,
    BreakStmt,
    ContinueStmt,
    EnumType,
    EnumTypeDef,
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
    Span,
    Stmt,
    TaskDef,
    TestBlockDef,
    ToolDef,
    TryCatchStmt,
    TypeAliasDef,
    TypeExpr,
    WorkflowDef,
    WorkflowReturnStep,
    WorkflowReviewStep,
    WorkflowStageStep,
    WorkflowStep,
    WhileStmt,
)
from .lexer import Token, lex


class ParseError(ValueError):
    pass


@dataclass
class Parser:
    tokens: list[Token]
    pos: int = 0
    # Registries built during parsing for resolving type aliases and enums.
    _type_aliases: dict[str, TypeAliasDef] | None = None
    _enum_types: dict[str, EnumTypeDef] | None = None

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

    def _span(self) -> Span:
        tok = self.current()
        return Span(tok.line, tok.col)

    def parse_program(self) -> Program:
        agents: dict[str, AgentDef] = {}
        tools: dict[str, ToolDef] = {}
        tasks: dict[str, TaskDef] = {}
        pipelines: dict[str, PipelineDef] = {}
        workflows: dict[str, WorkflowDef] = {}
        type_aliases: dict[str, TypeAliasDef] = {}
        enum_types: dict[str, EnumTypeDef] = {}
        test_blocks: list[TestBlockDef] = []

        # Make alias/enum registries available during parsing for type resolution.
        self._type_aliases = type_aliases
        self._enum_types = enum_types

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
            elif token_kind == "WORKFLOW":
                workflow = self.parse_workflow()
                if workflow.name in workflows or workflow.name in pipelines:
                    raise ParseError(f"Duplicate workflow: {workflow.name}")
                workflows[workflow.name] = workflow
            elif token_kind == "TYPE":
                alias = self.parse_type_alias()
                if alias.name in type_aliases:
                    raise ParseError(f"Duplicate type alias: {alias.name}")
                type_aliases[alias.name] = alias
            elif token_kind == "ENUM":
                enum = self.parse_enum_type()
                if enum.name in enum_types:
                    raise ParseError(f"Duplicate enum: {enum.name}")
                enum_types[enum.name] = enum
            elif token_kind == "TEST":
                test_block = self.parse_test_block()
                test_blocks.append(test_block)
            else:
                token = self.current()
                raise ParseError(
                    f"Unexpected token {token.kind} at {token.line}:{token.col}"
                )

        return Program(
            agents=agents,
            tools=tools,
            tasks=tasks,
            pipelines=pipelines,
            workflows=workflows,
            type_aliases=type_aliases,
            enum_types=enum_types,
            test_blocks=test_blocks,
        )

    def parse_type_alias(self) -> TypeAliasDef:
        self.expect("TYPE")
        name = self.expect("ID").value
        self.expect("EQUAL")
        type_expr = self.parse_type()
        self.expect("SEMI")
        return TypeAliasDef(name=name, type_expr=type_expr)

    def parse_enum_type(self) -> EnumTypeDef:
        self.expect("ENUM")
        name = self.expect("ID").value
        self.expect("LBRACE")
        variants: list[str] = []
        seen: set[str] = set()
        if self.current().kind != "RBRACE":
            first = self.expect("ID").value
            variants.append(first)
            seen.add(first)
            while self.match("COMMA"):
                if self.current().kind == "RBRACE":
                    break  # trailing comma
                variant = self.expect("ID").value
                if variant in seen:
                    raise ParseError(f"Duplicate enum variant '{variant}' in enum '{name}'.")
                variants.append(variant)
                seen.add(variant)
        self.expect("RBRACE")
        self.expect("SEMI")
        return EnumTypeDef(name=name, variants=tuple(variants))

    def parse_test_block(self) -> TestBlockDef:
        self.expect("TEST")
        name = self.expect("STRING").value
        statements = self.parse_block()
        return TestBlockDef(name=name, statements=tuple(statements))

    def parse_agent(self) -> AgentDef:
        self.expect("AGENT")
        name = self.expect("ID").value
        self.expect("LBRACE")

        model: str | None = None
        tools: list[str] = []
        saw_model = False
        saw_tools = False

        # Parse agent body - model is optional, tools required
        while self.current().kind != "RBRACE":
            if self.match("COMMA"):
                continue
            if self.current().kind == "MODEL":
                if saw_model:
                    raise ParseError("Duplicate 'model' in agent definition.")
                self.advance()
                self.expect("COLON")
                model = self.expect("STRING").value
                saw_model = True
            elif self.current().kind == "TOOLS":
                if saw_tools:
                    raise ParseError("Duplicate 'tools' in agent definition.")
                self.advance()
                self.expect("COLON")
                tools = self.parse_tool_list()
                saw_tools = True
            else:
                token = self.current()
                raise ParseError(
                    f"Unexpected token in agent body: {token.kind} at {token.line}:{token.col}"
                )

        if not saw_tools:
            raise ParseError(f"Agent '{name}' must declare tools.")

        self.expect("RBRACE")
        return AgentDef(name=name, model=model, tools=tuple(tools))

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
            params=tuple(params),
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
        return ToolDef(name=name, params=tuple(params), return_type=return_type)

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
            params=tuple(params),
            return_type=return_type,
            statements=tuple(statements),
        )

    def parse_workflow(self) -> WorkflowDef:
        self.expect("WORKFLOW")
        name = self.expect("ID").value
        self.expect("LPAREN")
        params = self.parse_params()
        self.expect("RPAREN")
        self.expect("ARROW")
        return_type = self.parse_type()
        steps = self.parse_workflow_block()
        return WorkflowDef(
            name=name,
            params=tuple(params),
            return_type=return_type,
            steps=tuple(steps),
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

        # Check for type alias or enum reference
        if token.kind == "ID":
            if self._type_aliases and token.value in self._type_aliases:
                self.advance()
                return self._type_aliases[token.value].type_expr
            if self._enum_types and token.value in self._enum_types:
                self.advance()
                return EnumType(name=token.value)

        raise ParseError(f"Invalid type at {token.line}:{token.col}: {token.kind}")

    def parse_block(self) -> list[Stmt]:
        self.expect("LBRACE")
        statements: list[Stmt] = []
        while self.current().kind != "RBRACE":
            statements.append(self.parse_stmt())
        self.expect("RBRACE")
        return statements

    def parse_workflow_block(self) -> list[WorkflowStep]:
        self.expect("LBRACE")
        steps: list[WorkflowStep] = []
        while self.current().kind != "RBRACE":
            steps.append(self.parse_workflow_step())
        self.expect("RBRACE")
        return steps

    def parse_workflow_step(self) -> WorkflowStep:
        token = self.current()

        if token.kind == "STAGE":
            self.advance()
            target = self.expect("ID").value
            self.expect("EQUAL")
            agent_name = self.expect("ID").value
            self.expect("DOES")
            task_name = self.expect("ID").value
            self.expect("LPAREN")
            args = self.parse_call_args()
            self.expect("RPAREN")
            self.expect("SEMI")
            return WorkflowStageStep(
                target=target,
                agent_name=agent_name,
                task_name=task_name,
                args=tuple(args),
            )

        if token.kind == "REVIEW":
            self.advance()
            target = self.expect("ID").value
            self.expect("EQUAL")
            reviewer_agent = self.expect("ID").value
            self.expect("CHECKS")
            source = self.expect("ID").value
            self.expect("REVISE")
            self.expect("WITH")
            reviser_agent = self.expect("ID").value
            self.expect("USING")
            revise_task_name = self.expect("ID").value
            self.expect("MAX_ROUNDS")
            rounds_token = self.expect("NUMBER")
            if "." in rounds_token.value:
                raise ParseError("max_rounds must be an integer.")
            max_rounds = int(rounds_token.value)
            if max_rounds < 0:
                raise ParseError("max_rounds cannot be negative.")
            self.expect("SEMI")
            return WorkflowReviewStep(
                target=target,
                reviewer_agent=reviewer_agent,
                source=source,
                reviser_agent=reviser_agent,
                revise_task_name=revise_task_name,
                max_rounds=max_rounds,
            )

        if token.kind == "RETURN":
            self.advance()
            expr = self.parse_expr()
            self.expect("SEMI")
            return WorkflowReturnStep(expr=expr)

        raise ParseError(
            f"Unexpected workflow step token: {token.kind} at {token.line}:{token.col}"
        )

    def parse_stmt(self) -> Stmt:
        token = self.current()
        span = self._span()

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

        if token.kind == "TRY":
            return self.parse_try_catch_stmt()

        if token.kind == "ASSERT":
            return self.parse_assert_stmt()

        if token.kind == "RETURN":
            self.advance()
            expr = self.parse_expr()
            self.expect("SEMI")
            return ReturnStmt(expr=expr, span=span)

        if token.kind == "BREAK":
            self.advance()
            self.expect("SEMI")
            return BreakStmt(span=span)

        if token.kind == "CONTINUE":
            self.advance()
            self.expect("SEMI")
            return ContinueStmt(span=span)

        raise ParseError(
            f"Unexpected statement token: {token.kind} at {token.line}:{token.col}"
        )

    def parse_try_catch_stmt(self) -> TryCatchStmt:
        span = self._span()
        self.expect("TRY")
        try_body = self.parse_block()
        self.expect("CATCH")
        error_var = self.expect("ID").value
        catch_body = self.parse_block()
        return TryCatchStmt(
            try_body=tuple(try_body),
            error_var=error_var,
            catch_body=tuple(catch_body),
            span=span,
        )

    def parse_assert_stmt(self) -> AssertStmt:
        span = self._span()
        self.expect("ASSERT")
        condition = self.parse_expr()
        message: str | None = None
        if self.match("COMMA"):
            message = self.expect("STRING").value
        self.expect("SEMI")
        return AssertStmt(condition=condition, message=message, span=span)

    def parse_if_stmt(self) -> Stmt:
        span = self._span()
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
                then_statements=tuple(then_statements),
                else_statements=tuple(else_statements) if else_statements is not None else None,
                span=span,
            )

        condition = self.parse_expr()
        then_statements = self.parse_block()
        else_statements: list[Stmt] | None = None
        if self.match("ELSE"):
            else_statements = self.parse_block()
        return IfStmt(
            condition=condition,
            then_statements=tuple(then_statements),
            else_statements=tuple(else_statements) if else_statements is not None else None,
            span=span,
        )

    def parse_while_stmt(self) -> WhileStmt:
        span = self._span()
        self.expect("WHILE")
        condition = self.parse_expr()
        statements = self.parse_block()
        return WhileStmt(condition=condition, statements=tuple(statements), span=span)

    def parse_parallel_stmt(self) -> ParallelStmt:
        span = self._span()
        self.expect("PARALLEL")

        max_concurrency: int | None = None
        if self.match("MAX_CONCURRENCY"):
            mc_token = self.expect("NUMBER")
            if "." in mc_token.value:
                raise ParseError("max_concurrency must be an integer.")
            max_concurrency = int(mc_token.value)
            if max_concurrency < 1:
                raise ParseError("max_concurrency must be at least 1.")

        self.expect("LBRACE")
        branches: list[RunStmt] = []
        while self.current().kind != "RBRACE":
            branch = self.parse_run_stmt()
            self.expect("SEMI")
            branches.append(branch)
        self.expect("RBRACE")
        self.expect("JOIN")
        self.expect("SEMI")
        return ParallelStmt(branches=tuple(branches), max_concurrency=max_concurrency, span=span)

    def parse_run_stmt(self) -> RunStmt:
        span = self._span()
        self.expect("LET")
        target = self.expect("ID").value
        self.expect("EQUAL")

        # Check for shorthand syntax: let r = task_name(args) by agent;
        # vs full syntax: let r = run task_name with { ... } ...;
        if self.match("RUN"):
            task_name = self.expect("ID").value
            self.expect("WITH")
            args = self.parse_arg_map()
        else:
            # Shorthand: let r = task_name(arg1, arg2) ...;
            task_name = self.expect("ID").value
            self.expect("LPAREN")
            positional_args = self.parse_call_args()
            self.expect("RPAREN")
            # Desugar positional args to named args using task param order.
            # We need to look up the task to get param names. We'll store them
            # positionally and resolve later or look them up now.
            # For now, we store as __pos_0, __pos_1, etc and resolve in a second pass.
            args = {}
            for idx, arg_expr in enumerate(positional_args):
                args[f"__pos_{idx}"] = arg_expr
            # Mark this as needing resolution
            args["__shorthand__"] = LiteralExpr(value=True)

        agent_name: str | None = None
        retries = 0
        on_fail = "abort"
        fallback_expr: Expr | None = None
        timeout: float | None = None

        saw_by = False
        saw_retries = False
        saw_on_fail = False
        saw_timeout = False
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

            if self.match("TIMEOUT"):
                if saw_timeout:
                    raise ParseError("Duplicate 'timeout' clause in run statement.")
                timeout_token = self.expect("NUMBER")
                timeout = float(timeout_token.value)
                if timeout <= 0:
                    raise ParseError("Timeout must be positive.")
                saw_timeout = True
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
            timeout=timeout,
            span=span,
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

    def parse_call_args(self) -> list[Expr]:
        args: list[Expr] = []
        if self.current().kind == "RPAREN":
            return args
        while True:
            args.append(self.parse_expr())
            if not self.match("COMMA"):
                break
        return args

    def parse_expr(self) -> Expr:
        return self.parse_equality()

    def parse_equality(self) -> Expr:
        expr = self.parse_addition()
        while True:
            span = self._span()
            if self.match("EQEQ"):
                right = self.parse_addition()
                expr = BinaryExpr(op="==", left=expr, right=right, span=span)
                continue
            if self.match("NEQ"):
                right = self.parse_addition()
                expr = BinaryExpr(op="!=", left=expr, right=right, span=span)
                continue
            return expr

    def parse_addition(self) -> Expr:
        expr = self.parse_primary()
        while True:
            span = self._span()
            if not self.match("PLUS"):
                return expr
            right = self.parse_primary()
            expr = BinaryExpr(op="+", left=expr, right=right, span=span)

    def parse_primary(self) -> Expr:
        token = self.current()
        span = self._span()

        if token.kind == "STRING":
            self.advance()
            return LiteralExpr(value=token.value, span=span)

        if token.kind == "NUMBER":
            self.advance()
            if "." in token.value:
                return LiteralExpr(value=float(token.value), span=span)
            return LiteralExpr(value=int(token.value), span=span)

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
            return LiteralExpr(value=True, span=span)

        if token.kind == "FALSE":
            self.advance()
            return LiteralExpr(value=False, span=span)

        if token.kind == "NULL":
            self.advance()
            return LiteralExpr(value=None, span=span)

        if token.kind == "ID":
            parts = [self.advance().value]
            while self.match("DOT"):
                parts.append(self.expect("ID").value)
            return RefExpr(parts=tuple(parts), span=span)

        raise ParseError(
            f"Invalid expression token {token.kind} at {token.line}:{token.col}"
        )

    def parse_object_literal(self) -> ObjExpr:
        span = self._span()
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
        return ObjExpr(fields=fields, span=span)

    def parse_list_literal(self) -> ListExpr:
        span = self._span()
        self.expect("LBRACKET")
        items: list[Expr] = []
        if self.current().kind != "RBRACKET":
            while True:
                items.append(self.parse_expr())
                if not self.match("COMMA"):
                    break
        self.expect("RBRACKET")
        return ListExpr(items=tuple(items), span=span)


def _resolve_shorthand_args(program: Program) -> Program:
    """Resolve shorthand positional args to named args using task param definitions."""
    new_pipelines: dict[str, PipelineDef] = {}
    for name, pipeline in program.pipelines.items():
        new_stmts = _resolve_stmts(pipeline.statements, program)
        new_pipelines[name] = PipelineDef(
            name=pipeline.name,
            params=pipeline.params,
            return_type=pipeline.return_type,
            statements=tuple(new_stmts),
        )
    new_test_blocks: list[TestBlockDef] = []
    for tb in program.test_blocks:
        new_stmts = _resolve_stmts(tb.statements, program)
        new_test_blocks.append(TestBlockDef(name=tb.name, statements=tuple(new_stmts)))

    return Program(
        agents=program.agents,
        tools=program.tools,
        tasks=program.tasks,
        pipelines=new_pipelines,
        workflows=program.workflows,
        type_aliases=program.type_aliases,
        enum_types=program.enum_types,
        test_blocks=new_test_blocks,
    )


def _resolve_stmts(stmts: tuple[Stmt, ...] | list[Stmt], program: Program) -> list[Stmt]:
    result: list[Stmt] = []
    for stmt in stmts:
        if isinstance(stmt, RunStmt):
            result.append(_resolve_run_stmt(stmt, program))
        elif isinstance(stmt, ParallelStmt):
            new_branches = tuple(_resolve_run_stmt(b, program) for b in stmt.branches)
            result.append(ParallelStmt(branches=new_branches, max_concurrency=stmt.max_concurrency, span=stmt.span))
        elif isinstance(stmt, IfStmt):
            then_stmts = tuple(_resolve_stmts(stmt.then_statements, program))
            else_stmts = tuple(_resolve_stmts(stmt.else_statements, program)) if stmt.else_statements is not None else None
            result.append(IfStmt(condition=stmt.condition, then_statements=then_stmts, else_statements=else_stmts, span=stmt.span))
        elif isinstance(stmt, IfLetStmt):
            then_stmts = tuple(_resolve_stmts(stmt.then_statements, program))
            else_stmts = tuple(_resolve_stmts(stmt.else_statements, program)) if stmt.else_statements is not None else None
            result.append(IfLetStmt(binding=stmt.binding, option_expr=stmt.option_expr, then_statements=then_stmts, else_statements=else_stmts, span=stmt.span))
        elif isinstance(stmt, WhileStmt):
            body = tuple(_resolve_stmts(stmt.statements, program))
            result.append(WhileStmt(condition=stmt.condition, statements=body, span=stmt.span))
        elif isinstance(stmt, TryCatchStmt):
            try_body = tuple(_resolve_stmts(stmt.try_body, program))
            catch_body = tuple(_resolve_stmts(stmt.catch_body, program))
            result.append(TryCatchStmt(try_body=try_body, error_var=stmt.error_var, catch_body=catch_body, span=stmt.span))
        else:
            result.append(stmt)
    return result


def _resolve_run_stmt(stmt: RunStmt, program: Program) -> RunStmt:
    if "__shorthand__" not in stmt.args:
        return stmt

    task = program.tasks.get(stmt.task_name)
    if task is None:
        # Also check pipelines for pipeline-calls-pipeline shorthand
        pipeline = program.pipelines.get(stmt.task_name)
        if pipeline is not None:
            param_names = [p.name for p in pipeline.params]
        else:
            raise ParseError(f"Shorthand call to unknown task or pipeline '{stmt.task_name}'.")
    else:
        param_names = [p.name for p in task.params]

    # Collect positional args (excluding the __shorthand__ marker)
    pos_args = []
    for key in sorted(k for k in stmt.args if k.startswith("__pos_")):
        pos_args.append(stmt.args[key])

    if len(pos_args) != len(param_names):
        raise ParseError(
            f"Shorthand call to '{stmt.task_name}' expected {len(param_names)} args, got {len(pos_args)}."
        )

    named_args = {param_names[i]: pos_args[i] for i in range(len(pos_args))}
    return RunStmt(
        target=stmt.target,
        task_name=stmt.task_name,
        args=named_args,
        agent_name=stmt.agent_name,
        retries=stmt.retries,
        on_fail=stmt.on_fail,
        fallback_expr=stmt.fallback_expr,
        timeout=stmt.timeout,
        span=stmt.span,
    )


def parse_program(source: str, *, lower: bool = True) -> Program:
    parser = Parser(tokens=lex(source))
    program = parser.parse_program()

    # Resolve shorthand args
    program = _resolve_shorthand_args(program)

    if not lower:
        return program

    from .lowering import lower_program

    return lower_program(program)
