use agl::context::ExecutionContext;
use agl::modules::{ModuleError, load_module};
use agl::{Registry, execute_pipeline};
use agl::{check_program, format_program, parse_program};

#[test]
fn modules_resolve_qualified_public_names_and_cache_interfaces() {
    let root = std::env::temp_dir().join(format!("agl-modules-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("util.agent"),
        r#"
        language "0.6";
        public task greet(name: String) -> String idempotency pure {}
        private task hidden() -> String idempotency pure {}
    "#,
    )
    .unwrap();
    std::fs::write(
        root.join("main.agent"),
        r#"
        language "0.6";
        import util from "util.agent";
        public pipeline main(name: String) -> String {
          let greeting = util::greet(name);
          return greeting;
        }
    "#,
    )
    .unwrap();
    let program = load_module(root.join("main.agent")).unwrap();
    assert!(program.tasks.contains_key("util::greet"));
    assert!(program.tasks.contains_key("util::hidden"));
    assert!(root.join(".agl-cache/util.agent.json").exists());
    let mut registry = Registry::default();
    registry.register("util::greet", |args, _| {
        Ok(format!("hello {}", args["name"].as_str().unwrap()).into())
    });
    let value = execute_pipeline(
        &program,
        "main",
        [("name".into(), "Ada".into())].into_iter().collect(),
        &registry,
        &ExecutionContext::deterministic(1),
    )
    .unwrap();
    assert_eq!(value, "hello Ada");

    std::fs::write(
        root.join("private.agent"),
        r#"
        language "0.6";
        import util from "util.agent";
        public pipeline main() -> String {
          let value = util::hidden();
          return value;
        }
    "#,
    )
    .unwrap();
    assert!(matches!(
        load_module(root.join("private.agent")),
        Err(ModuleError::Private { .. })
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn package_lock_api_docs_formatter_and_protocol_are_deterministic() {
    let root = std::env::temp_dir().join(format!("agl-package-{}", std::process::id()));
    std::fs::create_dir_all(root.join("dep")).unwrap();
    std::fs::write(
        root.join("dep/lib.agent"),
        "language \"0.6\"; public task dep() -> String idempotency pure {}",
    )
    .unwrap();
    std::fs::write(
        root.join("agl.json"),
        r#"{
      "name":"example","version":"1.2.3","language":"0.6","entry":"main.agent",
      "dependencies":{"dep":{"kind":"local","path":"dep"}}
    }"#,
    )
    .unwrap();
    let first = agl::package::lock_manifest(root.join("agl.json")).unwrap();
    let second = agl::package::lock_manifest(root.join("agl.json")).unwrap();
    assert_eq!(first, second);
    agl::package::verify_lock(&first).unwrap();
    assert!(first.dependencies["dep"].integrity.starts_with("fnv1a64:"));

    let source = r#"
      language "0.6";
      public task greet(name: String) -> String effects [network] idempotency idempotent {}
      public pipeline main(name: String) -> String effects [network] {
        let value = greet(name);
        return value;
      }
    "#;
    let program = parse_program(source).unwrap();
    check_program(&program).unwrap();
    let formatted = format_program(&program);
    check_program(&parse_program(&formatted).unwrap()).unwrap();
    let api = agl::documentation::api_interface(&program);
    assert_eq!(api.exports["main"].effects, vec!["network"]);
    assert!(agl::documentation::markdown(&program, "Example").contains("`main`"));
    let mut changed = api.clone();
    changed.exports.remove("greet");
    assert!(!agl::documentation::compare_api(&api, &changed).compatible);
    let report = agl::documentation::compare_api(&api, &changed);
    assert!(agl::documentation::validate_semver_upgrade("1.2.3", "1.3.0", &report).is_err());
    assert!(agl::documentation::validate_semver_upgrade("1.2.3", "2.0.0", &report).is_ok());

    let response = agl::tooling::handle(agl::tooling::CompilerRequest {
        id: 1.into(),
        method: "completion".into(),
        source: source.into(),
        pipeline: None,
        symbol: None,
        replacement: None,
    });
    assert!(
        response.ok
            && response
                .result
                .as_array()
                .unwrap()
                .iter()
                .any(|value| value == "main")
    );
    assert!(
        agl::tooling::shell_completion("zsh")
            .unwrap()
            .contains("--check")
    );
    assert!(
        agl::extension::ExtensionDescriptor {
            api_version: agl::extension::EXTENSION_API_VERSION,
            name: "test".into(),
            version: "1.0.0".into()
        }
        .validate()
        .is_ok()
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn deployment_policy_denies_undeclared_capability_boundaries() {
    let program = parse_program(
        r#"
      language "0.6";
      public task publish() -> String effects [external_write] idempotency idempotent {}
      public pipeline release() -> String {
        let value = publish();
        return value;
      }
    "#,
    )
    .unwrap();
    let policy = agl::policy::DeploymentPolicy {
        allowed_effects: Some(["external_write".into()].into_iter().collect()),
        require_approval_for_external_write: true,
        ..Default::default()
    };
    assert!(policy.validate_program(&program).is_err());
    assert_eq!(
        agl::policy::static_summary(&program)["release"]["external_write"],
        true
    );
}

#[test]
fn module_cycles_are_diagnostic() {
    let root = std::env::temp_dir().join(format!("agl-cycle-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("a.agent"),
        "language \"0.6\"; import b from \"b.agent\";",
    )
    .unwrap();
    std::fs::write(
        root.join("b.agent"),
        "language \"0.6\"; import a from \"a.agent\";",
    )
    .unwrap();
    assert!(matches!(
        load_module(root.join("a.agent")),
        Err(ModuleError::Cycle(_))
    ));
    std::fs::remove_dir_all(root).unwrap();
}
