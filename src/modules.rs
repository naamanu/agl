use crate::ast::*;
use crate::{check_program, parse_program};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModuleError {
    #[error("module I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("module parse error at {path}: {message}")]
    Parse { path: PathBuf, message: String },
    #[error("module cycle: {0}")]
    Cycle(String),
    #[error("module merge error: {0}")]
    Merge(String),
    #[error("private declaration '{name}' is not exported by module '{module}'")]
    Private { module: String, name: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInterface {
    pub language: String,
    pub fingerprint: String,
    pub exports: BTreeMap<String, Value>,
}

pub fn load_module(path: impl AsRef<Path>) -> Result<Program, ModuleError> {
    let path = fs::canonicalize(path.as_ref()).map_err(|source| ModuleError::Io {
        path: path.as_ref().into(),
        source,
    })?;
    let mut stack = Vec::new();
    let program = load_recursive(&path, &mut stack)?;
    check_program(&program).map_err(|error| ModuleError::Parse {
        path,
        message: error.to_string(),
    })?;
    Ok(program)
}

fn load_recursive(path: &Path, stack: &mut Vec<PathBuf>) -> Result<Program, ModuleError> {
    if let Some(index) = stack.iter().position(|entry| entry == path) {
        let mut cycle = stack[index..]
            .iter()
            .map(|entry| entry.display().to_string())
            .collect::<Vec<_>>();
        cycle.push(path.display().to_string());
        return Err(ModuleError::Cycle(cycle.join(" -> ")));
    }
    stack.push(path.into());
    let source = fs::read_to_string(path).map_err(|source| ModuleError::Io {
        path: path.into(),
        source,
    })?;
    let mut program = parse_program(&source).map_err(|error| ModuleError::Parse {
        path: path.into(),
        message: error.to_string(),
    })?;
    let root_references =
        qualified_references(&serde_json::to_value(&program).unwrap_or(Value::Null));
    let imports = std::mem::take(&mut program.imports);
    for (alias, relative) in imports {
        let child_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(relative);
        let child_path = fs::canonicalize(&child_path).map_err(|source| ModuleError::Io {
            path: child_path,
            source,
        })?;
        let child = load_recursive(&child_path, stack)?;
        for call in root_references
            .iter()
            .filter(|name| name.starts_with(&format!("{alias}::")))
        {
            let local = call.trim_start_matches(&format!("{alias}::"));
            if !child.public.contains(local) {
                return Err(ModuleError::Private {
                    module: alias.clone(),
                    name: local.into(),
                });
            }
        }
        merge(&mut program, qualify(child, &alias))?;
    }
    stack.pop();
    crate::parser::resolve_loaded_shorthand(&mut program).map_err(|error| ModuleError::Parse {
        path: path.into(),
        message: error.to_string(),
    })?;
    write_interface(path, &program)?;
    Ok(program)
}

fn qualify(mut program: Program, prefix: &str) -> Program {
    let qualify_name = |name: &str| format!("{prefix}::{name}");
    rewrite_program(&mut program, &qualify_name);
    program.public = program
        .public
        .into_iter()
        .map(|name| qualify_name(&name))
        .collect();
    program
}

fn rewrite_program(program: &mut Program, rename: &impl Fn(&str) -> String) {
    program.agents = take_map(&mut program.agents, rename, |mut value| {
        value.name = rename(&value.name);
        value.tools.iter_mut().for_each(|name| *name = rename(name));
        value
    });
    program.tools = take_map(&mut program.tools, rename, |mut value| {
        value.name = rename(&value.name);
        value
            .params
            .iter_mut()
            .for_each(|p| rewrite_type(&mut p.ty, rename));
        rewrite_type(&mut value.return_type, rename);
        value
    });
    program.tasks = take_map(&mut program.tasks, rename, |mut value| {
        value.name = rename(&value.name);
        value
            .params
            .iter_mut()
            .for_each(|p| rewrite_type(&mut p.ty, rename));
        rewrite_type(&mut value.return_type, rename);
        value
    });
    program.pipelines = take_map(&mut program.pipelines, rename, |mut value| {
        value.name = rename(&value.name);
        value
            .params
            .iter_mut()
            .for_each(|p| rewrite_type(&mut p.ty, rename));
        rewrite_type(&mut value.return_type, rename);
        rewrite_statements(&mut value.statements, rename);
        value
    });
    program.aliases = take_types(&mut program.aliases, rename);
    program.records = take_map(&mut program.records, rename, |mut value| {
        value.name = rename(&value.name);
        value
            .fields
            .values_mut()
            .for_each(|ty| rewrite_type(ty, rename));
        value
    });
    program.unions = take_map(&mut program.unions, rename, |mut value| {
        value.name = rename(&value.name);
        value
            .variants
            .values_mut()
            .flat_map(|fields| fields.values_mut())
            .for_each(|ty| rewrite_type(ty, rename));
        value
    });
    program.enums = std::mem::take(&mut program.enums)
        .into_iter()
        .map(|(name, value)| (rename(&name), value))
        .collect();
    program.evals = take_map(&mut program.evals, rename, |mut value| {
        value.name = rename(&value.name);
        value.pipeline = rename(&value.pipeline);
        value.semantic_grader = value.semantic_grader.map(|name| rename(&name));
        value
    });
    for test in &mut program.tests {
        rewrite_statements(&mut test.statements, rename);
    }
}

fn rewrite_statements(statements: &mut [Stmt], rename: &impl Fn(&str) -> String) {
    for statement in statements {
        match statement {
            Stmt::Run(run) => rewrite_run(run, rename),
            Stmt::Parallel { branches, .. } | Stmt::Race { branches, .. } => {
                branches.iter_mut().for_each(|run| rewrite_run(run, rename))
            }
            Stmt::ParallelMap { run, .. } => rewrite_run(run, rename),
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
                rewrite_statements(then_body, rename);
                rewrite_statements(else_body, rename);
            }
            Stmt::While { body, .. } => rewrite_statements(body, rename),
            Stmt::TryCatch {
                try_body,
                catch_body,
                ..
            } => {
                rewrite_statements(try_body, rename);
                rewrite_statements(catch_body, rename);
            }
            Stmt::Match { arms, .. } => {
                for arm in arms {
                    arm.type_name = rename(&arm.type_name);
                    rewrite_statements(&mut arm.body, rename);
                }
            }
            _ => {}
        }
    }
}
fn rewrite_run(run: &mut RunStmt, rename: &impl Fn(&str) -> String) {
    run.callable = rename(&run.callable);
    run.agent = run.agent.take().map(|name| rename(&name));
}
fn rewrite_type(ty: &mut TypeExpr, rename: &impl Fn(&str) -> String) {
    match ty {
        TypeExpr::List(inner) | TypeExpr::Option(inner) => rewrite_type(inner, rename),
        TypeExpr::Result(ok, error) => {
            rewrite_type(ok, rename);
            rewrite_type(error, rename);
        }
        TypeExpr::Obj(fields) => fields.values_mut().for_each(|ty| rewrite_type(ty, rename)),
        TypeExpr::Record(name)
        | TypeExpr::Union(name)
        | TypeExpr::Enum(name)
        | TypeExpr::Alias(name)
            if name != "<null>" =>
        {
            *name = rename(name)
        }
        _ => {}
    }
}
fn take_map<T>(
    map: &mut BTreeMap<String, T>,
    rename: &impl Fn(&str) -> String,
    transform: impl Fn(T) -> T,
) -> BTreeMap<String, T> {
    std::mem::take(map)
        .into_iter()
        .map(|(name, value)| (rename(&name), transform(value)))
        .collect()
}

// A small explicit helper avoids requiring every declaration to implement a naming trait.
fn take_types(
    map: &mut BTreeMap<String, TypeExpr>,
    rename: &impl Fn(&str) -> String,
) -> BTreeMap<String, TypeExpr> {
    std::mem::take(map)
        .into_iter()
        .map(|(name, mut ty)| {
            rewrite_type(&mut ty, rename);
            (rename(&name), ty)
        })
        .collect()
}

fn merge(target: &mut Program, source: Program) -> Result<(), ModuleError> {
    macro_rules! merge_map {
        ($field:ident) => {
            for (name, value) in source.$field {
                if target.$field.insert(name.clone(), value).is_some() {
                    return Err(ModuleError::Merge(format!(
                        "duplicate qualified declaration '{name}'"
                    )));
                }
            }
        };
    }
    merge_map!(agents);
    merge_map!(tools);
    merge_map!(tasks);
    merge_map!(pipelines);
    merge_map!(aliases);
    merge_map!(records);
    merge_map!(unions);
    merge_map!(enums);
    merge_map!(evals);
    target.public.extend(source.public);
    target.tests.extend(source.tests);
    Ok(())
}

fn qualified_references(value: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    fn visit(value: &Value, out: &mut BTreeSet<String>) {
        match value {
            Value::String(text) if text.contains("::") => {
                out.insert(text.clone());
            }
            Value::Array(items) => {
                for item in items {
                    visit(item, out);
                }
            }
            Value::Object(fields) => {
                for value in fields.values() {
                    visit(value, out);
                }
            }
            _ => {}
        }
    }
    visit(value, &mut out);
    out
}
fn write_interface(path: &Path, program: &Program) -> Result<(), ModuleError> {
    let mut exports = BTreeMap::new();
    let serialized = serde_json::to_value(program).unwrap_or(Value::Null);
    for name in &program.public {
        exports.insert(name.clone(), declaration_json(&serialized, name));
    }
    let bytes = serde_json::to_vec(&exports).unwrap_or_default();
    let interface = ModuleInterface {
        language: program.language_version.clone(),
        fingerprint: crate::runtime::content_fingerprint(&bytes),
        exports,
    };
    let cache = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".agl-cache");
    fs::create_dir_all(&cache).map_err(|source| ModuleError::Io {
        path: cache.clone(),
        source,
    })?;
    let target = cache.join(format!(
        "{}.json",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("module")
    ));
    fs::write(&target, serde_json::to_vec_pretty(&interface).unwrap()).map_err(|source| {
        ModuleError::Io {
            path: target,
            source,
        }
    })
}
fn declaration_json(program: &Value, name: &str) -> Value {
    for field in [
        "agents",
        "tools",
        "tasks",
        "pipelines",
        "aliases",
        "records",
        "unions",
        "enums",
        "evals",
    ] {
        if let Some(value) = program.get(field).and_then(|map| map.get(name)) {
            return value.clone();
        }
    }
    Value::Null
}
