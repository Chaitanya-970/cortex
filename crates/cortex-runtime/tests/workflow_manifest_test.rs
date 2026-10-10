//! Comprehensive integration test suite for declarative multi-agent workflow manifests (workflow.yaml).

use cortex_core::{CortexError, WorkflowAgentRef, WorkflowManifest, WorkflowStage};
use cortex_runtime::workflow::{WorkflowManifestExt, WorkflowYamlParser};
use std::fs;

const CANONICAL_WORKFLOW_YAML: &str = r#"
version: "1"
name: "code-refactor-team"
description: "Autonomous code research, implementation, and pull request review pipeline"

agents:
  - id: "manager"
    manifest: "./agents/manager.yaml"
    role: "coordinator"
  - id: "researcher"
    manifest: "./agents/researcher.yaml"
    role: "researcher"
  - id: "coder"
    manifest: "./agents/coder.yaml"
    role: "implementer"
  - id: "reviewer"
    manifest: "./agents/reviewer.yaml"
    role: "evaluator"

stages:
  - id: "research"
    agent: "researcher"
    action: "Analyze workspace and identify required modifications"
    outputs: ["research_notes"]
  - id: "implementation"
    agent: "coder"
    depends_on: ["research"]
    action: "Implement refactoring per research_notes"
    outputs: ["code_diff"]
  - id: "review"
    agent: "reviewer"
    depends_on: ["implementation"]
    action: "Review code_diff, run test suite, and approve or request revisions"
    max_iterations: 3
"#;

#[test]
fn test_canonical_workflow_yaml_parsing_and_roundtrip() {
    let manifest = WorkflowYamlParser::parse_str(CANONICAL_WORKFLOW_YAML)
        .expect("canonical workflow yaml should parse cleanly");

    assert_eq!(manifest.version, "1");
    assert_eq!(manifest.name, "code-refactor-team");
    assert_eq!(
        manifest.description.as_deref(),
        Some("Autonomous code research, implementation, and pull request review pipeline")
    );
    assert_eq!(manifest.agents.len(), 4);
    assert_eq!(manifest.stages.len(), 3);

    // Verify agents
    let manager = manifest.get_agent("manager").expect("manager agent");
    assert_eq!(manager.role, "coordinator");
    assert_eq!(manager.manifest, "./agents/manager.yaml");

    let coder = manifest.get_agent("coder").expect("coder agent");
    assert_eq!(coder.role, "implementer");

    // Verify stages
    let research = manifest.get_stage("research").expect("research stage");
    assert_eq!(research.agent, "researcher");
    assert!(research.depends_on.is_empty());
    assert_eq!(research.outputs, vec!["research_notes"]);

    let impl_stage = manifest
        .get_stage("implementation")
        .expect("implementation stage");
    assert_eq!(impl_stage.agent, "coder");
    assert_eq!(impl_stage.depends_on, vec!["research"]);
    assert_eq!(impl_stage.outputs, vec!["code_diff"]);

    let review = manifest.get_stage("review").expect("review stage");
    assert_eq!(review.agent, "reviewer");
    assert_eq!(review.depends_on, vec!["implementation"]);
    assert_eq!(review.max_iterations, Some(3));

    // Full roundtrip serialization
    let serialized = WorkflowYamlParser::to_yaml_string(&manifest).expect("serialize to yaml");
    let reparsed = WorkflowYamlParser::parse_str(&serialized).expect("reparse serialized yaml");
    assert_eq!(manifest, reparsed);

    // Trait extension roundtrip
    let via_ext = WorkflowManifest::from_yaml_str(&serialized).expect("trait parse");
    assert_eq!(manifest, via_ext);
}

#[test]
fn test_malformed_yaml_syntax_reports_line_and_column() {
    let malformed_yaml = r#"
version: "1"
name: "broken-team"
agents:
  - id: "agent_1"
    manifest: "foo.yaml"
    role: [unclosed bracket
"#;

    let err = WorkflowYamlParser::parse_str(malformed_yaml).unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("invalid workflow YAML at line"),
        "error message should contain line number diagnostic: {}",
        err_msg
    );
    assert!(
        err_msg.contains("column"),
        "error message should contain column number diagnostic: {}",
        err_msg
    );
}

