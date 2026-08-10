use super::{
    MUTABLE_KEYS, active_profile_path, apply_target_for_transition,
    bootstrap_active_generation_inner, commit_for_transition, parse_profile_for,
    prepare_for_transition, projection_bytes_for_generation, reconcile_for_transition,
    reject_non_broker_report_mutation, transaction_root_path, validate_pair_and_project_for,
};
use crate::config::{
    AudioPolicy, Config, DrainConfig, Executables, HealthConfig, IdentityConfig, Policy,
    RootConfig, ServiceConfig, SourceConfig, StorageConfig, TrustedExecutable,
};
use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

const AVADO: &[u8] = include_bytes!("../../../packaging/appliances/avado-i3-16g.env");
const ICP: &[u8] = include_bytes!("../../../packaging/appliances/icp-j3455-8g.env");

fn replace_assignment(source: &[u8], key: &str, replacement: &str) -> Vec<u8> {
    let text = std::str::from_utf8(source).unwrap();
    let mut found = false;
    let mut output = String::new();
    for line in text.lines() {
        if line.starts_with(&format!("{key}=")) {
            found = true;
            output.push_str(key);
            output.push('=');
            output.push_str(replacement);
        } else {
            output.push_str(line);
        }
        output.push('\n');
    }
    assert!(found, "fixture key absent: {key}");
    output.into_bytes()
}

#[test]
fn shipped_profiles_have_exact_distinct_schemas_and_bounded_projections() {
    let avado = parse_profile_for(false, AVADO).unwrap();
    let icp = parse_profile_for(true, ICP).unwrap();
    assert!(!avado.contains_key("ASTRID_EDGE_AUDIO_DEVICE"));
    assert_eq!(
        icp.get("ASTRID_EDGE_AUDIO_DEVICE").map(String::as_str),
        Some("off")
    );
    let avado_projection = validate_pair_and_project_for(false, &avado, &avado).unwrap();
    let icp_projection = validate_pair_and_project_for(true, &icp, &icp).unwrap();
    let avado_text = std::str::from_utf8(&avado_projection).unwrap();
    assert_eq!(avado_text.lines().count(), MUTABLE_KEYS.len());
    assert!(avado_text.contains("ASTRID_EDGE_AUTONOMY_PROMPT_MAX_CHARS=1200\n"));
    assert!(!avado_text.contains("INSTANCE_NAME"));
    assert!(!avado_text.contains("SOCKET"));
    assert!(!avado_text.contains("TUNING_ENABLED"));
    assert_ne!(avado_projection, icp_projection);
}

#[test]
fn unknown_duplicate_missing_and_shell_syntax_are_rejected() {
    let mut unknown = AVADO.to_vec();
    unknown.extend_from_slice(b"ASTRID_EDGE_SURPRISE_AUTHORITY=true\n");
    assert!(parse_profile_for(false, &unknown).is_err());

    let mut duplicate = AVADO.to_vec();
    duplicate.extend_from_slice(b"TOKIO_WORKER_THREADS=4\n");
    assert!(parse_profile_for(false, &duplicate).is_err());

    let missing = std::str::from_utf8(AVADO)
        .unwrap()
        .lines()
        .filter(|line| !line.starts_with("ASTRID_EDGE_TICK_HZ="))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(parse_profile_for(false, missing.as_bytes()).is_err());

    for unsafe_value in ["$(id)", "`id`", "x;id", "\"$HOME\""] {
        let changed = replace_assignment(AVADO, "ASTRID_OLLAMA_MODEL", unsafe_value);
        assert!(parse_profile_for(false, &changed).is_err());
    }
}

#[test]
fn identity_authority_network_path_and_safety_keys_cannot_change() {
    let prior = parse_profile_for(false, AVADO).unwrap();
    for (key, value) in [
        ("ASTRID_EDGE_INSTANCE_NAME", "\"Different Astrid\""),
        ("ASTRID_EDGE_TELEMETRY_ADDR", "127.0.0.1:7999"),
        ("ASTRID_EDGE_SOCKET", ".astrid/run/other.sock"),
        ("ASTRID_EDGE_FILL_TARGET", "0.70"),
        ("ASTRID_EDGE_RESERVOIR_TUNING_ENABLED", "true"),
        ("ASTRID_EDGE_DEDICATED_STEWARD_INTERVAL_MINUTES", "60"),
        ("ASTRID_EDGE_SELF_CHANGE_ENABLED", "true"),
        ("ASTRID_EDGE_SELF_CHANGE_ROOT", ".astrid/other"),
        ("ASTRID_EDGE_WEB_BROKER_SOCKET_PATH", "/run/other.sock"),
        ("ASTRID_OLLAMA_MODEL", "\"different:4b\""),
    ] {
        let candidate = replace_assignment(AVADO, key, value);
        let candidate = parse_profile_for(false, &candidate).unwrap();
        assert!(
            validate_pair_and_project_for(false, &candidate, &prior).is_err(),
            "protected mutation accepted: {key}"
        );
    }
}

