//! Security and container breakout containment integration tests for DockerSandbox.

use cortex_core::CortexError;
use cortex_runtime::sandbox::{
    DockerSandbox, DockerSandboxConfig, HostSandbox, NetworkIsolationPolicy, Sandbox, SandboxMode,
};
use std::fs;
use std::path::PathBuf;

fn create_temp_workspace(name: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!(
        "cortex_sandbox_test_{}_{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();

    let outside = std::env::temp_dir().join(format!(
        "cortex_outside_test_{}_{}",
        std::process::id(),
        name
    ));
    let _ = fs::remove_dir_all(&outside);
    fs::create_dir_all(&outside).unwrap();

    (base, outside)
}

#[test]
fn test_docker_sandbox_workspace_containment_and_traversal_prevention() {
    let (workspace, outside) = create_temp_workspace("containment");

    let sub_dir = workspace.join("src").join("nested");
    fs::create_dir_all(&sub_dir).unwrap();

    let config = DockerSandboxConfig::new(&workspace)
        .with_image("alpine:3.19")
        .with_network_policy(NetworkIsolationPolicy::Offline);

    let sandbox = DockerSandbox::new(config);
    assert_eq!(sandbox.mode(), SandboxMode::Container);

    // 1. Valid subdirectory inside workspace mounts correctly
    let valid_args = sandbox
        .build_docker_args("echo 1", Some(&sub_dir))
        .expect("nested subdir inside workspace must succeed");
    assert!(valid_args.contains(&"/workspace/src/nested".to_string()));

    // 2. Traversal attempt via relative path "../" escaping root must be rejected
    let relative_escape = workspace.join("..").join("forbidden");
    let rel_err = sandbox.build_docker_args("cat /etc/shadow", Some(&relative_escape));
    assert!(rel_err.is_err());
    assert!(matches!(
        rel_err.unwrap_err(),
        CortexError::PermissionDenied(_)
    ));

    // 3. Absolute path pointing to outside directory must be rejected
    let abs_err = sandbox.build_docker_args("cat secret.txt", Some(&outside));
    assert!(abs_err.is_err());
    assert!(matches!(
        abs_err.unwrap_err(),
        CortexError::PermissionDenied(_)
    ));

    let _ = fs::remove_dir_all(&workspace);
    let _ = fs::remove_dir_all(&outside);
}

#[test]
fn test_docker_sandbox_resource_caps_and_security_flags() {
    let (workspace, _) = create_temp_workspace("security_caps");

    let config = DockerSandboxConfig::new(&workspace)
        .with_image("debian:bookworm-slim")
        .with_memory_mb(512)
        .with_cpu_quota(1.5)
        .with_max_pids(75);

    let sandbox = DockerSandbox::new(config);
    let args = sandbox.build_docker_args("ls -la", None).unwrap();

    // Verify capability dropping
    let cap_drop_idx = args.iter().position(|a| a == "--cap-drop").unwrap();
    assert_eq!(args[cap_drop_idx + 1], "ALL");

    // Verify no-new-privileges
    let priv_idx = args.iter().position(|a| a == "--security-opt").unwrap();
    assert_eq!(args[priv_idx + 1], "no-new-privileges:true");

    // Verify resource limits
    let mem_idx = args.iter().position(|a| a == "--memory").unwrap();
    assert_eq!(args[mem_idx + 1], format!("{}b", 512 * 1024 * 1024));

    let cpu_idx = args.iter().position(|a| a == "--cpus").unwrap();
    assert_eq!(args[cpu_idx + 1], "1.50");

    let pids_idx = args.iter().position(|a| a == "--pids-limit").unwrap();
    assert_eq!(args[pids_idx + 1], "75");

    // Verify read-only root filesystem with ephemeral /tmp
    assert!(args.contains(&"--read-only".to_string()));
    assert!(args.contains(&"--tmpfs".to_string()));

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn test_docker_sandbox_network_isolation_modes() {
    let (workspace, _) = create_temp_workspace("network");

    // 1. Offline mode
    let cfg_offline =
        DockerSandboxConfig::new(&workspace).with_network_policy(NetworkIsolationPolicy::Offline);
    let sb_offline = DockerSandbox::new(cfg_offline);
    let args_off = sb_offline
        .build_docker_args("curl google.com", None)
        .unwrap();
    let net_idx = args_off.iter().position(|a| a == "--network").unwrap();
    assert_eq!(args_off[net_idx + 1], "none");

    // 2. Intranet-only mode
    let cfg_intra = DockerSandboxConfig::new(&workspace)
        .with_network_policy(NetworkIsolationPolicy::IntranetOnly);
    let sb_intra = DockerSandbox::new(cfg_intra);
    let args_intra = sb_intra.build_docker_args("curl internal", None).unwrap();
    let net_idx_intra = args_intra.iter().position(|a| a == "--network").unwrap();
    assert_eq!(args_intra[net_idx_intra + 1], "internal");

    // 3. Full internet mode
    let cfg_full = DockerSandboxConfig::new(&workspace)
        .with_network_policy(NetworkIsolationPolicy::FullInternet);
    let sb_full = DockerSandbox::new(cfg_full);
    let args_full = sb_full.build_docker_args("curl api.com", None).unwrap();
    let net_idx_full = args_full.iter().position(|a| a == "--network").unwrap();
    assert_eq!(args_full[net_idx_full + 1], "bridge");

    let _ = fs::remove_dir_all(&workspace);
}

#[test]
fn test_host_sandbox_fallback_and_containment() {
    let (workspace, outside) = create_temp_workspace("host_sb");

    let sandbox = HostSandbox::new(&workspace);
    assert_eq!(sandbox.mode(), SandboxMode::Host);
    assert!(sandbox.is_healthy().unwrap());

    // Valid inside
    let inner = workspace.join("nested");
    fs::create_dir_all(&inner).unwrap();
    assert!(sandbox.validate_path(&inner).is_ok());

    // Invalid outside
    assert!(sandbox.validate_path(&outside).is_err());

    // Simple execution
    let res = sandbox.execute("echo 'hello sandbox'", None).unwrap();
    assert_eq!(res.exit_code, 0);
    assert!(res.stdout.contains("hello sandbox"));

    let _ = fs::remove_dir_all(&workspace);
    let _ = fs::remove_dir_all(&outside);
}
