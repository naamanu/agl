//! Compatibility loader for existing Python AGL task plugins.
//!
//! Calls use an isolated Python process and a small JSON protocol. Native Rust
//! applications should prefer registering handlers directly on `Registry`.

use crate::runtime::Registry;
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::Write;
use std::process::{Command, Stdio};
use thiserror::Error;

const BRIDGE: &str = r#"
import importlib
import importlib.util
import json
import sys

class Registry:
    def __init__(self):
        self.tasks = {}
        self.tools = {}
    def register_task(self, name, handler): self.tasks[name] = handler
    def register_tool(self, name, handler): self.tools[name] = handler

def load_plugin(path):
    if path.endswith('.py'):
        spec = importlib.util.spec_from_file_location('_agl_rust_plugin', path)
        if spec is None or spec.loader is None:
            raise ImportError(f"Cannot load plugin from '{path}'.")
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
    else:
        module = importlib.import_module(path)
    if not hasattr(module, 'register'):
        raise AttributeError(f"Plugin '{path}' has no register(registry) function.")
    registry = Registry()
    module.register(registry)
    return registry

mode, path = sys.argv[1], sys.argv[2]
registry = load_plugin(path)
if mode == 'manifest':
    print(json.dumps({'tasks': sorted(registry.tasks), 'tools': sorted(registry.tools)}))
else:
    name = sys.argv[3]
    payload = json.load(sys.stdin)
    print(json.dumps(registry.tasks[name](payload['args'], payload.get('agent'))))
"#;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("failed to start Python plugin bridge: {0}")]
    Start(#[from] std::io::Error),
    #[error("plugin '{plugin}' failed: {detail}")]
    Failed { plugin: String, detail: String },
    #[error("plugin '{plugin}' returned invalid JSON: {detail}")]
    InvalidJson { plugin: String, detail: String },
}

#[derive(Debug, Clone, Deserialize)]
pub struct PluginManifest {
    pub tasks: Vec<String>,
    pub tools: Vec<String>,
}

pub fn load_python_plugin(
    registry: &mut Registry,
    plugin: impl Into<String>,
) -> Result<PluginManifest, PluginError> {
    let plugin = plugin.into();
    let output = Command::new("python3")
        .args(["-c", BRIDGE, "manifest", &plugin])
        .output()?;
    if !output.status.success() {
        return Err(PluginError::Failed {
            plugin,
            detail: String::from_utf8_lossy(&output.stderr).trim().into(),
        });
    }
    let manifest: PluginManifest =
        serde_json::from_slice(&output.stdout).map_err(|e| PluginError::InvalidJson {
            plugin: plugin.clone(),
            detail: e.to_string(),
        })?;
    for task in &manifest.tasks {
        let (plugin, task) = (plugin.clone(), task.clone());
        registry.register(task.clone(), move |args, agent| {
            call_python_task(&plugin, &task, args, agent).map_err(|e| e.to_string())
        });
    }
    Ok(manifest)
}

fn call_python_task(
    plugin: &str,
    task: &str,
    args: &std::collections::BTreeMap<String, Value>,
    agent: Option<&str>,
) -> Result<Value, PluginError> {
    let mut child = Command::new("python3")
        .args(["-c", BRIDGE, "call-task", plugin, task])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    serde_json::to_writer(
        child.stdin.as_mut().expect("piped stdin"),
        &json!({"args":args,"agent":agent}),
    )
    .map_err(|e| PluginError::InvalidJson {
        plugin: plugin.into(),
        detail: e.to_string(),
    })?;
    child.stdin.take().expect("piped stdin").flush()?;
    let output = child.wait_with_output()?;
    if !output.status.success() {
        return Err(PluginError::Failed {
            plugin: plugin.into(),
            detail: String::from_utf8_lossy(&output.stderr).trim().into(),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|e| PluginError::InvalidJson {
        plugin: plugin.into(),
        detail: e.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::ExecutionContext;
    use crate::{execute_pipeline, parse_program};
    use std::collections::BTreeMap;

    #[test]
    fn loads_and_executes_existing_python_plugin() {
        let source = r#"
            task risky_enrich(topic: String) -> Obj{extra: String} {}
            pipeline main(topic: String) -> String {
              let result = risky_enrich(topic);
              return result.extra;
            }
        "#;
        let program = parse_program(source).unwrap();
        let mut registry = Registry::default();
        let manifest = load_python_plugin(&mut registry, "examples/showcase_plugin.py").unwrap();
        assert!(manifest.tasks.contains(&"risky_enrich".into()));
        let result = execute_pipeline(
            &program,
            "main",
            BTreeMap::from([("topic".into(), Value::String("Rust".into()))]),
            &registry,
            &ExecutionContext::default(),
        )
        .unwrap();
        assert_eq!(result, "[Enriched context for 'Rust']");
    }
}
