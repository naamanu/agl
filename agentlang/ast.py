from __future__ import annotations

from dataclasses import dataclass, field
from types import MappingProxyType


@dataclass(frozen=True)
class Span:
    line: int
    col: int


@dataclass(frozen=True)
class TypeExpr:
    pass


@dataclass(frozen=True)
class PrimitiveType(TypeExpr):
    name: str


@dataclass(frozen=True)
class ListType(TypeExpr):
    item_type: TypeExpr


@dataclass(frozen=True)
class OptionType(TypeExpr):
    item_type: TypeExpr


@dataclass(frozen=True)
class ObjType(TypeExpr):
    fields: MappingProxyType[str, TypeExpr]

    def __init__(self, fields: dict[str, TypeExpr] | MappingProxyType[str, TypeExpr]) -> None:
        object.__setattr__(self, "fields", MappingProxyType(dict(fields)))


@dataclass(frozen=True)
class EnumType(TypeExpr):
    name: str


@dataclass(frozen=True)
class Param:
    name: str
    type_expr: TypeExpr


@dataclass(frozen=True)
class AgentDef:
    name: str
    model: str | None
    tools: tuple[str, ...]


@dataclass(frozen=True)
class TaskDef:
    name: str
    params: tuple[Param, ...]
    return_type: TypeExpr
    execution_mode: str = "handler"


@dataclass(frozen=True)
class ToolDef:
    name: str
    params: tuple[Param, ...]
    return_type: TypeExpr


@dataclass(frozen=True)
class TypeAliasDef:
    name: str
    type_expr: TypeExpr


@dataclass(frozen=True)
class EnumTypeDef:
    name: str
    variants: tuple[str, ...]


@dataclass(frozen=True)
class Expr:
    span: Span | None = field(default=None, compare=False, repr=False)


@dataclass(frozen=True)
class LiteralExpr(Expr):
    value: object = None


@dataclass(frozen=True)
class RefExpr(Expr):
    parts: tuple[str, ...] = ()


@dataclass(frozen=True)
class BinaryExpr(Expr):
    op: str = ""
    left: Expr = field(default_factory=Expr)
    right: Expr = field(default_factory=Expr)


@dataclass(frozen=True)
class ObjExpr(Expr):
    fields: MappingProxyType[str, Expr] = field(default_factory=lambda: MappingProxyType({}))

    def __init__(
        self,
        fields: dict[str, Expr] | MappingProxyType[str, Expr] | None = None,
        *,
        span: Span | None = None,
    ) -> None:
        object.__setattr__(self, "span", span)
        object.__setattr__(self, "fields", MappingProxyType(dict(fields or {})))


@dataclass(frozen=True)
class ListExpr(Expr):
    items: tuple[Expr, ...] = ()


@dataclass(frozen=True)
class Stmt:
    span: Span | None = field(default=None, compare=False, repr=False)


@dataclass(frozen=True)
class RunStmt(Stmt):
    target: str = ""
    task_name: str = ""
    args: MappingProxyType[str, Expr] = field(default_factory=lambda: MappingProxyType({}))
    agent_name: str | None = None
    retries: int = 0
    on_fail: str = "abort"
    fallback_expr: Expr | None = None
    timeout: float | None = None

    def __init__(
        self,
        target: str = "",
        task_name: str = "",
        args: dict[str, Expr] | MappingProxyType[str, Expr] | None = None,
        agent_name: str | None = None,
        retries: int = 0,
        on_fail: str = "abort",
        fallback_expr: Expr | None = None,
        timeout: float | None = None,
        *,
        span: Span | None = None,
    ) -> None:
        object.__setattr__(self, "span", span)
        object.__setattr__(self, "target", target)
        object.__setattr__(self, "task_name", task_name)
        object.__setattr__(self, "args", MappingProxyType(dict(args or {})))
        object.__setattr__(self, "agent_name", agent_name)
        object.__setattr__(self, "retries", retries)
        object.__setattr__(self, "on_fail", on_fail)
        object.__setattr__(self, "fallback_expr", fallback_expr)
        object.__setattr__(self, "timeout", timeout)


@dataclass(frozen=True)
class ParallelStmt(Stmt):
    branches: tuple[RunStmt, ...] = ()
    max_concurrency: int | None = None


@dataclass(frozen=True)
class IfStmt(Stmt):
    condition: Expr = field(default_factory=Expr)
    then_statements: tuple[Stmt, ...] = ()
    else_statements: tuple[Stmt, ...] | None = None


