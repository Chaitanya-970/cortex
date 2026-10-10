use cortex_core::AgentId;
use cortex_runtime::agent::{
    AgentCapability, AgentDescriptor, AgentManager, AgentManifest, AgentModelConfig, AgentRegistry,
    AgentState,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

fn make_manifest(name: &str, role: &str, tools: Vec<String>, model: &str) -> AgentManifest {
    AgentManifest::new(
        name,
        role,
        "/tmp/workspace",
        AgentModelConfig::new("mock", model),
    )
    .with_tools(tools)
}

#[test]
fn test_registry_registration_and_lookup() {
    let registry = AgentRegistry::new();
    assert!(registry.is_empty());
    assert_eq!(registry.len(), 0);

    let id = AgentId::generate();
    let desc = AgentDescriptor::new(
        id.clone(),
        "Code Evaluator",
        "Code Reviewer",
        AgentState::Ready,
        PathBuf::from("/tmp/eval"),
    )
    .with_capability(AgentCapability::tool("read_file"))
    .with_capability(AgentCapability::tool("git_diff"))
    .with_capability(AgentCapability::domain("reviewer"))
    .with_capability(AgentCapability::model_tier("reasoning"))
    .with_tag("production");

    assert!(registry.register(desc.clone()).is_ok());
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    assert!(registry.contains(&id));

    // Duplicate registration is rejected
    let duplicate_err = registry.register(desc);
    assert!(duplicate_err.is_err());

    // Get by ID
    let retrieved = registry.get(&id).expect("descriptor should be present");
    assert_eq!(retrieved.name, "Code Evaluator");
    assert_eq!(retrieved.role, "Code Reviewer");
    assert_eq!(retrieved.status, AgentState::Ready);
    assert_eq!(retrieved.capabilities.len(), 4);
    assert_eq!(retrieved.tags, vec!["production"]);
}

#[test]
fn test_registry_query_by_role_and_tag() {
    let registry = AgentRegistry::new();

    let id1 = AgentId::generate();
    let desc1 = AgentDescriptor::new(
        id1.clone(),
        "Alice",
        "Lead Software Engineer",
        AgentState::Running,
        PathBuf::from("/ws1"),
    )
    .with_tag("backend")
    .with_tag("rust");

    let id2 = AgentId::generate();
    let desc2 = AgentDescriptor::new(
        id2.clone(),
        "Bob",
        "Security Reviewer",
        AgentState::Stopped,
        PathBuf::from("/ws2"),
    )
    .with_tag("security");

    registry.register(desc1).unwrap();
    registry.register(desc2).unwrap();

    // Query by role (exact and substring, case-insensitive)
    let engineers = registry.find_by_role("engineer");
    assert_eq!(engineers.len(), 1);
    assert_eq!(engineers[0].id, id1);

    let reviewers = registry.find_by_role("Security Reviewer");
    assert_eq!(reviewers.len(), 1);
    assert_eq!(reviewers[0].id, id2);

    let unknown_role = registry.find_by_role("Product Manager");
    assert!(unknown_role.is_empty());

    // Query by tag
    let rust_agents = registry.find_by_tag("rust");
    assert_eq!(rust_agents.len(), 1);
    assert_eq!(rust_agents[0].id, id1);

    let sec_agents = registry.find_by_tag("SECURITY");
    assert_eq!(sec_agents.len(), 1);
    assert_eq!(sec_agents[0].id, id2);
}

#[test]
fn test_registry_query_by_capability_and_wildcards() {
    let registry = AgentRegistry::new();

    let id1 = AgentId::generate();
    let desc1 = AgentDescriptor::new(
        id1.clone(),
        "Coder",
        "Software Engineer",
        AgentState::Running,
        PathBuf::from("/ws1"),
    )
    .with_capability(AgentCapability::tool("git_commit"))
    .with_capability(AgentCapability::tool("git_diff"))
    .with_capability(AgentCapability::tool("shell"))
    .with_capability(AgentCapability::model_tier("coding"))
    .with_capability(AgentCapability::domain("coder"));

    let id2 = AgentId::generate();
    let desc2 = AgentDescriptor::new(
        id2.clone(),
        "Researcher",
        "Research Specialist",
        AgentState::Ready,
        PathBuf::from("/ws2"),
    )
    .with_capability(AgentCapability::tool("read_file"))
    .with_capability(AgentCapability::model_tier("fast"))
    .with_capability(AgentCapability::domain("researcher"));

    registry.register(desc1).unwrap();
    registry.register(desc2).unwrap();

    // Wildcard tool search: git_* matches git_commit and git_diff on Coder
    let git_capable = registry.find_by_capability("git_*");
    assert_eq!(git_capable.len(), 1);
    assert_eq!(git_capable[0].id, id1);

    // Exact tool search
    let shell_capable = registry.find_by_capability("shell");
    assert_eq!(shell_capable.len(), 1);
    assert_eq!(shell_capable[0].id, id1);

    // Typed prefix search
    let tool_read = registry.find_by_capability("tool:read_file");
    assert_eq!(tool_read.len(), 1);
    assert_eq!(tool_read[0].id, id2);

    // Model tier search
    let fast_tier = registry.find_by_capability("tier:fast");
    assert_eq!(fast_tier.len(), 1);
    assert_eq!(fast_tier[0].id, id2);

    let coding_tier = registry.find_by_capability("coding");
    assert_eq!(coding_tier.len(), 1);
    assert_eq!(coding_tier[0].id, id1);

    // Domain capability search
    let researchers = registry.find_by_capability("domain:researcher");
    assert_eq!(researchers.len(), 1);
    assert_eq!(researchers[0].id, id2);
}

#[test]
fn test_registry_active_agents_filtering() {
    let registry = AgentRegistry::new();

    let id_running = AgentId::generate();
    let id_paused = AgentId::generate();
    let id_ready = AgentId::generate();
    let id_stopped = AgentId::generate();

    registry
        .register(AgentDescriptor::new(
            id_running.clone(),
            "Running Agent",
            "Worker",
            AgentState::Running,
            PathBuf::from("/ws"),
        ))
        .unwrap();

    registry
        .register(AgentDescriptor::new(
            id_paused.clone(),
            "Paused Agent",
            "Worker",
            AgentState::Paused,
            PathBuf::from("/ws"),
        ))
        .unwrap();

    registry
        .register(AgentDescriptor::new(
            id_ready.clone(),
            "Ready Agent",
            "Worker",
            AgentState::Ready,
            PathBuf::from("/ws"),
        ))
        .unwrap();

    registry
        .register(AgentDescriptor::new(
            id_stopped.clone(),
            "Stopped Agent",
            "Worker",
            AgentState::Stopped,
            PathBuf::from("/ws"),
        ))
        .unwrap();

    let active = registry.list_active();
    assert_eq!(active.len(), 2);
    let active_ids: Vec<AgentId> = active.into_iter().map(|d| d.id).collect();
    assert!(active_ids.contains(&id_running));
    assert!(active_ids.contains(&id_paused));
    assert!(!active_ids.contains(&id_ready));
    assert!(!active_ids.contains(&id_stopped));

    // Transition paused to stopped
    registry
        .update_status(&id_paused, AgentState::Stopped)
        .unwrap();
    let active_after = registry.list_active();
    assert_eq!(active_after.len(), 1);
    assert_eq!(active_after[0].id, id_running);
}

#[test]
fn test_agent_manager_registry_lifecycle_synchronization() {
    let manager = AgentManager::new();
    let registry = manager.registry();

    // 1. Create agent via AgentManager
    let manifest = make_manifest(
        "reviewer",
        "Code Reviewer",
        vec!["read_file".into(), "git_diff".into()],
        "claude-3-5-sonnet",
    );
    let agent = manager.create(manifest).expect("create agent");
    let agent_id = agent.id.clone();

    // Verify registry catalog was updated
    let desc = registry
        .get(&agent_id)
        .expect("agent should be in registry");
    assert_eq!(desc.name, "reviewer");
    assert_eq!(desc.role, "Code Reviewer");
    assert_eq!(desc.status, AgentState::Created);
    assert!(desc
        .capabilities
        .iter()
        .any(|c| c.matches("tool:read_file")));
    assert!(desc.capabilities.iter().any(|c| c.matches("tier:coding")));
    assert!(desc
        .capabilities
        .iter()
        .any(|c| c.matches("domain:reviewer")));

    // 2. Prepare agent -> Ready
    manager.prepare(&agent_id).unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Ready);

    // 3. Start agent -> Running
    manager.start(&agent_id).unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Running);
    assert_eq!(registry.list_active().len(), 1);

    // 4. Pause agent -> Paused
    manager.pause(&agent_id).unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Paused);
    assert_eq!(registry.list_active().len(), 1);

    // 5. Resume agent -> Running
    manager.resume(&agent_id).unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Running);

    // 6. Stop agent -> Stopped
    manager.stop(&agent_id).unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Stopped);
    assert!(registry.list_active().is_empty());

    // 7. Restart agent -> Ready
    manager.restart(&agent_id).unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Ready);

    // 8. Fail agent -> Failed
    manager.fail(&agent_id, "crash error").unwrap();
    assert_eq!(registry.get(&agent_id).unwrap().status, AgentState::Failed);

    // 9. Restart and remove agent
    manager.restart(&agent_id).unwrap();
    manager.stop(&agent_id).unwrap();
    manager.remove(&agent_id).unwrap();

    // Verify deregistered from registry
    assert!(registry.get(&agent_id).is_none());
    assert!(!registry.contains(&agent_id));
}

