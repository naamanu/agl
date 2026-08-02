use crate::ast::{Program, Stmt};
use crate::checker::infer_program_effects;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeploymentPolicy {
    #[serde(default)]
    pub allowed_effects: Option<BTreeSet<String>>,
    #[serde(default)]
    pub allowed_tools: Option<BTreeSet<String>>,
    #[serde(default)]
    pub network_hosts: BTreeSet<String>,
    #[serde(default)]
    pub filesystem_paths: BTreeSet<String>,
    #[serde(default)]
    pub require_approval_for_external_write: bool,
}
#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("policy I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("policy JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("policy denied {0}")]
    Denied(String),
}
impl DeploymentPolicy {
    pub fn load(path: impl AsRef<Path>) -> Result<Self, PolicyError> {
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }
    pub fn validate_program(&self, program: &Program) -> Result<(), PolicyError> {
        let inferred = infer_program_effects(program);
        if let Some(allowed) = &self.allowed_effects {
            for (pipeline, effects) in &inferred {
                let denied: Vec<_> = effects.difference(allowed).collect();
                if !denied.is_empty() {
                    return Err(PolicyError::Denied(format!(
                        "pipeline '{pipeline}' effects {denied:?}"
                    )));
                }
            }
        }
        if self.require_approval_for_external_write {
            for (name, pipeline) in &program.pipelines {
                if inferred[name].contains("external_write") && !has_approval(&pipeline.statements)
                {
                    return Err(PolicyError::Denied(format!(
                        "external-write pipeline '{name}' without approval"
                    )));
                }
            }
        }
        Ok(())
    }
    pub fn validate_tool(&self, name: &str, args: &Map<String, Value>) -> Result<(), PolicyError> {
        if self
            .allowed_tools
            .as_ref()
            .is_some_and(|allowed| !allowed.contains(name))
        {
            return Err(PolicyError::Denied(format!("tool '{name}'")));
        }
        if let Some(host) = args
            .get("url")
            .and_then(Value::as_str)
            .and_then(|value| url::Url::parse(value).ok())
            .and_then(|url| url.host_str().map(str::to_owned))
            && !self.network_hosts.is_empty()
            && !self.network_hosts.contains(&host)
        {
            return Err(PolicyError::Denied(format!("network host '{host}'")));
        }
        if let Some(path) = args.get("path").and_then(Value::as_str)
            && !self.filesystem_paths.is_empty()
            && !self
                .filesystem_paths
                .iter()
                .any(|root| Path::new(path).starts_with(root))
        {
            return Err(PolicyError::Denied(format!("filesystem path '{path}'")));
        }
        Ok(())
    }
}
pub fn static_summary(program: &Program) -> BTreeMap<String, Value> {
    let effects = infer_program_effects(program);
    program.pipelines.iter().map(|(name, pipeline)| (name.clone(), serde_json::json!({"effects":effects[name],"external_write":effects[name].contains("external_write"),"human_approval":has_approval(&pipeline.statements)}))).collect()
}
fn has_approval(statements: &[Stmt]) -> bool {
    statements.iter().any(|statement| match statement {
        Stmt::Approve { .. } => true,
        Stmt::If {
            then_body,
            else_body,
            ..
        }
        | Stmt::IfLet {
            then_body,
            else_body,
            ..
        } => has_approval(then_body) || has_approval(else_body),
        Stmt::While { body, .. } => has_approval(body),
        Stmt::TryCatch {
            try_body,
            catch_body,
            ..
        } => has_approval(try_body) || has_approval(catch_body),
        Stmt::Match { arms, .. } => arms.iter().any(|arm| has_approval(&arm.body)),
        _ => false,
    })
}