#[test]
fn only_bounded_operational_values_enter_the_projection() {
    let prior = parse_profile_for(false, AVADO).unwrap();
    let candidate = replace_assignment(AVADO, "ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES", "15");
    let candidate = parse_profile_for(false, &candidate).unwrap();
    let projection = validate_pair_and_project_for(false, &candidate, &prior).unwrap();
    assert!(
        std::str::from_utf8(&projection)
            .unwrap()
            .contains("ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES=15\n")
    );

    for (key, value) in [
        ("TOKIO_WORKER_THREADS", "5"),
        ("ASTRID_EDGE_TICK_HZ", "1000"),
        ("ASTRID_EDGE_AUTONOMY_PROMPT_MAX_CHARS", "1201"),
        ("ASTRID_OLLAMA_CONTEXT", "8192"),
        ("ASTRID_OLLAMA_MAX_OUTPUT", "193"),
        ("ASTRID_OLLAMA_KEEP_ALIVE", "forever"),
        ("ASTRID_EDGE_SPECTRAL_ENABLED", "yes"),
    ] {
        let candidate = replace_assignment(AVADO, key, value);
        let candidate = parse_profile_for(false, &candidate).unwrap();
        assert!(
            validate_pair_and_project_for(false, &candidate, &prior).is_err(),
            "out-of-envelope value accepted: {key}={value}"
        );
    }
}

#[test]
fn packaged_reports_without_a_broker_route_are_not_mutable() {
    assert!(reject_non_broker_report_mutation(&[]).is_ok());
    assert!(
        reject_non_broker_report_mutation(&["scripts/report_edge_appliance.py".to_owned()]).is_ok()
    );
    for path in [
        "scripts/edge_hindsight.py",
        "scripts/report_edge_appliance.sh",
        "scripts/report_edge_fleet_activity.py",
    ] {
        assert!(reject_non_broker_report_mutation(&[path.to_owned()]).is_err());
    }
}

