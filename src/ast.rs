use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const CURRENT_LANGUAGE_VERSION: &str = "0.5";
pub const SUPPORTED_LANGUAGE_VERSIONS: &[&str] = &["0.2", "0.3", "0.4", "0.5"];

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
    Failure,
    List(Box<TypeExpr>),
    Option(Box<TypeExpr>),
    Result(Box<TypeExpr>, Box<TypeExpr>),
    Obj(BTreeMap<String, TypeExpr>),
    Record(String),
    Union(String),
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
    Record {
        name: String,
        fields: BTreeMap<String, Expr>,
        span: Span,
    },
    Variant {
        type_name: String,
        variant: String,
        fields: BTreeMap<String, Expr>,
        span: Span,
    },
    Result {
        ok: bool,
        value: Box<Expr>,
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
            | Self::Record { span, .. }
            | Self::Variant { span, .. }
            | Self::Result { span, .. }
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
    pub retry_policy: RetryPolicy,
    pub retry_on: Vec<(String, String)>,
    pub on_fail: OnFail,
    pub timeout: Option<f64>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub initial_ms: u64,
    pub max_ms: u64,
    pub multiplier: f64,
    pub jitter: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            initial_ms: 0,
            max_ms: 30_000,
            multiplier: 2.0,
            jitter: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OnFail {
    Abort,
    Use(Expr),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Stmt {
    Run(RunStmt),
    Approve {
        target: String,
        approval: String,
        prompt: String,
        expires_seconds: Option<u64>,
        delegate: Option<String>,
        span: Span,
    },
    ParallelMap {
        target: String,
        binding: String,
        items: Expr,
        run: RunStmt,
        max_concurrency: usize,
        failure_policy: FailurePolicy,
        span: Span,
    },
    Race {
        target: String,
        branches: Vec<RunStmt>,
        span: Span,
    },
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
        structured: bool,
        catch_body: Vec<Stmt>,
        span: Span,
    },
    Match {
        value: Expr,
        arms: Vec<MatchArm>,
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
            Self::Approve { span, .. } => *span,
            Self::Parallel { span, .. }
            | Self::ParallelMap { span, .. }
            | Self::Race { span, .. }
            | Self::If { span, .. }
            | Self::IfLet { span, .. }
            | Self::While { span, .. }
            | Self::TryCatch { span, .. }
            | Self::Match { span, .. }
            | Self::Assert { span, .. }
            | Self::Return { span, .. }
            | Self::Break(span)
            | Self::Continue(span) => *span,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FailurePolicy {
    FailFast,
    CollectAll,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub type_name: String,
    pub variant: String,
    pub bindings: Vec<String>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    pub model: Option<String>,
    pub tools: Vec<String>,
    pub requirements: AgentRequirements,
    pub deployment: Option<AgentDeployment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentRequirements {
    pub capabilities: BTreeSet<String>,
    pub min_context: Option<u64>,
    pub max_latency_ms: Option<u64>,
    pub quality: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentDeployment {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
    #[serde(default)]
    pub context_window: Option<u64>,
    #[serde(default)]
    pub expected_latency_ms: Option<u64>,
    #[serde(default)]
    pub quality: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub agent_task: bool,
    pub effects: BTreeSet<String>,
    pub idempotency: Idempotency,
    pub concurrency_group: Option<String>,
    pub concurrency_limit: Option<u32>,
    pub rate_limit_per_second: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub effects: BTreeSet<String>,
    pub idempotency: Idempotency,
    pub concurrency_group: Option<String>,
    pub concurrency_limit: Option<u32>,
    pub rate_limit_per_second: Option<u32>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Idempotency {
    Pure,
    Idempotent,
    KeyedBy(String),
    NonIdempotent,
    #[default]
    Unspecified,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordDef {
    pub name: String,
    pub fields: BTreeMap<String, TypeExpr>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnionDef {
    pub name: String,
    pub variants: BTreeMap<String, BTreeMap<String, TypeExpr>>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PipelineDef {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeExpr,
    pub effects: Option<BTreeSet<String>>,
    pub budget: Option<ResourceBudget>,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub time_ms: Option<u64>,
    pub tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    pub tool_calls: Option<u64>,
    pub retries: Option<u64>,
    pub concurrency: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TestBlock {
    pub name: String,
    pub statements: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalDef {
    pub name: String,
    pub pipeline: String,
    pub dataset: String,
    pub trials: u32,
    pub baseline: Option<String>,
    pub assert_schema: bool,
    pub assert_expected: bool,
    pub max_latency_ms: Option<u64>,
    pub max_cost_usd: Option<f64>,
    pub semantic_grader: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub language_version: String,
    pub agents: BTreeMap<String, AgentDef>,
    pub tools: BTreeMap<String, ToolDef>,
    pub tasks: BTreeMap<String, TaskDef>,
    pub pipelines: BTreeMap<String, PipelineDef>,
    pub aliases: BTreeMap<String, TypeExpr>,
    pub records: BTreeMap<String, RecordDef>,
    pub unions: BTreeMap<String, UnionDef>,
    pub enums: BTreeMap<String, Vec<String>>,
    pub tests: Vec<TestBlock>,
    pub evals: BTreeMap<String, EvalDef>,
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
            records: BTreeMap::new(),
            unions: BTreeMap::new(),
            enums: BTreeMap::new(),
            tests: Vec::new(),
            evals: BTreeMap::new(),
        }
    }
}
