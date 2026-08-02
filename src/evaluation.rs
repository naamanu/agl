use crate::ast::{EvalDef, Program};
use crate::context::ExecutionContext;
use crate::runtime::{Registry, execute_pipeline};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("evaluation I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid evaluation data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("evaluation '{0}' has no cases")]
    Empty(String),
    #[error("evaluation baseline regressed: {0}")]
    Regression(String),
}

#[derive(Debug, Clone, Deserialize)]
struct DatasetCase {
    input: Value,
    #[serde(default)]
    expected: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvalReport {
    pub name: String,
    pub cases: usize,
    pub trials: usize,
    pub passed: usize,
    pub pass_rate: f64,
    pub latency_ms: Distribution,
    pub cost_usd: Distribution,
    pub failures: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Distribution {
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub min: f64,
    pub max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalBaseline {
    pub min_pass_rate: f64,
    pub max_mean_latency_ms: f64,
    pub max_mean_cost_usd: f64,
}

impl EvalBaseline {
    pub fn from_report(report: &EvalReport) -> Self {
        Self {
            min_pass_rate: report.pass_rate,
            max_mean_latency_ms: (report.latency_ms.mean * 1.20).max(1.0),
            max_mean_cost_usd: report.cost_usd.mean * 1.05 + f64::EPSILON,
        }
    }
}

pub fn run_evaluation(
    program: &Program,
    eval: &EvalDef,
    root: &Path,
    registry: &Registry,
) -> Result<EvalReport, EvalError> {
    let raw = fs::read_to_string(root.join(&eval.dataset))?;
    let mut cases = Vec::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        cases.push(serde_json::from_str::<DatasetCase>(line)?);
    }
    if cases.is_empty() {
        return Err(EvalError::Empty(eval.name.clone()));
    }
    let mut latencies = Vec::new();
    let mut costs = Vec::new();
    let mut passed = 0;
    let mut failures = Vec::new();
    for (case_index, case) in cases.iter().enumerate() {
        for trial in 0..eval.trials {
            let context =
                ExecutionContext::deterministic(((case_index as u64) << 32) | u64::from(trial));
            let inputs = case.input.as_object().map(|object| {
                object
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect::<BTreeMap<_, _>>()
            });
            let started = Instant::now();
            let result = match inputs {
                Some(inputs) => {
                    execute_pipeline(program, &eval.pipeline, inputs, registry, &context)
                        .map_err(|error| error.to_string())
                }
                None => Err("dataset input must be an object".into()),
            };
            let latency = started.elapsed().as_secs_f64() * 1000.0;
            let cost = context
                .events()
                .iter()
                .filter(|event| event.kind == "provider_usage")
                .filter_map(|event| {
                    event
                        .fields
                        .pointer("/usage/cost_usd")
                        .and_then(Value::as_f64)
                })
                .sum::<f64>();
            latencies.push(latency);
            costs.push(cost);
            let mut reasons = Vec::new();
            match result {
                Ok(actual) => {
                    if eval.assert_expected && case.expected.as_ref() != Some(&actual) {
                        reasons.push("expected value mismatch".into());
                    }
                    if let Some(grader) = &eval.semantic_grader {
                        let grader_inputs = [
                            ("actual".into(), actual),
                            (
                                "expected".into(),
                                case.expected.clone().unwrap_or(Value::Null),
                            ),
                        ]
                        .into_iter()
                        .collect();
                        match execute_pipeline(program, grader, grader_inputs, registry, &context) {
                            Ok(Value::Bool(true)) => {}
                            Ok(_) => reasons.push("semantic grader rejected output".into()),
                            Err(error) => reasons.push(format!("semantic grader failed: {error}")),
                        }
                    }
                }
                Err(error) => reasons.push(error),
            }
            if let Some(limit) = eval.max_latency_ms
                && latency > limit as f64
            {
                reasons.push(format!("latency {latency:.3}ms exceeded {limit}ms"));
            }
            if let Some(limit) = eval.max_cost_usd
                && cost > limit
            {
                reasons.push(format!("cost ${cost:.6} exceeded ${limit:.6}"));
            }
            if reasons.is_empty() {
                passed += 1;
            } else {
                failures.push(format!(
                    "case {case_index}, trial {trial}: {}",
                    reasons.join("; ")
                ));
            }
        }
    }
    let trials = cases.len() * eval.trials as usize;
    let report = EvalReport {
        name: eval.name.clone(),
        cases: cases.len(),
        trials,
        passed,
        pass_rate: passed as f64 / trials as f64,
        latency_ms: distribution(latencies),
        cost_usd: distribution(costs),
        failures,
    };
    if let Some(path) = &eval.baseline {
        let baseline: EvalBaseline = serde_json::from_str(&fs::read_to_string(root.join(path))?)?;
        compare_baseline(&report, &baseline)?;
    }
    Ok(report)
}

pub fn compare_baseline(report: &EvalReport, baseline: &EvalBaseline) -> Result<(), EvalError> {
    let mut regressions = Vec::new();
    if report.pass_rate < baseline.min_pass_rate {
        regressions.push(format!(
            "pass rate {:.3} < {:.3}",
            report.pass_rate, baseline.min_pass_rate
        ));
    }
    if report.latency_ms.mean > baseline.max_mean_latency_ms {
        regressions.push(format!(
            "mean latency {:.3}ms > {:.3}ms",
            report.latency_ms.mean, baseline.max_mean_latency_ms
        ));
    }
    if report.cost_usd.mean > baseline.max_mean_cost_usd {
        regressions.push(format!(
            "mean cost {:.6} > {:.6}",
            report.cost_usd.mean, baseline.max_mean_cost_usd
        ));
    }
    if regressions.is_empty() {
        Ok(())
    } else {
        Err(EvalError::Regression(regressions.join(", ")))
    }
}

fn distribution(mut values: Vec<f64>) -> Distribution {
    values.sort_by(f64::total_cmp);
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let percentile = |p: f64| values[((values.len() - 1) as f64 * p).ceil() as usize];
    Distribution {
        mean,
        p50: percentile(0.50),
        p95: percentile(0.95),
        min: values[0],
        max: values[values.len() - 1],
    }
}