#[test]
fn profile_switch_is_atomic_and_boot_reconciliation_obeys_selected_generation() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = temp.path().canonicalize().unwrap();
    let config = fixture_config(&root);
    fs::create_dir(&config.roots.releases).unwrap();
    fs::create_dir(&config.roots.state_snapshots).unwrap();
    fs::set_permissions(
        &config.roots.state_snapshots,
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::create_dir(transaction_root_path(&config)).unwrap();
    fs::set_permissions(
        transaction_root_path(&config),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();

    let prior = generation(&config, "gen-prior", AVADO, None);
    let target_profile = replace_assignment(AVADO, "ASTRID_EDGE_AUTONOMY_INTERVAL_MINUTES", "15");
    let target = generation(&config, "gen-target", &target_profile, Some(&prior));
    let prior_projection = projection_bytes_for_generation(&config, &prior).unwrap();
    let target_projection = projection_bytes_for_generation(&config, &target).unwrap();
    fs::write(active_profile_path(&config), &prior_projection).unwrap();
    fs::set_permissions(
        active_profile_path(&config),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();

    let first = prepare_for_transition(&config, &target, &prior, false).unwrap();
    let applied = apply_target_for_transition(&config, &first, false).unwrap();
    assert_eq!(applied.generation_id, "gen-target");
    assert_eq!(
        fs::read(active_profile_path(&config)).unwrap(),
        target_projection
    );

    // Simulate power loss after the atomic profile install but before the
    // outer generation journal committed. Boot selected the prior slot.
    let reconciled = reconcile_for_transition(&config, &prior, false).unwrap();
    assert_ne!(reconciled.transaction_id, "none");
    assert_eq!(
        fs::read(active_profile_path(&config)).unwrap(),
        prior_projection
    );
    assert!(!transaction_root_path(&config).join("pending.json").exists());

    let second = prepare_for_transition(&config, &target, &prior, false).unwrap();
    apply_target_for_transition(&config, &second, false).unwrap();
    commit_for_transition(&config, &second, false).unwrap();
    assert!(!transaction_root_path(&config).join("pending.json").exists());
    assert_eq!(
        fs::read(active_profile_path(&config)).unwrap(),
        target_projection
    );
    let verified = reconcile_for_transition(&config, &target, false).unwrap();
    assert_eq!(verified.transaction_id, "none");
}

#[test]
fn untracked_active_profile_tampering_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = temp.path().canonicalize().unwrap();
    let config = fixture_config(&root);
    fs::create_dir(&config.roots.releases).unwrap();
    fs::create_dir(&config.roots.state_snapshots).unwrap();
    fs::set_permissions(
        &config.roots.state_snapshots,
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::create_dir(transaction_root_path(&config)).unwrap();
    fs::set_permissions(
        transaction_root_path(&config),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let generation = generation(&config, "gen-a", AVADO, None);
    fs::write(active_profile_path(&config), b"TOKIO_WORKER_THREADS=4\n").unwrap();
    fs::set_permissions(
        active_profile_path(&config),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    assert!(reconcile_for_transition(&config, &generation, false).is_err());
}

#[test]
fn profile_bootstrap_is_create_once_idempotent_and_never_repairs_divergence() {
    let temporary = tempfile::tempdir().unwrap();
    fs::set_permissions(temporary.path(), fs::Permissions::from_mode(0o700)).unwrap();
    let root = temporary.path().canonicalize().unwrap();
    let config = fixture_config(&root);
    fs::create_dir(&config.roots.releases).unwrap();
    fs::create_dir(&config.roots.state_snapshots).unwrap();
    fs::set_permissions(
        &config.roots.state_snapshots,
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::create_dir(transaction_root_path(&config)).unwrap();
    fs::set_permissions(
        transaction_root_path(&config),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    let generation = generation(&config, "gen-bootstrap", AVADO, None);
    let first = bootstrap_active_generation_inner(&config, &generation, false).unwrap();
    assert_eq!(first.generation_id, "gen-bootstrap");
    let expected = fs::read(active_profile_path(&config)).unwrap();
    let second = bootstrap_active_generation_inner(&config, &generation, false).unwrap();
    assert_eq!(second.active_profile_sha256, first.active_profile_sha256);
    assert_eq!(fs::read(active_profile_path(&config)).unwrap(), expected);

    fs::set_permissions(
        active_profile_path(&config),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    fs::write(active_profile_path(&config), b"divergent=true\n").unwrap();
    fs::set_permissions(
        active_profile_path(&config),
        fs::Permissions::from_mode(0o400),
    )
    .unwrap();
    assert!(bootstrap_active_generation_inner(&config, &generation, false).is_err());
    assert_eq!(
        fs::read(active_profile_path(&config)).unwrap(),
        b"divergent=true\n"
    );
}

fn generation(config: &Config, id: &str, profile: &[u8], prior: Option<&Path>) -> PathBuf {
    let root = config.roots.releases.join(id);
    fs::create_dir(&root).unwrap();
    let profile_path = root.join("packaging/appliances/avado-i3-16g.env");
    fs::create_dir_all(profile_path.parent().unwrap()).unwrap();
    fs::write(profile_path, profile).unwrap();
    for report in [
        "astrid_at_a_glance.py",
        "report_edge_activity.py",
        "report_edge_appliance.py",
    ] {
        let path = root.join("scripts").join(report);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, format!("# {report}\n")).unwrap();
    }
    let schema = if let Some(prior) = prior {
        super::write_release_projection_manifest(
            config,
            &["packaging/appliances/avado-i3-16g.env".to_owned()],
            &root,
            prior,
        )
        .unwrap();
        "astrid.edge_self_change.generation.v1"
    } else {
        "astrid.edge_self_change.initial_generation.v1"
    };
    fs::write(
        root.join(".astrid-edge-generation.json"),
        serde_json::to_vec(&serde_json::json!({"schema": schema})).unwrap(),
    )
    .unwrap();
    root
}

#[allow(clippy::too_many_lines)]
fn fixture_config(root: &Path) -> Config {
    let executable = TrustedExecutable {
        path: root.join("native"),
        sha256: "a".repeat(64),
    };
    Config {
        schema: "astrid.edge_rescue_helper.config.v1".into(),
        appliance_id: "avado-test".into(),
        target: "x86_64-unknown-linux-gnu".into(),
        model: "qwen3.5:4b".into(),
        ollama_origin: "http://127.0.0.1:11434".into(),
        source: SourceConfig {
            root: root.join("source"),
            manifest: root.join("source/manifest"),
            signature: root.join("source/signature"),
            signing_key: root.join("source-key"),
            intent_attestation_key: root.join("intent-key"),
            ledger_attestation_key: root.join("ledger-key"),
            vendor: root.join("source/vendor"),
        },
        roots: RootConfig {
            supervisor_state: root.to_path_buf(),
            candidate_store: root.join("candidates"),
            model_handoff_root: root.join("handoff"),
            model_handoff_ledger: root.join("handoff.jsonl"),
            candidate_work: root.join("work"),
            build_store: root.join("builds"),
            releases: root.join("releases"),
            active_link: root.join("current"),
            generation_binding: root.join("current-generation"),
            maintenance_lease: root.join("maintenance.json"),
            maintenance_mutex: root.join("maintenance.lock"),
            state_snapshots: root.join("snapshots"),
            workspace: root.join("workspace"),
            system_unit_root: root.join("system-units"),
            unit_policy: root.join("unit-policy.json"),
            unit_transactions: root.join("snapshots/unit-transactions"),
            candidate_sandbox_root: root.join("candidate-rootfs"),
        },
        identities: IdentityConfig {
            steward_uid: 10,
            steward_gid: 10,
            builder_uid: 11,
            builder_gid: 11,
            updater_uid: 12,
            updater_gid: 12,
            runtime_uid: 13,
            runtime_gid: 13,
        },
        executables: Executables {
            cargo: executable.clone(),
            rustc: executable.clone(),
            rustfmt: executable.clone(),
            python: executable.clone(),
            systemctl: executable.clone(),
            systemd_run: executable.clone(),
            systemd_analyze: executable.clone(),
            checkpoint: executable.clone(),
            capsule_builder: executable.clone(),
            invariant_runner: executable.clone(),
            package_verifier: executable.clone(),
            state_store: executable,
        },
        services: ServiceConfig {
            core: "astrid.service".into(),
            warmup: "astrid-model-warmup.service".into(),
            edge: "astrid-edge-runtime.service".into(),
        },
        storage: StorageConfig {
            config: root.join("storage.json"),
            config_sha256: "b".repeat(64),
            install_attestation: root.join("storage-install.json"),
            health_attestation: root.join("storage-health.json"),
            runtime_state_mount: root.join("runtime"),
            rollback_mount: root.join("snapshots"),
            backing_uuid: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
            runtime_filesystem_uuid: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into(),
            rollback_filesystem_uuid: "cccccccc-cccc-4ccc-8ccc-cccccccccccc".into(),
            image_bytes: 32 * 1024 * 1024 * 1024,
            host_reserve_bytes: 64 * 1024 * 1024 * 1024,
            store_minimum_free_bytes: 4 * 1024 * 1024 * 1024,
            emergency_inode_reserve_files: 65_536,
        },
        policy: Policy {
            maximum_files: 25,
            maximum_changed_lines: 4_000,
            build_workers: 4,
            command_timeout_seconds: 60,
            pipeline_timeout_seconds: 600,
            maximum_candidate_bytes: 4 * 1024 * 1024,
            minimum_free_disk_bytes: 1024 * 1024,
            candidate_memory_max_bytes: 4 * 1024 * 1024 * 1024,
            candidate_memory_swap_max_bytes: 128 * 1024 * 1024,
            candidate_tasks_max: 256,
            candidate_cpu_quota_percent: 400,
            network_policy: "private-network-none:v1".into(),
            dependency_policy: "signed-vendor-offline-locked:v1".into(),
        },
        drain: DrainConfig {
            autonomy_state: root.join("workspace/autonomous/state.json"),
            model_lock: root.join("model.lock"),
            model_lock_gid: 14,
            maintenance_edge_acknowledgement: root.join("workspace/edge-ack.json"),
            maintenance_core_acknowledgement: root.join("core-ack.json"),
            activity_ledgers: vec![root.join("workspace/actions/receipts.jsonl")],
            maximum_wait_seconds: 30,
            poll_milliseconds: 100,
        },
        health: HealthConfig {
            sensor_state: root.join("sensor.json"),
            hindsight_state: root.join("hindsight.json"),
            fill_history: root.join("fill.jsonl"),
            model_warmup_receipt: root.join("model-warmup-receipt.json"),
            model_warmup_uid: 15,
            meminfo: root.join("meminfo"),
            swaps: root.join("swaps"),
            thermal_celsius: root.join("thermal"),
            telemetry_addr: "127.0.0.1:7878".parse().unwrap(),
            audio_policy: AudioPolicy::RequiredFreshNumeric,
            expected_audio_source: "alsa_numeric_features".into(),
            maximum_age_seconds: 120,
            maximum_thermal_celsius: 85.0,
            minimum_available_ram_bytes: 2 * 1024 * 1024 * 1024,
            maximum_swap_bytes: 128 * 1024 * 1024,
            minimum_fill_samples: 10,
        },
    }
}