#[test]
fn test_validation_schema_required_fields() {
    // Missing / empty name
    let no_name_yaml = r#"
version: "1"
name: ""
agents:
  - id: "a1"
    manifest: "a1.yaml"
    role: "worker"
stages:
  - id: "s1"
    agent: "a1"
    action: "do work"
"#;
    let err = WorkflowYamlParser::parse_str(no_name_yaml).unwrap_err();
    assert!(err.to_string().contains("name cannot be empty"));

    // Unsupported version
    let bad_version_yaml = r#"
version: "2"
name: "v2-team"
agents:
  - id: "a1"
    manifest: "a1.yaml"
    role: "worker"
stages:
  - id: "s1"
    agent: "a1"
    action: "do work"
"#;
    let err = WorkflowYamlParser::parse_str(bad_version_yaml).unwrap_err();
    assert!(err.to_string().contains("unsupported workflow version"));

    // Empty agents list
    let no_agents_yaml = r#"
version: "1"
name: "no-agents"
agents: []
stages:
  - id: "s1"
    agent: "a1"
    action: "do work"
"#;
    let err = WorkflowYamlParser::parse_str(no_agents_yaml).unwrap_err();
    assert!(err.to_string().contains("at least one agent"));

    // Empty stages list
    let no_stages_yaml = r#"
version: "1"
name: "no-stages"
agents:
  - id: "a1"
    manifest: "a1.yaml"
    role: "worker"
stages: []
"#;
    let err = WorkflowYamlParser::parse_str(no_stages_yaml).unwrap_err();
    assert!(err.to_string().contains("at least one stage"));
}

#[test]
fn test_validation_rejects_duplicate_agent_and_stage_ids() {
    // Duplicate agent ID
    let dup_agent_yaml = r#"
version: "1"
name: "dup-agent"
agents:
  - id: "worker"
    manifest: "w1.yaml"
    role: "first"
  - id: "worker"
    manifest: "w2.yaml"
    role: "second"
stages:
  - id: "s1"
    agent: "worker"
    action: "execute"
"#;
    let err = WorkflowYamlParser::parse_str(dup_agent_yaml).unwrap_err();
    assert!(err.to_string().contains("duplicate agent id 'worker'"));

    // Duplicate stage ID
    let dup_stage_yaml = r#"
version: "1"
name: "dup-stage"
agents:
  - id: "worker"
    manifest: "w.yaml"
    role: "builder"
stages:
  - id: "build"
    agent: "worker"
    action: "compile"
  - id: "build"
    agent: "worker"
    action: "re-compile"
"#;
    let err = WorkflowYamlParser::parse_str(dup_stage_yaml).unwrap_err();
    assert!(err.to_string().contains("duplicate stage id 'build'"));
}

#[test]
fn test_validation_rejects_undefined_agent_and_self_dependency() {
    // Stage referencing non-existent agent
    let undefined_agent_yaml = r#"
version: "1"
name: "bad-ref"
agents:
  - id: "planner"
    manifest: "p.yaml"
    role: "lead"
stages:
  - id: "execute"
    agent: "executor"
    action: "run"
"#;
    let err = WorkflowYamlParser::parse_str(undefined_agent_yaml).unwrap_err();
    assert!(err
        .to_string()
        .contains("references undefined agent 'executor'"));

    // Stage depending on itself
    let self_dep_yaml = r#"
version: "1"
name: "self-loop"
agents:
  - id: "worker"
    manifest: "w.yaml"
    role: "exec"
stages:
  - id: "task"
    agent: "worker"
    depends_on: ["task"]
    action: "run"
"#;
    let err = WorkflowYamlParser::parse_str(self_dep_yaml).unwrap_err();
    assert!(err.to_string().contains("cannot depend on itself"));

    // Stage depending on non-existent stage
    let missing_dep_yaml = r#"
version: "1"
name: "missing-dep"
agents:
  - id: "worker"
    manifest: "w.yaml"
    role: "exec"
stages:
  - id: "task_b"
    agent: "worker"
    depends_on: ["task_a"]
    action: "run"
"#;
    let err = WorkflowYamlParser::parse_str(missing_dep_yaml).unwrap_err();
    assert!(err
        .to_string()
        .contains("depends on unknown stage 'task_a'"));
}

