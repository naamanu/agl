use crate::ast::*;
use crate::lexer::{Kind, LexError, Token, lex};
use serde_json::{Number, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ParseError {
    #[error(transparent)]
    Lex(#[from] LexError),
    #[error("{message} at {line}:{col}")]
    Syntax {
        message: String,
        line: usize,
        col: usize,
    },
    #[error("{0}")]
    Semantic(String),
}

pub fn parse_program(source: &str) -> Result<Program, ParseError> {
    let mut p = Parser {
        tokens: lex(source)?,
        pos: 0,
        program: Program::default(),
        workflows: Vec::new(),
    };
    p.program()?;
    p.resolve_shorthand()?;
    p.lower_workflows()?;
    Ok(p.program)
}

#[derive(Debug)]
struct Workflow {
    name: String,
    params: Vec<Param>,
    return_type: TypeExpr,
    steps: Vec<WorkflowStep>,
}
#[derive(Debug)]
enum WorkflowStep {
    Stage {
        target: String,
        agent: String,
        task: String,
        args: Vec<Expr>,
        span: Span,
    },
    Review {
        target: String,
        reviewer: String,
        source: String,
        reviser: String,
        task: String,
        rounds: u32,
        span: Span,
    },
    Return(Expr, Span),
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    program: Program,
    workflows: Vec<Workflow>,
}
impl Parser {
    fn cur(&self) -> &Token {
        &self.tokens[self.pos]
    }
    fn at(&self, text: &str) -> bool {
        self.cur().text == text
    }
    fn bump(&mut self) -> Token {
        let t = self.cur().clone();
        self.pos += 1;
        t
    }
    fn take(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect(&mut self, text: &str) -> Result<Token, ParseError> {
        if self.at(text) {
            Ok(self.bump())
        } else {
            Err(self.error(format!("expected {text:?}, got {:?}", self.cur().text)))
        }
    }
    fn ident(&mut self) -> Result<Token, ParseError> {
        if self.cur().kind == Kind::Id {
            Ok(self.bump())
        } else {
            Err(self.error(format!("expected identifier, got {:?}", self.cur().text)))
        }
    }
    fn string(&mut self) -> Result<Token, ParseError> {
        if self.cur().kind == Kind::String {
            Ok(self.bump())
        } else {
            Err(self.error("expected string".into()))
        }
    }
    fn number(&mut self) -> Result<Token, ParseError> {
        if self.cur().kind == Kind::Number {
            Ok(self.bump())
        } else {
            Err(self.error("expected number".into()))
        }
    }
    fn error(&self, message: String) -> ParseError {
        ParseError::Syntax {
            message,
            line: self.cur().span.line,
            col: self.cur().span.col,
        }
    }

    fn program(&mut self) -> Result<(), ParseError> {
        while self.cur().kind != Kind::Eof {
            match self.cur().text.as_str() {
                "agent" => {
                    let x = self.agent()?;
                    unique(&self.program.agents, &x.name, "agent")?;
                    self.program.agents.insert(x.name.clone(), x);
                }
                "tool" => {
                    let x = self.tool()?;
                    unique(&self.program.tools, &x.name, "tool")?;
                    self.program.tools.insert(x.name.clone(), x);
                }
                "task" => {
                    let x = self.task()?;
                    unique(&self.program.tasks, &x.name, "task")?;
                    self.program.tasks.insert(x.name.clone(), x);
                }
                "pipeline" => {
                    let x = self.pipeline()?;
                    unique(&self.program.pipelines, &x.name, "pipeline")?;
                    self.program.pipelines.insert(x.name.clone(), x);
                }
                "workflow" => {
                    let workflow = self.workflow()?;
                    self.workflows.push(workflow);
                }
                "type" => self.alias()?,
                "enum" => self.enum_def()?,
                "test" => self.test_block()?,
                _ => {
                    return Err(
                        self.error(format!("unexpected top-level token {:?}", self.cur().text))
                    );
                }
            }
        }
        Ok(())
    }
    fn alias(&mut self) -> Result<(), ParseError> {
        self.expect("type")?;
        let n = self.ident()?.text;
        self.expect("=")?;
        let ty = self.ty()?;
        self.expect(";")?;
        if self.program.aliases.insert(n.clone(), ty).is_some() {
            return Err(ParseError::Semantic(format!("duplicate type alias: {n}")));
        }
        Ok(())
    }
    fn enum_def(&mut self) -> Result<(), ParseError> {
        self.expect("enum")?;
        let n = self.ident()?.text;
        self.expect("{")?;
        let mut xs = Vec::new();
        let mut seen = BTreeSet::new();
        while !self.at("}") {
            let x = self.ident()?.text;
            if !seen.insert(x.clone()) {
                return Err(ParseError::Semantic(format!(
                    "duplicate enum variant '{x}' in enum '{n}'"
                )));
            }
            xs.push(x);
            if !self.take(",") {
                break;
            }
        }
        self.expect("}")?;
        self.expect(";")?;
        if self.program.enums.insert(n.clone(), xs).is_some() {
            return Err(ParseError::Semantic(format!("duplicate enum: {n}")));
        }
        Ok(())
    }
    fn agent(&mut self) -> Result<AgentDef, ParseError> {
        self.expect("agent")?;
        let name = self.ident()?.text;
        self.expect("{")?;
        let (mut model, mut tools) = (None, None);
        while !self.at("}") {
            if self.take(",") {
                continue;
            }
            match self.cur().text.as_str() {
                "model" => {
                    self.bump();
                    self.expect(":")?;
                    if model.replace(self.string()?.text).is_some() {
                        return Err(ParseError::Semantic(
                            "duplicate 'model' in agent definition".into(),
                        ));
                    }
                }
                "tools" => {
                    self.bump();
                    self.expect(":")?;
                    if tools.is_some() {
                        return Err(ParseError::Semantic(
                            "duplicate 'tools' in agent definition".into(),
                        ));
                    }
                    self.expect("[")?;
                    let mut x = Vec::new();
                    while !self.at("]") {
                        x.push(self.ident()?.text);
                        if !self.take(",") {
                            break;
                        }
                    }
                    self.expect("]")?;
                    tools = Some(x)
                }
                _ => return Err(self.error("unexpected token in agent body".into())),
            }
        }
        self.expect("}")?;
        Ok(AgentDef {
            name: name.clone(),
            model,
            tools: tools.ok_or_else(|| {
                ParseError::Semantic(format!("agent '{name}' must declare tools"))
            })?,
        })
    }
    fn task(&mut self) -> Result<TaskDef, ParseError> {
        self.expect("task")?;
        let name = self.ident()?.text;
        let params = self.signature_params()?;
        self.expect("->")?;
        let return_type = self.ty()?;
        let agent_task = if self.take("by") {
            self.expect("agent")?;
            true
        } else {
            false
        };
        self.expect("{")?;
        self.expect("}")?;
        Ok(TaskDef {
            name,
            params,
            return_type,
            agent_task,
        })
    }
    fn tool(&mut self) -> Result<ToolDef, ParseError> {
        self.expect("tool")?;
        let name = self.ident()?.text;
        let params = self.signature_params()?;
        self.expect("->")?;
        let return_type = self.ty()?;
        self.expect("{")?;
        self.expect("}")?;
        Ok(ToolDef {
            name,
            params,
            return_type,
        })
    }
    fn pipeline(&mut self) -> Result<PipelineDef, ParseError> {
        self.expect("pipeline")?;
        let name = self.ident()?.text;
        let params = self.signature_params()?;
        self.expect("->")?;
        let return_type = self.ty()?;
        let statements = self.block()?;
        Ok(PipelineDef {
            name,
            params,
            return_type,
            statements,
        })
    }
    fn workflow(&mut self) -> Result<Workflow, ParseError> {
        self.expect("workflow")?;
        let name = self.ident()?.text;
        let params = self.signature_params()?;
        self.expect("->")?;
        let return_type = self.ty()?;
        self.expect("{")?;
        let mut steps = Vec::new();
        while !self.at("}") {
            let span = self.cur().span;
            match self.cur().text.as_str() {
                "stage" => {
                    self.bump();
                    let target = self.ident()?.text;
                    self.expect("=")?;
                    let agent = self.ident()?.text;
                    self.expect("does")?;
                    let task = self.ident()?.text;
                    let args = self.call_args()?;
                    self.expect(";")?;
                    steps.push(WorkflowStep::Stage {
                        target,
                        agent,
                        task,
                        args,
                        span,
                    })
                }
                "review" => {
                    self.bump();
                    let target = self.ident()?.text;
                    self.expect("=")?;
                    let reviewer = self.ident()?.text;
                    self.expect("checks")?;
                    let source = self.ident()?.text;
                    self.expect("revise")?;
                    self.expect("with")?;
                    let reviser = self.ident()?.text;
                    self.expect("using")?;
                    let task = self.ident()?.text;
                    self.expect("max_rounds")?;
                    let n = self.integer("max_rounds")?;
                    self.expect(";")?;
                    steps.push(WorkflowStep::Review {
                        target,
                        reviewer,
                        source,
                        reviser,
                        task,
                        rounds: n,
                        span,
                    })
                }
                "return" => {
                    self.bump();
                    let e = self.expr()?;
                    self.expect(";")?;
                    steps.push(WorkflowStep::Return(e, span))
                }
                _ => return Err(self.error("unexpected workflow step".into())),
            }
        }
        self.expect("}")?;
        Ok(Workflow {
            name,
            params,
            return_type,
            steps,
        })
    }
    fn test_block(&mut self) -> Result<(), ParseError> {
        self.expect("test")?;
        let name = self.string()?.text;
        let statements = self.block()?;
        self.program.tests.push(TestBlock { name, statements });
        Ok(())
    }
    fn signature_params(&mut self) -> Result<Vec<Param>, ParseError> {
        self.expect("(")?;
        let mut ps = Vec::new();
        let mut seen = BTreeSet::new();
        while !self.at(")") {
            let n = self.ident()?.text;
            if !seen.insert(n.clone()) {
                return Err(ParseError::Semantic(format!("duplicate parameter '{n}'")));
            }
            self.expect(":")?;
            ps.push(Param {
                name: n,
                ty: self.ty()?,
            });
            if !self.take(",") {
                break;
            }
        }
        self.expect(")")?;
        Ok(ps)
    }
    fn ty(&mut self) -> Result<TypeExpr, ParseError> {
        let t = self.ident()?.text;
        match t.as_str() {
            "String" => Ok(TypeExpr::String),
            "Number" => Ok(TypeExpr::Number),
            "Bool" => Ok(TypeExpr::Bool),
            "List" | "Option" => {
                self.expect("[")?;
                let inner = self.ty()?;
                self.expect("]")?;
                Ok(if t == "List" {
                    TypeExpr::List(Box::new(inner))
                } else {
                    TypeExpr::Option(Box::new(inner))
                })
            }
            "Obj" => {
                self.expect("{")?;
                let mut fs = BTreeMap::new();
                while !self.at("}") {
                    let n = self.ident()?.text;
                    self.expect(":")?;
                    let v = self.ty()?;
                    if fs.insert(n.clone(), v).is_some() {
                        return Err(ParseError::Semantic(format!(
                            "duplicate object type field '{n}'"
                        )));
                    }
                    if !self.take(",") {
                        break;
                    }
                }
                self.expect("}")?;
                Ok(TypeExpr::Obj(fs))
            }
            _ if self.program.enums.contains_key(&t) => Ok(TypeExpr::Enum(t)),
            _ if self.program.aliases.contains_key(&t) => Ok(TypeExpr::Alias(t)),
            _ => Err(ParseError::Semantic(format!(
                "unknown type '{t}' (types must be declared before use)"
            ))),
        }
    }
    fn block(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect("{")?;
        let mut xs = Vec::new();
        while !self.at("}") {
            xs.push(self.stmt()?)
        }
        self.expect("}")?;
        Ok(xs)
    }
    fn stmt(&mut self) -> Result<Stmt, ParseError> {
        let span = self.cur().span;
        match self.cur().text.as_str() {
            "let" => {
                let x = self.run()?;
                self.expect(";")?;
                Ok(Stmt::Run(x))
            }
            "parallel" => {
                self.bump();
                let max_concurrency = if self.take("max_concurrency") {
                    Some(self.integer("max_concurrency")? as usize)
                } else {
                    None
                };
                self.expect("{")?;
                let mut branches = Vec::new();
                while !self.at("}") {
                    branches.push(self.run()?);
                    self.expect(";")?;
                }
                self.expect("}")?;
                self.expect("join")?;
                self.expect(";")?;
                Ok(Stmt::Parallel {
                    branches,
                    max_concurrency,
                    span,
                })
            }
            "if" => {
                self.bump();
                if self.take("let") {
                    let binding = self.ident()?.text;
                    self.expect("=")?;
                    let option = self.expr()?;
                    let then_body = self.block()?;
                    let else_body = if self.take("else") {
                        self.block()?
                    } else {
                        Vec::new()
                    };
                    Ok(Stmt::IfLet {
                        binding,
                        option,
                        then_body,
                        else_body,
                        span,
                    })
                } else {
                    let condition = self.expr()?;
                    let then_body = self.block()?;
                    let else_body = if self.take("else") {
                        self.block()?
                    } else {
                        Vec::new()
                    };
                    Ok(Stmt::If {
                        condition,
                        then_body,
                        else_body,
                        span,
                    })
                }
            }
            "while" => {
                self.bump();
                let condition = self.expr()?;
                let body = self.block()?;
                Ok(Stmt::While {
                    condition,
                    body,
                    span,
                })
            }
            "try" => {
                self.bump();
                let try_body = self.block()?;
                self.expect("catch")?;
                let error_var = self.ident()?.text;
                let catch_body = self.block()?;
                Ok(Stmt::TryCatch {
                    try_body,
                    error_var,
                    catch_body,
                    span,
                })
            }
            "assert" => {
                self.bump();
                let condition = self.expr()?;
                let message = if self.take(",") {
                    Some(self.string()?.text)
                } else {
                    None
                };
                self.expect(";")?;
                Ok(Stmt::Assert {
                    condition,
                    message,
                    span,
                })
            }
            "return" => {
                self.bump();
                let expr = self.expr()?;
                self.expect(";")?;
                Ok(Stmt::Return { expr, span })
            }
            "break" => {
                self.bump();
                self.expect(";")?;
                Ok(Stmt::Break(span))
            }
            "continue" => {
                self.bump();
                self.expect(";")?;
                Ok(Stmt::Continue(span))
            }
            _ => Err(self.error("unexpected statement".into())),
        }
    }
    fn run(&mut self) -> Result<RunStmt, ParseError> {
        let span = self.expect("let")?.span;
        let target = self.ident()?.text;
        self.expect("=")?;
        let callable;
        let mut args = BTreeMap::new();
        if self.take("run") {
            callable = self.ident()?.text;
            self.expect("with")?;
            args = self.arg_map()?
        } else {
            callable = self.ident()?.text;
            for (i, e) in self.call_args()?.into_iter().enumerate() {
                args.insert(format!("__pos_{i}"), e);
            }
        }
        let (mut agent, mut retries, mut on_fail, mut timeout) = (None, 0, OnFail::Abort, None);
        let mut seen = BTreeSet::new();
        loop {
            let clause = self.cur().text.clone();
            if !matches!(clause.as_str(), "by" | "retries" | "on_fail" | "timeout") {
                break;
            }
            if !seen.insert(clause.clone()) {
                return Err(ParseError::Semantic(format!(
                    "duplicate '{clause}' clause in run statement"
                )));
            }
            self.bump();
            match clause.as_str() {
                "by" => agent = Some(self.ident()?.text),
                "retries" => retries = self.integer("retries")?,
                "on_fail" => {
                    if self.take("abort") {
                        on_fail = OnFail::Abort
                    } else {
                        self.expect("use")?;
                        on_fail = OnFail::Use(self.expr()?)
                    }
                }
                "timeout" => {
                    let n = self.number()?.text.parse::<f64>().unwrap();
                    if n <= 0.0 {
                        return Err(ParseError::Semantic("timeout must be positive".into()));
                    }
                    timeout = Some(n)
                }
                _ => unreachable!(),
            }
        }
        Ok(RunStmt {
            target,
            callable,
            args,
            agent,
            retries,
            on_fail,
            timeout,
            span,
        })
    }
    fn integer(&mut self, label: &str) -> Result<u32, ParseError> {
        let t = self.number()?;
        if t.text.contains('.') {
            return Err(ParseError::Semantic(format!("{label} must be an integer")));
        }
        t.text
            .parse()
            .map_err(|_| ParseError::Semantic(format!("invalid {label}")))
    }
    fn arg_map(&mut self) -> Result<BTreeMap<String, Expr>, ParseError> {
        self.expect("{")?;
        let mut m = BTreeMap::new();
        while !self.at("}") {
            let n = self.ident()?.text;
            self.expect(":")?;
            let e = self.expr()?;
            if m.insert(n.clone(), e).is_some() {
                return Err(ParseError::Semantic(format!("duplicate argument '{n}'")));
            }
            if !self.take(",") {
                break;
            }
        }
        self.expect("}")?;
        Ok(m)
    }
    fn call_args(&mut self) -> Result<Vec<Expr>, ParseError> {
        self.expect("(")?;
        let mut xs = Vec::new();
        while !self.at(")") {
            xs.push(self.expr()?);
            if !self.take(",") {
                break;
            }
        }
        self.expect(")")?;
        Ok(xs)
    }
    fn expr(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.add()?;
        loop {
            let span = self.cur().span;
            let op = if self.take("==") {
                BinaryOp::Eq
            } else if self.take("!=") {
                BinaryOp::Ne
            } else {
                break;
            };
            let right = self.add()?;
            e = Expr::Binary {
                op,
                left: Box::new(e),
                right: Box::new(right),
                span,
            }
        }
        Ok(e)
    }
    fn add(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.primary()?;
        while self.at("+") {
            let span = self.bump().span;
            let right = self.primary()?;
            e = Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(e),
                right: Box::new(right),
                span,
            }
        }
        Ok(e)
    }
    fn primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.cur().clone();
        match t.kind {
            Kind::String => {
                self.bump();
                Ok(Expr::Literal {
                    value: Value::String(t.text),
                    span: t.span,
                })
            }
            Kind::Number => {
                self.bump();
                let n = if t.text.contains('.') {
                    Number::from_f64(t.text.parse().unwrap()).unwrap()
                } else {
                    Number::from(t.text.parse::<i64>().unwrap())
                };
                Ok(Expr::Literal {
                    value: Value::Number(n),
                    span: t.span,
                })
            }
            _ => match t.text.as_str() {
                "true" | "false" => {
                    self.bump();
                    Ok(Expr::Literal {
                        value: Value::Bool(t.text == "true"),
                        span: t.span,
                    })
                }
                "null" => {
                    self.bump();
                    Ok(Expr::Literal {
                        value: Value::Null,
                        span: t.span,
                    })
                }
                "(" => {
                    self.bump();
                    let e = self.expr()?;
                    self.expect(")")?;
                    Ok(e)
                }
                "{" => {
                    self.bump();
                    let mut fields = BTreeMap::new();
                    while !self.at("}") {
                        let n = self.ident()?.text;
                        self.expect(":")?;
                        let e = self.expr()?;
                        if fields.insert(n.clone(), e).is_some() {
                            return Err(ParseError::Semantic(format!(
                                "duplicate object literal field '{n}'"
                            )));
                        }
                        if !self.take(",") {
                            break;
                        }
                    }
                    self.expect("}")?;
                    Ok(Expr::Obj {
                        fields,
                        span: t.span,
                    })
                }
                "[" => {
                    self.bump();
                    let mut items = Vec::new();
                    while !self.at("]") {
                        items.push(self.expr()?);
                        if !self.take(",") {
                            break;
                        }
                    }
                    self.expect("]")?;
                    Ok(Expr::List {
                        items,
                        span: t.span,
                    })
                }
                _ if t.kind == Kind::Id => {
                    let mut parts = vec![self.bump().text];
                    while self.take(".") {
                        parts.push(self.ident()?.text)
                    }
                    Ok(Expr::Ref {
                        parts,
                        span: t.span,
                    })
                }
                _ => Err(self.error("invalid expression".into())),
            },
        }
    }

    fn resolve_shorthand(&mut self) -> Result<(), ParseError> {
        let signatures: BTreeMap<String, Vec<String>> =
            self.program
                .tasks
                .iter()
                .map(|(n, t)| (n.clone(), t.params.iter().map(|p| p.name.clone()).collect()))
                .chain(
                    self.program.pipelines.iter().map(|(n, t)| {
                        (n.clone(), t.params.iter().map(|p| p.name.clone()).collect())
                    }),
                )
                .collect();
        for p in self.program.pipelines.values_mut() {
            resolve_stmts(&mut p.statements, &signatures)?
        }
        for t in &mut self.program.tests {
            resolve_stmts(&mut t.statements, &signatures)?
        }
        Ok(())
    }
    fn lower_workflows(&mut self) -> Result<(), ParseError> {
        if self.workflows.iter().any(|w| {
            w.steps
                .iter()
                .any(|s| matches!(s, WorkflowStep::Review { .. }))
        }) {
            let countdown = TaskDef {
                name: "countdown".into(),
                params: vec![Param {
                    name: "current".into(),
                    ty: TypeExpr::Number,
                }],
                return_type: TypeExpr::Obj(BTreeMap::from([
                    ("done".into(), TypeExpr::Bool),
                    ("next".into(), TypeExpr::Number),
                ])),
                agent_task: false,
            };
            match self.program.tasks.get("countdown") {
                None => {
                    self.program.tasks.insert("countdown".into(), countdown);
                }
                Some(existing) if existing == &countdown => {}
                Some(_) => return Err(ParseError::Semantic("workflow lowering requires task 'countdown(current: Number) -> Obj{next: Number, done: Bool}'".into())),
            }
        }
        for w in std::mem::take(&mut self.workflows) {
            if self.program.pipelines.contains_key(&w.name) {
                return Err(ParseError::Semantic(format!(
                    "duplicate workflow: {}",
                    w.name
                )));
            }
            let mut body = Vec::new();
            let mut aliases: BTreeMap<String, String> = w
                .params
                .iter()
                .map(|p| (p.name.clone(), p.name.clone()))
                .collect();
            let mut types: BTreeMap<String, TypeExpr> = w
                .params
                .iter()
                .map(|p| (p.name.clone(), p.ty.clone()))
                .collect();
            for step in w.steps {
                match step {
                    WorkflowStep::Stage {
                        target,
                        agent,
                        task,
                        args,
                        span,
                    } => {
                        let def = self.program.tasks.get(&task).ok_or_else(|| {
                            ParseError::Semantic(format!(
                                "workflow '{}' references unknown task '{task}'",
                                w.name
                            ))
                        })?;
                        if args.len() != def.params.len() {
                            return Err(ParseError::Semantic(format!(
                                "workflow '{}' stage '{task}' expected {} args, got {}",
                                w.name,
                                def.params.len(),
                                args.len()
                            )));
                        }
                        let named = def
                            .params
                            .iter()
                            .zip(args)
                            .map(|(p, e)| (p.name.clone(), rewrite(e, &aliases)))
                            .collect();
                        body.push(Stmt::Run(RunStmt {
                            target: target.clone(),
                            callable: task,
                            args: named,
                            agent: Some(agent),
                            retries: 0,
                            on_fail: OnFail::Abort,
                            timeout: None,
                            span,
                        }));
                        aliases.insert(target.clone(), target.clone());
                        types.insert(target.clone(), def.return_type.clone());
                    }
                    WorkflowStep::Return(e, span) => body.push(Stmt::Return {
                        expr: rewrite(e, &aliases),
                        span,
                    }),
                    WorkflowStep::Review {
                        target,
                        reviewer,
                        source,
                        reviser,
                        task,
                        rounds,
                        span,
                    } => {
                        let source_var = aliases.get(&source).cloned().ok_or_else(|| {
                            ParseError::Semantic(format!(
                                "workflow '{}' references unknown artifact '{source}'",
                                w.name
                            ))
                        })?;
                        let source_type = types.get(&source).cloned().ok_or_else(|| {
                            ParseError::Semantic(format!(
                                "workflow '{}' references unknown artifact '{source}'",
                                w.name
                            ))
                        })?;
                        let fields = match &source_type {
                            TypeExpr::Obj(fields) => fields,
                            _ => {
                                return Err(ParseError::Semantic(format!(
                                    "workflow '{}' can only review object-shaped artifacts",
                                    w.name
                                )));
                            }
                        };
                        let review_name = format!("review_{target}");
                        let review_task =
                            self.program.tasks.get(&review_name).ok_or_else(|| {
                                ParseError::Semantic(format!(
                                    "workflow '{}' could not infer review task '{review_name}'",
                                    w.name
                                ))
                            })?;
                        let revise_task = self.program.tasks.get(&task).ok_or_else(|| {
                            ParseError::Semantic(format!(
                                "workflow '{}' references unknown revise task '{task}'",
                                w.name
                            ))
                        })?;
                        if revise_task.return_type != source_type {
                            return Err(ParseError::Semantic(format!(
                                "workflow '{}' requires revise task '{task}' to return the reviewed artifact type",
                                w.name
                            )));
                        }
                        let review_var = format!("__{target}_review");
                        let remaining_var = format!("__{target}_remaining");
                        let review_args = auto_bind(
                            &review_task.params,
                            &w.params,
                            &source,
                            fields,
                            &aliases,
                            &BTreeMap::new(),
                            &w.name,
                            &review_name,
                        )?;
                        body.push(Stmt::Run(RunStmt {
                            target: review_var.clone(),
                            callable: review_name.clone(),
                            args: review_args,
                            agent: Some(reviewer.clone()),
                            retries: 0,
                            on_fail: OnFail::Abort,
                            timeout: None,
                            span,
                        }));
                        body.push(Stmt::Run(RunStmt {
                            target: remaining_var.clone(),
                            callable: "countdown".into(),
                            args: BTreeMap::from([(
                                "current".into(),
                                Expr::Literal {
                                    value: Value::from(rounds + 1),
                                    span,
                                },
                            )]),
                            agent: None,
                            retries: 0,
                            on_fail: OnFail::Abort,
                            timeout: None,
                            span,
                        }));
                        let extras = BTreeMap::from([
                            (
                                "approved".into(),
                                Expr::Ref {
                                    parts: vec![review_var.clone(), "approved".into()],
                                    span,
                                },
                            ),
                            (
                                "feedback".into(),
                                Expr::Ref {
                                    parts: vec![review_var.clone(), "feedback".into()],
                                    span,
                                },
                            ),
                        ]);
                        let revise_args = auto_bind(
                            &revise_task.params,
                            &w.params,
                            &source,
                            fields,
                            &aliases,
                            &extras,
                            &w.name,
                            &task,
                        )?;
                        let repeated_review_args = auto_bind(
                            &review_task.params,
                            &w.params,
                            &source,
                            fields,
                            &aliases,
                            &BTreeMap::new(),
                            &w.name,
                            &review_name,
                        )?;
                        let loop_body = vec![
                            Stmt::If {
                                condition: Expr::Ref {
                                    parts: vec![remaining_var.clone(), "done".into()],
                                    span,
                                },
                                then_body: vec![Stmt::Break(span)],
                                else_body: vec![],
                                span,
                            },
                            Stmt::Run(RunStmt {
                                target: source_var.clone(),
                                callable: task,
                                args: revise_args,
                                agent: Some(reviser),
                                retries: 0,
                                on_fail: OnFail::Abort,
                                timeout: None,
                                span,
                            }),
                            Stmt::Run(RunStmt {
                                target: review_var.clone(),
                                callable: review_name,
                                args: repeated_review_args,
                                agent: Some(reviewer),
                                retries: 0,
                                on_fail: OnFail::Abort,
                                timeout: None,
                                span,
                            }),
                            Stmt::Run(RunStmt {
                                target: remaining_var.clone(),
                                callable: "countdown".into(),
                                args: BTreeMap::from([(
                                    "current".into(),
                                    Expr::Ref {
                                        parts: vec![remaining_var.clone(), "next".into()],
                                        span,
                                    },
                                )]),
                                agent: None,
                                retries: 0,
                                on_fail: OnFail::Abort,
                                timeout: None,
                                span,
                            }),
                        ];
                        body.push(Stmt::While {
                            condition: Expr::Binary {
                                op: BinaryOp::Eq,
                                left: Box::new(Expr::Ref {
                                    parts: vec![review_var, "approved".into()],
                                    span,
                                }),
                                right: Box::new(Expr::Literal {
                                    value: Value::Bool(false),
                                    span,
                                }),
                                span,
                            },
                            body: loop_body,
                            span,
                        });
                        aliases.remove(&source);
                        types.remove(&source);
                        aliases.insert(target.clone(), source_var);
                        types.insert(target, source_type);
                    }
                }
            }
            self.program.pipelines.insert(
                w.name.clone(),
                PipelineDef {
                    name: w.name,
                    params: w.params,
                    return_type: w.return_type,
                    statements: body,
                },
            );
        }
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)] // Mirrors the workflow lowering judgment inputs.
fn auto_bind(
    task_params: &[Param],
    workflow_params: &[Param],
    source: &str,
    source_fields: &BTreeMap<String, TypeExpr>,
    aliases: &BTreeMap<String, String>,
    extras: &BTreeMap<String, Expr>,
    workflow: &str,
    task: &str,
) -> Result<BTreeMap<String, Expr>, ParseError> {
    let span = Span { line: 1, col: 1 };
    let mut available: BTreeMap<String, Expr> = workflow_params
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                Expr::Ref {
                    parts: vec![p.name.clone()],
                    span,
                },
            )
        })
        .collect();
    for field in source_fields.keys() {
        available.insert(
            field.clone(),
            Expr::Ref {
                parts: vec![source.into(), field.clone()],
                span,
            },
        );
    }
    available.extend(extras.clone());
    task_params
        .iter()
        .map(|p| {
            available
                .get(&p.name)
                .cloned()
                .map(|e| (p.name.clone(), rewrite(e, aliases)))
                .ok_or_else(|| {
                    ParseError::Semantic(format!(
                        "workflow '{workflow}' could not auto-bind parameter '{}' for task '{task}'",
                        p.name
                    ))
                })
        })
        .collect()
}

