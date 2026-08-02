//! Provider-neutral deployment bindings for source-level agent requirements.

use crate::ast::{AgentDeployment, AgentRequirements, Program};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Clone, Deserialize)]
pub struct DeploymentConfig {
    pub agents: BTreeMap<String, AgentDeployment>,
}

#[derive(Debug, Error)]
pub enum DeploymentError {
    #[error("failed to read deployment configuration: {0}")]
    Read(#[from] std::io::Error),
    #[error("invalid deployment JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("deployment has no binding for agent '{0}'")]
    MissingAgent(String),
    #[error("deployment references unknown agent '{0}'")]
    UnknownAgent(String),
    #[error(
        "agent '{agent}' is bound to provider '{actual}', but adapter '{expected}' was selected"
    )]
    Provider {
        agent: String,
        expected: String,
        actual: String,
    },
    #[error("agent '{agent}' binding is missing capabilities {missing:?}")]
    Capabilities { agent: String, missing: Vec<String> },
    #[error("agent '{agent}' requires context >= {required}, binding provides {actual:?}")]
    Context {
        agent: String,
        required: u64,
        actual: Option<u64>,
    },
    #[error("agent '{agent}' requires latency <= {required}ms, binding provides {actual:?}ms")]
    Latency {
        agent: String,
        required: u64,
        actual: Option<u64>,
    },
    #[error("agent '{agent}' requires quality '{required}', binding provides {actual:?}")]
    Quality {
        agent: String,
        required: String,
        actual: Option<String>,
    },
}

impl DeploymentConfig {
    pub fn from_json(source: &str) -> Result<Self, DeploymentError> {
        Ok(serde_json::from_str(source)?)
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self, DeploymentError> {
        Self::from_json(&std::fs::read_to_string(path)?)
    }

    pub fn apply(
        &self,
        program: &mut Program,
        selected_provider: Option<&str>,
    ) -> Result<(), DeploymentError> {
        for name in self.agents.keys() {
            if !program.agents.contains_key(name) {
                return Err(DeploymentError::UnknownAgent(name.clone()));
            }
        }
        for (name, agent) in &mut program.agents {
            let binding = self
                .agents
                .get(name)
                .ok_or_else(|| DeploymentError::MissingAgent(name.clone()))?;
            validate(name, &agent.requirements, binding, selected_provider)?;
            agent.deployment = Some(binding.clone());
        }
        Ok(())
    }
}

fn validate(
    name: &str,
    requirements: &AgentRequirements,
    binding: &AgentDeployment,
    selected_provider: Option<&str>,
) -> Result<(), DeploymentError> {
    if let Some(expected) = selected_provider
        && binding.provider != expected
    {
        return Err(DeploymentError::Provider {
            agent: name.into(),
            expected: expected.into(),
            actual: binding.provider.clone(),
        });
    }
    let missing: Vec<_> = requirements
        .capabilities
        .difference(&binding.capabilities)
        .cloned()
        .collect();
    if !missing.is_empty() {
        return Err(DeploymentError::Capabilities {
            agent: name.into(),
            missing,
        });
    }
    if let Some(required) = requirements.min_context
        && binding
            .context_window
            .is_none_or(|actual| actual < required)
    {
        return Err(DeploymentError::Context {
            agent: name.into(),
            required,
            actual: binding.context_window,
        });
    }
    if let Some(required) = requirements.max_latency_ms
        && binding
            .expected_latency_ms
            .is_none_or(|actual| actual > required)
    {
        return Err(DeploymentError::Latency {
            agent: name.into(),
            required,
            actual: binding.expected_latency_ms,
        });
    }
    if let Some(required) = &requirements.quality
        && binding
            .quality
            .as_ref()
            .is_none_or(|actual| quality_rank(actual) < quality_rank(required))
    {
        return Err(DeploymentError::Quality {
            agent: name.into(),
            required: required.clone(),
            actual: binding.quality.clone(),
        });
    }
    Ok(())
}

fn quality_rank(value: &str) -> u8 {
    match value {
        "low" => 1,
        "medium" => 2,
        "high" => 3,
        "frontier" => 4,
        _ => 0,
    }
}

pub fn required_capabilities(program: &Program) -> BTreeMap<String, BTreeSet<String>> {
    program
        .agents
        .iter()
        .map(|(name, agent)| (name.clone(), agent.requirements.capabilities.clone()))
        .collect()
}