#[test]
fn test_validation_rejects_cyclic_dependencies() {
    // 2-node cycle: A -> B -> A
    let two_node_cycle_yaml = r#"
version: "1"
name: "cycle-2"
agents:
  - id: "worker"
    manifest: "w.yaml"
    role: "builder"
stages:
  - id: "stage_a"
    agent: "worker"
    depends_on: ["stage_b"]
    action: "do A"
  - id: "stage_b"
    agent: "worker"
    depends_on: ["stage_a"]
    action: "do B"
"#;
    let err = WorkflowYamlParser::parse_str(two_node_cycle_yaml).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("cyclic dependency detected in workflow stages"),
        "expected cycle error, got: {}",
        msg
    );

    // 3-node cycle: research -> impl -> review -> research
    let three_node_cycle_yaml = r#"
version: "1"
name: "cycle-3"
agents:
  - id: "w1"
    manifest: "w1.yaml"
    role: "r1"
stages:
  - id: "stage_1"
    agent: "w1"
    depends_on: ["stage_3"]
    action: "first"
  - id: "stage_2"
    agent: "w1"
    depends_on: ["stage_1"]
    action: "second"
  - id: "stage_3"
    agent: "w1"
    depends_on: ["stage_2"]
    action: "third"
"#;
    let err = WorkflowYamlParser::parse_str(three_node_cycle_yaml).unwrap_err();
    assert!(err.to_string().contains("cyclic dependency detected"));
}

#[test]
fn test_topological_execution_order_and_concurrent_waves() {
    // Diamond DAG:
    //      start
    //     /     \
    //   branch1 branch2
    //     \     /
    //      join
    let diamond_yaml = r#"
version: "1"
name: "diamond-pipeline"
agents:
  - id: "agent_a"
    manifest: "a.yaml"
    role: "worker"
stages:
  - id: "start"
    agent: "agent_a"
    action: "initialize"
  - id: "branch1"
    agent: "agent_a"
    depends_on: ["start"]
    action: "process left"
  - id: "branch2"
    agent: "agent_a"
    depends_on: ["start"]
    action: "process right"
  - id: "join"
    agent: "agent_a"
    depends_on: ["branch1", "branch2"]
    action: "merge results"
"#;

    let manifest = WorkflowYamlParser::parse_str(diamond_yaml).expect("parse diamond DAG");

    let order = manifest.execution_order().expect("execution order");
    assert_eq!(order.len(), 4);
    assert_eq!(order[0], "start");
    assert_eq!(order[3], "join");
    // branch1 and branch2 can be in either order in linear list, but both before join
    let idx_b1 = order.iter().position(|x| x == "branch1").unwrap();
    let idx_b2 = order.iter().position(|x| x == "branch2").unwrap();
    let idx_join = order.iter().position(|x| x == "join").unwrap();
    assert!(idx_b1 < idx_join);
    assert!(idx_b2 < idx_join);

    // Parallel waves
    let waves = manifest.execution_waves().expect("execution waves");
    assert_eq!(waves.len(), 3);
    assert_eq!(waves[0], vec!["start"]);
    assert_eq!(waves[1], vec!["branch1", "branch2"]);
    assert_eq!(waves[2], vec!["join"]);
}

#[test]
fn test_iteration_bounds_and_resource_limits() {
    let invalid_zero_iter_yaml = r#"
version: "1"
name: "zero-iter"
agents:
  - id: "w"
    manifest: "w.yaml"
    role: "worker"
stages:
  - id: "loop_stage"
    agent: "w"
    action: "retry"
    max_iterations: 0
"#;
    let err = WorkflowYamlParser::parse_str(invalid_zero_iter_yaml).unwrap_err();
    assert!(err
        .to_string()
        .contains("max_iterations must be greater than 0"));

    let excessive_iter_yaml = r#"
version: "1"
name: "excess-iter"
agents:
  - id: "w"
    manifest: "w.yaml"
    role: "worker"
stages:
  - id: "loop_stage"
    agent: "w"
    action: "retry"
    max_iterations: 1500
"#;
    let err = WorkflowYamlParser::parse_str(excessive_iter_yaml).unwrap_err();
    assert!(err
        .to_string()
        .contains("exceeds maximum allowed limit of 1000"));
}