@dataclass(frozen=True)
class IfLetStmt(Stmt):
    binding: str = ""
    option_expr: Expr = field(default_factory=Expr)
    then_statements: tuple[Stmt, ...] = ()
    else_statements: tuple[Stmt, ...] | None = None


@dataclass(frozen=True)
class WhileStmt(Stmt):
    condition: Expr = field(default_factory=Expr)
    statements: tuple[Stmt, ...] = ()


@dataclass(frozen=True)
class BreakStmt(Stmt):
    pass


@dataclass(frozen=True)
class ContinueStmt(Stmt):
    pass


@dataclass(frozen=True)
class ReturnStmt(Stmt):
    expr: Expr = field(default_factory=Expr)


@dataclass(frozen=True)
class TryCatchStmt(Stmt):
    try_body: tuple[Stmt, ...] = ()
    error_var: str = ""
    catch_body: tuple[Stmt, ...] = ()


@dataclass(frozen=True)
class AssertStmt(Stmt):
    condition: Expr = field(default_factory=Expr)
    message: str | None = None


@dataclass(frozen=True)
class WorkflowStep:
    pass


@dataclass(frozen=True)
class WorkflowStageStep(WorkflowStep):
    target: str = ""
    agent_name: str = ""
    task_name: str = ""
    args: tuple[Expr, ...] = ()


@dataclass(frozen=True)
class WorkflowReviewStep(WorkflowStep):
    target: str = ""
    reviewer_agent: str = ""
    source: str = ""
    reviser_agent: str = ""
    revise_task_name: str = ""
    max_rounds: int = 0


@dataclass(frozen=True)
class WorkflowReturnStep(WorkflowStep):
    expr: Expr = field(default_factory=Expr)


@dataclass(frozen=True)
class PipelineDef:
    name: str = ""
    params: tuple[Param, ...] = ()
    return_type: TypeExpr = field(default_factory=TypeExpr)
    statements: tuple[Stmt, ...] = ()


@dataclass(frozen=True)
class WorkflowDef:
    name: str = ""
    params: tuple[Param, ...] = ()
    return_type: TypeExpr = field(default_factory=TypeExpr)
    steps: tuple[WorkflowStep, ...] = ()


@dataclass(frozen=True)
class TestBlockDef:
    name: str = ""
    statements: tuple[Stmt, ...] = ()


@dataclass(frozen=True)
class Program:
    agents: MappingProxyType[str, AgentDef] = field(default_factory=lambda: MappingProxyType({}))
    tools: MappingProxyType[str, ToolDef] = field(default_factory=lambda: MappingProxyType({}))
    tasks: MappingProxyType[str, TaskDef] = field(default_factory=lambda: MappingProxyType({}))
    pipelines: MappingProxyType[str, PipelineDef] = field(default_factory=lambda: MappingProxyType({}))
    workflows: MappingProxyType[str, WorkflowDef] = field(default_factory=lambda: MappingProxyType({}))
    type_aliases: MappingProxyType[str, TypeAliasDef] = field(default_factory=lambda: MappingProxyType({}))
    enum_types: MappingProxyType[str, EnumTypeDef] = field(default_factory=lambda: MappingProxyType({}))
    test_blocks: tuple[TestBlockDef, ...] = ()

    def __init__(
        self,
        agents: dict | MappingProxyType | None = None,
        tools: dict | MappingProxyType | None = None,
        tasks: dict | MappingProxyType | None = None,
        pipelines: dict | MappingProxyType | None = None,
        workflows: dict | MappingProxyType | None = None,
        type_aliases: dict | MappingProxyType | None = None,
        enum_types: dict | MappingProxyType | None = None,
        test_blocks: tuple[TestBlockDef, ...] | list[TestBlockDef] | None = None,
    ) -> None:
        object.__setattr__(self, "agents", MappingProxyType(dict(agents or {})))
        object.__setattr__(self, "tools", MappingProxyType(dict(tools or {})))
        object.__setattr__(self, "tasks", MappingProxyType(dict(tasks or {})))
        object.__setattr__(self, "pipelines", MappingProxyType(dict(pipelines or {})))
        object.__setattr__(self, "workflows", MappingProxyType(dict(workflows or {})))
        object.__setattr__(self, "type_aliases", MappingProxyType(dict(type_aliases or {})))
        object.__setattr__(self, "enum_types", MappingProxyType(dict(enum_types or {})))
        object.__setattr__(self, "test_blocks", tuple(test_blocks or ()))
