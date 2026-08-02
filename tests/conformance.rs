use agl::context::ExecutionContext;
use agl::{Registry, check_program, execute_pipeline, format_pipeline, parse_program};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct Case {
    source: String,
    phase: String,
    code: Option<String>,
    pipeline: Option<String>,
    input: Option<Value>,
    expected: Option<Value>,
    contains: Option<String>,
}

#[test]
fn language_conformance_fixtures() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/conformance");
    let mut manifests = Vec::new();
    collect_manifests(&root, &mut manifests);
    manifests.sort();
    assert!(
        manifests.len() >= 8,
        "expected a representative fixture set"
    );
    for manifest in manifests {
        run_case(&root, &manifest);
    }
}

fn collect_manifests(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_manifests(&path, output);
        } else if path.extension().and_then(|value| value.to_str()) == Some("json") {
            output.push(path);
        }
    }
}

fn run_case(root: &Path, manifest: &Path) {
    let raw = fs::read_to_string(manifest).unwrap();
    let case: Case = serde_json::from_str(&raw).unwrap();
    let source = fs::read_to_string(root.join(&case.source)).unwrap();
    let label = manifest.strip_prefix(root).unwrap().display();

    let program = match parse_program(&source) {
        Ok(program) => program,
        Err(error) => {
            assert_eq!(case.phase, "reject", "{label}: unexpected parse failure");
            assert_eq!(case.code.as_deref(), Some(error.code()), "{label}");
            return;
        }
    };
    if let Err(error) = check_program(&program) {
        assert_eq!(case.phase, "reject", "{label}: unexpected check failure");
        assert_eq!(case.code.as_deref(), Some(error.code()), "{label}");
        return;
    }

    match case.phase.as_str() {
        "check" => {}
        "execute" => {
            let input = case.input.unwrap_or_else(|| serde_json::json!({}));
            let inputs: BTreeMap<_, _> = input
                .as_object()
                .expect("fixture input must be an object")
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            let result = execute_pipeline(
                &program,
                case.pipeline.as_deref().unwrap(),
                inputs,
                &Registry::default(),
                &ExecutionContext::default(),
            )
            .unwrap_or_else(|error| panic!("{label}: unexpected runtime failure: {error}"));
            assert_eq!(Some(result), case.expected, "{label}");
        }
        "execute_error" => {
            let error = execute_pipeline(
                &program,
                case.pipeline.as_deref().unwrap(),
                BTreeMap::new(),
                &Registry::default(),
                &ExecutionContext::default(),
            )
            .unwrap_err();
            assert_eq!(case.code.as_deref(), Some(error.code()), "{label}");
        }
        "lower" => {
            let pipeline = &program.pipelines[case.pipeline.as_deref().unwrap()];
            let lowered = format_pipeline(pipeline);
            assert!(
                lowered.contains(case.contains.as_deref().unwrap()),
                "{label}: lowered output was:\n{lowered}"
            );
        }
        "reject" => panic!("{label}: fixture expected rejection but was accepted"),
        phase => panic!("{label}: unknown fixture phase {phase:?}"),
    }
}
