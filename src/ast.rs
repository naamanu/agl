use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const CURRENT_LANGUAGE_VERSION: &str = "0.2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    String,
    Number,
    Bool,
    List(Box<TypeExpr>),
    Option(Box<TypeExpr>),
    Obj(BTreeMap<String, TypeExpr>),
    Enum(String),
    Alias(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    Literal {
        value: serde_json::Value,
        span: Span,
    },
    Ref {
        parts: Vec<String>,
        span: Span,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Obj {
        fields: BTreeMap<String, Expr>,
        span: Span,
    },
    List {
        items: Vec<Expr>,
        span: Span,
    },
}
impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Self::Literal { span, .. }
            | Self::Ref { span, .. }
            | Self::Binary { span, .. }
            | Self::Obj { span, .. }
            | Self::List { span, .. } => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    Add,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunStmt {
    pub target: String,
    pub callable: String,
    pub args: BTreeMap<String, Expr>,
    pub agent: Option<String>,
    pub retries: u32,
    pub on_fail: OnFail,
    pub timeout: Option<f64>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OnFail {
    Abort,
    Use(Expr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Run(RunStmt),
    Parallel {
        branches: Vec<RunStmt>,
        max_concurrency: Option<usize>,
        span: Span,
    },
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
        span: Span,
    },
    IfLet {
        binding: String,
        option: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
        span: Span,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    TryCatch {
        try_body: Vec<Stmt>,
        error_var: String,
        catch_body: Vec<Stmt>,
        span: Span,
    },
    Assert {
        condition: Expr,
        message: Option<String>,
        span: Span,
    },
    Return {
        expr: Expr,
        span: Span,
    },
}
impl Stmt {
    pub fn span(&self) -> Span {
        match self {
            Self::Run(x) => x.span,
            Self::Parallel { span, .. }
            | Self::If { span, .. }
            | Self::IfLet { span, .. }
            | Self::While { span, .. }
            | Self::TryCatch { span, .. }
            | Self::Assert { span, .. }
            | Self::Return { span, .. }
            | Self::Break(span)
            | Self::Continue(span) => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    pub model: Option<String>,
    pub tools: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub agent_task: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub statements: Vec<Stmt>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestBlock {
    pub name: String,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub language_version: String,
    pub agents: BTreeMap<String, AgentDef>,
    pub tools: BTreeMap<String, ToolDef>,
    pub tasks: BTreeMap<String, TaskDef>,
    pub pipelines: BTreeMap<String, PipelineDef>,
    pub aliases: BTreeMap<String, TypeExpr>,
    pub enums: BTreeMap<String, Vec<String>>,
    pub tests: Vec<TestBlock>,
}

impl Default for Program {
    fn default() -> Self {
        Self {
            language_version: CURRENT_LANGUAGE_VERSION.into(),
            agents: BTreeMap::new(),
            tools: BTreeMap::new(),
            tasks: BTreeMap::new(),
            pipelines: BTreeMap::new(),
            aliases: BTreeMap::new(),
            enums: BTreeMap::new(),
            tests: Vec::new(),
        }
    }
}