#[test]
fn test_concurrent_registry_operations() {
    let registry = Arc::new(AgentRegistry::new());
    let mut handles = Vec::new();

    // 10 concurrent threads registering agents
    for i in 0..10 {
        let reg = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            let id = AgentId::generate();
            let desc = AgentDescriptor::new(
                id.clone(),
                format!("Agent-{i}"),
                if i % 2 == 0 { "Coder" } else { "Reviewer" },
                AgentState::Running,
                PathBuf::from("/ws"),
            )
            .with_capability(AgentCapability::tool("shell"))
            .with_tag(format!("tag-{i}"));

            reg.register(desc).unwrap();
        }));
    }

    // 10 concurrent threads querying while registrations are happening
    for _ in 0..10 {
        let reg = Arc::clone(&registry);
        handles.push(thread::spawn(move || {
            for _ in 0..20 {
                let _ = reg.find_by_role("Coder");
                let _ = reg.find_by_capability("shell");
                let _ = reg.list_active();
                let _ = reg.len();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(registry.len(), 10);
    assert_eq!(registry.list_active().len(), 10);
    assert_eq!(registry.find_by_role("Coder").len(), 5);
    assert_eq!(registry.find_by_role("Reviewer").len(), 5);
}

#[test]
fn test_registry_compound_queries_and_ranking() {
    let registry = AgentRegistry::new();

    let id1 = AgentId::generate();
    let desc1 = AgentDescriptor::new(
        id1.clone(),
        "Senior Rust Engineer",
        "Lead Backend Engineer",
        AgentState::Running,
        PathBuf::from("/ws/backend"),
    )
    .with_capability(AgentCapability::tool("git_diff"))
    .with_capability(AgentCapability::tool("git_commit"))
    .with_capability(AgentCapability::tool("read_file"))
    .with_capability(AgentCapability::model_tier("coding"))
    .with_capability(AgentCapability::domain("coder"))
    .with_tag("rust")
    .with_tag("backend");

    let id2 = AgentId::generate();
    let desc2 = AgentDescriptor::new(
        id2.clone(),
        "Code Review Specialist",
        "Staff Reviewer",
        AgentState::Paused,
        PathBuf::from("/ws/review"),
    )
    .with_capability(AgentCapability::tool("git_diff"))
    .with_capability(AgentCapability::tool("read_file"))
    .with_capability(AgentCapability::model_tier("reasoning"))
    .with_capability(AgentCapability::domain("reviewer"))
    .with_tag("qa");

    let id3 = AgentId::generate();
    let desc3 = AgentDescriptor::new(
        id3.clone(),
        "Offline Security Auditor",
        "Security Lead",
        AgentState::Stopped,
        PathBuf::from("/ws/security"),
    )
    .with_capability(AgentCapability::tool("read_file"))
    .with_capability(AgentCapability::tool("shell"))
    .with_capability(AgentCapability::model_tier("reasoning"))
    .with_capability(AgentCapability::domain("reviewer"))
    .with_tag("security");

    registry.register(desc1.clone()).unwrap();
    registry.register(desc2).unwrap();
    registry.register(desc3).unwrap();

    // 1. find_by_all_capabilities (conjunction)
    let rust_coder = registry.find_by_all_capabilities(&["git_*", "domain:coder", "rust"]);
    assert_eq!(rust_coder.len(), 1);
    assert_eq!(rust_coder[0].id, id1);

    // Conjunction with model tier and tool
    let reasoning_diff = registry.find_by_all_capabilities(&["tier:reasoning", "git_diff"]);
    assert_eq!(reasoning_diff.len(), 1);
    assert_eq!(reasoning_diff[0].id, id2);

    // Conjunction that matches none
    let impossible = registry.find_by_all_capabilities(&["shell", "git_commit"]);
    assert!(impossible.is_empty());

    // 2. find_by_any_capability (disjunction)
    let commit_or_shell = registry.find_by_any_capability(&["git_commit", "shell"]);
    assert_eq!(commit_or_shell.len(), 2);
    let ids: Vec<AgentId> = commit_or_shell.into_iter().map(|d| d.id).collect();
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id3));

    // 3. rank_by_capabilities
    let queries = ["read_file", "git_diff", "tier:reasoning", "qa"];
    let ranked = registry.rank_by_capabilities(&queries);
    assert_eq!(ranked.len(), 3);
    // desc2 matches: read_file, git_diff, tier:reasoning, qa = 4 points
    assert_eq!(ranked[0].0.id, id2);
    assert_eq!(ranked[0].1, 4);

    // find_best_match selects highest ranking candidate
    let best = registry.find_best_match(&queries);
    assert_eq!(best.unwrap().id, id2);

    // 4. Availability filtering
    let active_reasoning = registry.find_active_by_capability("tier:reasoning");
    assert_eq!(active_reasoning.len(), 1);
    assert_eq!(active_reasoning[0].id, id2); // desc3 has tier:reasoning but is Stopped

    let active_all = registry.find_active_by_all_capabilities(&["read_file", "git_diff"]);
    assert_eq!(active_all.len(), 2);
    let active_ids: Vec<AgentId> = active_all.into_iter().map(|d| d.id).collect();
    assert!(active_ids.contains(&id1));
    assert!(active_ids.contains(&id2));

    let active_any = registry.find_active_by_any_capability(&["git_commit", "shell"]);
    assert_eq!(active_any.len(), 1);
    assert_eq!(active_any[0].id, id1); // desc1 (Running) has git_commit; desc3 has shell but is Stopped

    // 5. Query deduplication in scoring and ranking
    assert_eq!(
        desc1.match_score(&["git_diff", "git_diff", "GIT_DIFF", "   git_diff   "]),
        1
    );
    let dup_ranked = registry.rank_by_capabilities(&["read_file", "read_file"]);
    assert_eq!(dup_ranked[0].1, 1);
}