#[test]
fn test_workspace_resolution_verifies_referenced_agent_manifests() {
    let tmp_dir =
        std::env::temp_dir().join(format!("cortex_wf_integration_test_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(tmp_dir.join("agents")).unwrap();

    let researcher_path = tmp_dir.join("agents/researcher.yaml");
    let coder_path = tmp_dir.join("agents/coder.yaml");
    let reviewer_path = tmp_dir.join("agents/reviewer.json");

    fs::write(
        &researcher_path,
        "name: researcher\nrole: Research lead\nworkspace: .\n",
    )
    .unwrap();
    fs::write(&coder_path, "name: coder\nrole: Rust dev\nworkspace: .\n").unwrap();
    fs::write(
        &reviewer_path,
        r#"{"name": "reviewer", "role": "PR reviewer", "workspace": "."}"#,
    )
    .unwrap();

    let workflow_file = tmp_dir.join("workflow.yaml");
    let workflow_yaml = r#"
version: "1"
name: "verified-team"
description: "Workspace verified team"

agents:
  - id: "researcher"
    manifest: "./agents/researcher.yaml"
    role: "researcher"
  - id: "coder"
    manifest: "./agents/coder.yaml"
    role: "coder"
  - id: "reviewer"
    manifest: "./agents/reviewer.json"
    role: "reviewer"

stages:
  - id: "s1"
    agent: "researcher"
    action: "research"
  - id: "s2"
    agent: "coder"
    depends_on: ["s1"]
    action: "code"
  - id: "s3"
    agent: "reviewer"
    depends_on: ["s2"]
    action: "review"
"#;

    fs::write(&workflow_file, workflow_yaml).unwrap();

    // 1. Valid workspace resolution
    let manifest = WorkflowYamlParser::parse_file(&workflow_file).expect("parse workflow file");
    let res = WorkflowYamlParser::validate_workspace(&manifest, &tmp_dir);
    assert!(
        res.is_ok(),
        "expected workspace validation to succeed: {:?}",
        res
    );

    // 2. Missing agent manifest file triggers NotFound error
    let missing_agent_manifest = WorkflowManifest::new("broken-paths")
        .with_agent(WorkflowAgentRef::new(
            "ghost",
            "./agents/ghost_agent.yaml",
            "ghost",
        ))
        .with_stage(WorkflowStage::new("s_ghost", "ghost", "vanish"));

    let err =
        WorkflowYamlParser::validate_workspace(&missing_agent_manifest, &tmp_dir).unwrap_err();
    match err {
        CortexError::NotFound(msg) => {
            assert!(msg.contains("ghost_agent.yaml"));
            assert!(msg.contains("ghost"));
        }
        other => panic!("expected NotFound error, got: {:?}", other),
    }

    // 3. Empty agent manifest file triggers Validation error
    let empty_path = tmp_dir.join("agents/empty.yaml");
    fs::write(&empty_path, "   ").unwrap();
    let empty_agent_manifest = WorkflowManifest::new("empty-agent-wf")
        .with_agent(WorkflowAgentRef::new(
            "empty_agent",
            "./agents/empty.yaml",
            "empty",
        ))
        .with_stage(WorkflowStage::new("s_empty", "empty_agent", "do empty"));

    let err = WorkflowYamlParser::validate_workspace(&empty_agent_manifest, &tmp_dir).unwrap_err();
    assert!(err.to_string().contains("is empty"));

    let _ = fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_multi_format_json_and_auto_detection() {
    let tmp_dir =
        std::env::temp_dir().join(format!("cortex_wf_multiformat_{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    let manifest = WorkflowManifest::new("multiformat-team")
        .with_description("Multi-format serialization pipeline")
        .with_agent(WorkflowAgentRef::new("bot", "./bot.yaml", "automation"))
        .with_stage(WorkflowStage::new("s1", "bot", "run auto"));

    // JSON file roundtrip
    let json_file = tmp_dir.join("workflow.json");
    fs::write(&json_file, manifest.to_json_string().unwrap()).unwrap();
    let from_json = WorkflowYamlParser::parse_auto(&json_file).expect("parse json file");
    assert_eq!(manifest, from_json);

    // YAML file roundtrip
    let yaml_file = tmp_dir.join("workflow.yaml");
    fs::write(&yaml_file, manifest.to_yaml_string().unwrap()).unwrap();
    let from_yaml = WorkflowYamlParser::parse_auto(&yaml_file).expect("parse yaml file");
    assert_eq!(manifest, from_yaml);

    let _ = fs::remove_dir_all(&tmp_dir);
}
