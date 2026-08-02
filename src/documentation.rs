use crate::ast::*;
use crate::checker::infer_program_effects;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiInterface {
    pub language: String,
    pub exports: BTreeMap<String, ApiDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ApiDeclaration {
    pub kind: String,
    pub signature: Value,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompatibilityReport {
    pub compatible: bool,
    pub breaking: Vec<String>,
    pub additions: Vec<String>,
}

pub fn api_interface(program: &Program) -> ApiInterface {
    let inferred = infer_program_effects(program);
    let mut exports = BTreeMap::new();
    for name in &program.public {
        let declaration = if let Some(value) = program.tasks.get(name) {
            Some(ApiDeclaration {
                kind: "task".into(),
                signature: json!({"params":value.params,"return":value.return_type,"idempotency":value.idempotency}),
                effects: value.effects.iter().cloned().collect(),
            })
        } else if let Some(value) = program.tools.get(name) {
            Some(ApiDeclaration {
                kind: "tool".into(),
                signature: json!({"params":value.params,"return":value.return_type,"idempotency":value.idempotency}),
                effects: value.effects.iter().cloned().collect(),
            })
        } else if let Some(value) = program.pipelines.get(name) {
            Some(ApiDeclaration {
                kind: "pipeline".into(),
                signature: json!({"params":value.params,"return":value.return_type,"budget":value.budget}),
                effects: inferred.get(name).into_iter().flatten().cloned().collect(),
            })
        } else if let Some(value) = program.records.get(name) {
            Some(ApiDeclaration {
                kind: "record".into(),
                signature: json!(value.fields),
                effects: vec![],
            })
        } else if let Some(value) = program.unions.get(name) {
            Some(ApiDeclaration {
                kind: "union".into(),
                signature: json!(value.variants),
                effects: vec![],
            })
        } else if let Some(value) = program.enums.get(name) {
            Some(ApiDeclaration {
                kind: "enum".into(),
                signature: json!(value),
                effects: vec![],
            })
        } else if let Some(value) = program.aliases.get(name) {
            Some(ApiDeclaration {
                kind: "type".into(),
                signature: json!(value),
                effects: vec![],
            })
        } else {
            program.agents.get(name).map(|value| ApiDeclaration {
                kind: "agent".into(),
                signature: json!({"tools":value.tools,"requirements":value.requirements}),
                effects: vec!["model".into()],
            })
        };
        if let Some(declaration) = declaration {
            exports.insert(name.clone(), declaration);
        }
    }
    ApiInterface {
        language: program.language_version.clone(),
        exports,
    }
}

pub fn compare_api(previous: &ApiInterface, current: &ApiInterface) -> CompatibilityReport {
    let mut breaking = Vec::new();
    let mut additions = Vec::new();
    for (name, declaration) in &previous.exports {
        match current.exports.get(name) {
            None => breaking.push(format!("removed {name}")),
            Some(now) if now != declaration => breaking.push(format!("changed {name}")),
            _ => {}
        }
    }
    for name in current.exports.keys() {
        if !previous.exports.contains_key(name) {
            additions.push(format!("added {name}"));
        }
    }
    CompatibilityReport {
        compatible: breaking.is_empty(),
        breaking,
        additions,
    }
}

pub fn validate_semver_upgrade(
    previous: &str,
    current: &str,
    report: &CompatibilityReport,
) -> Result<(), String> {
    let parse = |version: &str| -> Result<(u64, u64, u64), String> {
        let values = version
            .split('.')
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "invalid semantic version")?;
        if values.len() != 3 {
            return Err("invalid semantic version".into());
        }
        Ok((values[0], values[1], values[2]))
    };
    let (old_major, old_minor, old_patch) = parse(previous)?;
    let (major, minor, patch) = parse(current)?;
    if (major, minor, patch) <= (old_major, old_minor, old_patch) {
        return Err("package version must increase".into());
    }
    if !report.compatible && major <= old_major {
        return Err("breaking API changes require a major version increment".into());
    }
    if report.compatible && !report.additions.is_empty() && major == old_major && minor <= old_minor
    {
        return Err("public API additions require a minor version increment".into());
    }
    Ok(())
}

pub fn markdown(program: &Program, title: &str) -> String {
    let interface = api_interface(program);
    let mut out = format!("# {title}\n\nLanguage: `{}`\n", interface.language);
    for (name, declaration) in interface.exports {
        out.push_str(&format!(
            "\n## `{name}`\n\nKind: `{}`\n\n",
            declaration.kind
        ));
        if !declaration.effects.is_empty() {
            out.push_str(&format!(
                "Effects: `{}`\n\n",
                declaration.effects.join(", ")
            ));
        }
        out.push_str("```json\n");
        out.push_str(&serde_json::to_string_pretty(&declaration.signature).unwrap());
        out.push_str("\n```\n");
    }
    out
}
