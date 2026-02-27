from __future__ import annotations

from dataclasses import dataclass


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
class ObjType(TypeExpr):
    # Stored as a sorted tuple of (name, type) pairs so the node is hashable.
    fields: tuple[tuple[str, TypeExpr], ...]

    def field_dict(self) -> dict[str, TypeExpr]:
        return dict(self.fields)


@dataclass(frozen=True)
class Param:
    name: str
    type_expr: TypeExpr


@dataclass(frozen=True)
class AgentDef:
    name: str
    model: str
    tools: tuple[str, ...]


@dataclass(frozen=True)
class TaskDef:
    name: str
    params: tuple[Param, ...]
    return_type: TypeExpr


@dataclass(frozen=True)
class Expr:
    pass


@dataclass(frozen=True)
class LiteralExpr(Expr):
    value: object


@dataclass(frozen=True)
class RefExpr(Expr):
    parts: tuple[str, ...]


@dataclass(frozen=True)
class BinaryExpr(Expr):
    op: str
    left: Expr
    right: Expr


@dataclass(frozen=True)
class ObjExpr(Expr):
    # Stored as a tuple of (name, expr) pairs so the node is hashable.
    fields: tuple[tuple[str, Expr], ...]


@dataclass(frozen=True)
class ListExpr(Expr):
    items: tuple[Expr, ...]


@dataclass(frozen=True)
class Stmt:
    pass


@dataclass(frozen=True)
class RunStmt(Stmt):
    target: str
    task_name: str
    # Stored as a tuple of (name, expr) pairs so the node is hashable.
    args: tuple[tuple[str, Expr], ...]
    agent_name: str | None
    retries: int
    on_fail: str
    fallback_expr: Expr | None


@dataclass(frozen=True)
class ParallelStmt(Stmt):
    branches: tuple[RunStmt, ...]


@dataclass(frozen=True)
class IfStmt(Stmt):
    condition: Expr
    then_statements: tuple[Stmt, ...]
    else_statements: tuple[Stmt, ...] | None


@dataclass(frozen=True)
class ReturnStmt(Stmt):
    expr: Expr


@dataclass(frozen=True)
class PipelineDef:
    name: str
    params: tuple[Param, ...]
    return_type: TypeExpr
    statements: tuple[Stmt, ...]


@dataclass(frozen=True)
class Program:
    agents: dict[str, AgentDef]
    tasks: dict[str, TaskDef]
    pipelines: dict[str, PipelineDef]
