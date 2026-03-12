from __future__ import annotations

from dataclasses import dataclass, field


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
    fields: dict[str, TypeExpr]


@dataclass(frozen=True)
class Param:
    name: str
    type_expr: TypeExpr


@dataclass(frozen=True)
class AgentDef:
    name: str
    model: str
    tools: list[str]


@dataclass(frozen=True)
class TaskDef:
    name: str
    params: list[Param]
    return_type: TypeExpr
    execution_mode: str = "handler"


@dataclass(frozen=True)
class ToolDef:
    name: str
    params: list[Param]
    return_type: TypeExpr


@dataclass(frozen=True)
class Expr:
    pass


@dataclass(frozen=True)
class LiteralExpr(Expr):
    value: object


@dataclass(frozen=True)
class RefExpr(Expr):
    parts: list[str]


@dataclass(frozen=True)
class BinaryExpr(Expr):
    op: str
    left: Expr
    right: Expr


@dataclass(frozen=True)
class ObjExpr(Expr):
    fields: dict[str, Expr]


@dataclass(frozen=True)
class ListExpr(Expr):
    items: list[Expr]


@dataclass(frozen=True)
class Stmt:
    pass


@dataclass(frozen=True)
class RunStmt(Stmt):
    target: str
    task_name: str
    args: dict[str, Expr]
    agent_name: str | None
    retries: int
    on_fail: str
    fallback_expr: Expr | None


@dataclass(frozen=True)
class ParallelStmt(Stmt):
    branches: list[RunStmt]


@dataclass(frozen=True)
class IfStmt(Stmt):
    condition: Expr
    then_statements: list[Stmt]
    else_statements: list[Stmt] | None


@dataclass(frozen=True)
class IfLetStmt(Stmt):
    binding: str
    option_expr: Expr
    then_statements: list[Stmt]
    else_statements: list[Stmt] | None


@dataclass(frozen=True)
class WhileStmt(Stmt):
    condition: Expr
    statements: list[Stmt]


@dataclass(frozen=True)
class BreakStmt(Stmt):
    pass


@dataclass(frozen=True)
class ContinueStmt(Stmt):
    pass


@dataclass(frozen=True)
class ReturnStmt(Stmt):
    expr: Expr


@dataclass(frozen=True)
class WorkflowStep:
    pass


@dataclass(frozen=True)
class WorkflowStageStep(WorkflowStep):
    target: str
    agent_name: str
    task_name: str
    args: list[Expr]


@dataclass(frozen=True)
class WorkflowReviewStep(WorkflowStep):
    target: str
    reviewer_agent: str
    source: str
    reviser_agent: str
    revise_task_name: str
    max_rounds: int


@dataclass(frozen=True)
class WorkflowReturnStep(WorkflowStep):
    expr: Expr


@dataclass(frozen=True)
class PipelineDef:
    name: str
    params: list[Param]
    return_type: TypeExpr
    statements: list[Stmt]


@dataclass(frozen=True)
class WorkflowDef:
    name: str
    params: list[Param]
    return_type: TypeExpr
    steps: list[WorkflowStep]


@dataclass(frozen=True)
class Program:
    agents: dict[str, AgentDef]
    tools: dict[str, ToolDef]
    tasks: dict[str, TaskDef]
    pipelines: dict[str, PipelineDef]
    workflows: dict[str, WorkflowDef] = field(default_factory=dict)