fn unique<T>(map: &BTreeMap<String, T>, name: &str, kind: &str) -> Result<(), ParseError> {
    if map.contains_key(name) {
        Err(ParseError::Semantic(format!("duplicate {kind}: {name}")))
    } else {
        Ok(())
    }
}
fn resolve_stmts(xs: &mut [Stmt], sigs: &BTreeMap<String, Vec<String>>) -> Result<(), ParseError> {
    for x in xs {
        match x {
            Stmt::Run(r) => resolve_run(r, sigs)?,
            Stmt::Parallel { branches, .. } => {
                for r in branches {
                    resolve_run(r, sigs)?
                }
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            }
            | Stmt::IfLet {
                then_body,
                else_body,
                ..
            } => {
                resolve_stmts(then_body, sigs)?;
                resolve_stmts(else_body, sigs)?
            }
            Stmt::While { body, .. } => resolve_stmts(body, sigs)?,
            Stmt::TryCatch {
                try_body,
                catch_body,
                ..
            } => {
                resolve_stmts(try_body, sigs)?;
                resolve_stmts(catch_body, sigs)?
            }
            _ => {}
        }
    }
    Ok(())
}
fn resolve_run(r: &mut RunStmt, sigs: &BTreeMap<String, Vec<String>>) -> Result<(), ParseError> {
    if !r.args.keys().any(|x| x.starts_with("__pos_")) {
        return Ok(());
    }
    let ps = sigs.get(&r.callable).ok_or_else(|| {
        ParseError::Semantic(format!("unknown task or pipeline '{}'", r.callable))
    })?;
    if r.args.len() != ps.len() {
        return Err(ParseError::Semantic(format!(
            "'{}' expected {} positional args, got {}",
            r.callable,
            ps.len(),
            r.args.len()
        )));
    }
    let old = std::mem::take(&mut r.args);
    for (i, p) in ps.iter().enumerate() {
        r.args.insert(p.clone(), old[&format!("__pos_{i}")].clone());
    }
    Ok(())
}
fn rewrite(e: Expr, aliases: &BTreeMap<String, String>) -> Expr {
    match e {
        Expr::Ref { mut parts, span } => {
            if let Some(x) = aliases.get(&parts[0]) {
                parts[0] = x.clone()
            }
            Expr::Ref { parts, span }
        }
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => Expr::Binary {
            op,
            left: Box::new(rewrite(*left, aliases)),
            right: Box::new(rewrite(*right, aliases)),
            span,
        },
        Expr::Obj { fields, span } => Expr::Obj {
            fields: fields
                .into_iter()
                .map(|(k, v)| (k, rewrite(v, aliases)))
                .collect(),
            span,
        },
        Expr::List { items, span } => Expr::List {
            items: items.into_iter().map(|x| rewrite(x, aliases)).collect(),
            span,
        },
        x => x,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_small_program() {
        let p=parse_program(r#"task echo(x: String) -> String {} pipeline main(x: String) -> String { let y = echo(x); return y; }"#).unwrap();
        assert_eq!(p.pipelines["main"].statements.len(), 2)
    }
    #[test]
    fn catches_duplicate_args() {
        assert!(parse_program("task x(a: String, a: String) -> String {}").is_err())
    }
}
