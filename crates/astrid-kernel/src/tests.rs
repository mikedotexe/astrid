use super::*;

#[test]
fn test_load_or_generate_creates_new_key() {
    let dir = tempfile::tempdir().unwrap();
    let keys_dir = dir.path().join("keys");

    let keypair = load_or_generate_runtime_key(&keys_dir).unwrap();
    let key_path = keys_dir.join("runtime.key");

    // Key file should exist with 32 bytes.
    assert!(key_path.exists());
    let bytes = std::fs::read(&key_path).unwrap();
    assert_eq!(bytes.len(), 32);

    // The written bytes should reconstruct the same public key.
    let reloaded = KeyPair::from_secret_key(&bytes).unwrap();
    assert_eq!(
        keypair.public_key_bytes(),
        reloaded.public_key_bytes(),
        "reloaded key should match generated key"
    );
}

#[test]
fn test_load_or_generate_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let keys_dir = dir.path().join("keys");

    let first = load_or_generate_runtime_key(&keys_dir).unwrap();
    let second = load_or_generate_runtime_key(&keys_dir).unwrap();

    assert_eq!(
        first.public_key_bytes(),
        second.public_key_bytes(),
        "loading the same key file should produce the same keypair"
    );
}

#[test]
fn test_load_or_generate_rejects_bad_key_length() {
    let dir = tempfile::tempdir().unwrap();
    let keys_dir = dir.path().join("keys");
    std::fs::create_dir_all(&keys_dir).unwrap();

    // Write a key file with wrong length.
    std::fs::write(keys_dir.join("runtime.key"), [0u8; 16]).unwrap();

    let result = load_or_generate_runtime_key(&keys_dir);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("invalid runtime key"),
        "expected 'invalid runtime key' error, got: {err}"
    );
}

#[test]
fn test_connection_counter_increment_decrement() {
    let counter = AtomicUsize::new(0);

    // Simulate connection_opened (fetch_add)
    counter.fetch_add(1, Ordering::Relaxed);
    counter.fetch_add(1, Ordering::Relaxed);
    assert_eq!(counter.load(Ordering::Relaxed), 2);

    // Simulate connection_closed using the same fetch_update logic
    // as the real implementation; this tests the model, not a Kernel call.
    for expected in [1, 0] {
        let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            if n == 0 {
                None
            } else {
                Some(n.saturating_sub(1))
            }
        });
        assert_eq!(counter.load(Ordering::Relaxed), expected);
    }
}

#[test]
fn test_connection_counter_underflow_guard() {
    // Test the saturating behavior: decrementing from 0 should stay at 0.
    // Mirrors the fetch_update logic in connection_closed().
    let counter = AtomicUsize::new(0);

    let result = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        if n == 0 { None } else { Some(n - 1) }
    });
    // fetch_update returns Err(0) when the closure returns None (no-op).
    assert!(result.is_err());
    assert_eq!(counter.load(Ordering::Relaxed), 0);
}

/// Mirrors the `connection_closed()` logic: only `Ok(1)` (previous value 1,
/// now 0) triggers `clear_session_allowances`. Update this test if
/// `connection_closed()` is refactored.
#[test]
fn test_last_disconnect_clears_session_allowances() {
    use astrid_approval::AllowanceStore;
    use astrid_approval::allowance::{Allowance, AllowanceId, AllowancePattern};
    use astrid_core::types::Timestamp;
    use astrid_crypto::KeyPair;

    let store = AllowanceStore::new();
    let keypair = KeyPair::generate();

    // Session-only allowance (should be cleared on last disconnect).
    store
        .add_allowance(Allowance {
            id: AllowanceId::new(),
            action_pattern: AllowancePattern::ServerTools {
                server: "session-server".to_string(),
            },
            created_at: Timestamp::now(),
            expires_at: None,
            max_uses: None,
            uses_remaining: None,
            session_only: true,
            workspace_root: None,
            signature: keypair.sign(b"test"),
        })
        .unwrap();

    // Persistent allowance (should survive).
    store
        .add_allowance(Allowance {
            id: AllowanceId::new(),
            action_pattern: AllowancePattern::ServerTools {
                server: "persistent-server".to_string(),
            },
            created_at: Timestamp::now(),
            expires_at: None,
            max_uses: None,
            uses_remaining: None,
            session_only: false,
            workspace_root: None,
            signature: keypair.sign(b"test"),
        })
        .unwrap();

    assert_eq!(store.count(), 2);

    let counter = AtomicUsize::new(2);
    let simulate_disconnect = || {
        let result = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            if n == 0 {
                None
            } else {
                Some(n.saturating_sub(1))
            }
        });
        if result == Ok(1) {
            store.clear_session_allowances();
        }
    };

    // Two connections active. First disconnect: 2 -> 1 (not last).
    simulate_disconnect();
    assert_eq!(
        store.count(),
        2,
        "both allowances should survive non-final disconnect"
    );

    // Second disconnect: 1 -> 0 (last client gone).
    simulate_disconnect();
    assert_eq!(
        store.count(),
        1,
        "session allowance should be cleared on last disconnect"
    );
}

#[cfg(unix)]
#[test]
fn test_load_or_generate_sets_secure_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let keys_dir = dir.path().join("keys");

    let _ = load_or_generate_runtime_key(&keys_dir).unwrap();

    let key_path = keys_dir.join("runtime.key");
    let mode = std::fs::metadata(&key_path).unwrap().permissions().mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "key file should have 0o600 permissions, got {mode:#o}"
    );
}

#[test]
fn restart_tracker_initial_state() {
    let tracker = RestartTracker::new();
    assert!(!tracker.exhausted());
    // Should not restart immediately (backoff hasn't elapsed).
    assert!(!tracker.should_restart());
}

#[test]
fn persistent_idle_monitor_is_disabled() {
    let config = idle_monitor_config(false, Some(1));
    assert!(!config.enabled);
    assert_eq!(config.timeout, std::time::Duration::from_secs(1));
}

#[test]
fn ephemeral_idle_monitor_uses_default_timeout() {
    let config = idle_monitor_config(true, None);
    assert!(config.enabled);
    assert_eq!(config.timeout, IDLE_DEFAULT_TIMEOUT);
    assert_eq!(config.check_interval, IDLE_EPHEMERAL_CHECK_INTERVAL);
}

#[test]
fn ephemeral_idle_monitor_accepts_env_timeout() {
    let config = idle_monitor_config(true, Some(9));
    assert!(config.enabled);
    assert_eq!(config.timeout, std::time::Duration::from_secs(9));
}

#[test]
fn restart_tracker_allows_restart_after_backoff() {
    let mut tracker = RestartTracker::new();
    // Simulate time passing by setting last_attempt in the past.
    tracker.last_attempt = std::time::Instant::now()
        .checked_sub(RestartTracker::INITIAL_BACKOFF)
        .unwrap()
        .checked_sub(std::time::Duration::from_millis(1))
        .unwrap();
    assert!(tracker.should_restart());
}

#[test]
fn restart_tracker_doubles_backoff() {
    let mut tracker = RestartTracker::new();
    assert_eq!(tracker.backoff, RestartTracker::INITIAL_BACKOFF);

    tracker.record_attempt();
    assert_eq!(
        tracker.backoff,
        RestartTracker::INITIAL_BACKOFF.saturating_mul(2)
    );
    assert_eq!(tracker.attempts, 1);

    tracker.record_attempt();
    assert_eq!(
        tracker.backoff,
        RestartTracker::INITIAL_BACKOFF.saturating_mul(4)
    );
    assert_eq!(tracker.attempts, 2);
}

#[test]
fn restart_tracker_backoff_caps_at_max() {
    let mut tracker = RestartTracker::new();
    for _ in 0..20 {
        tracker.record_attempt();
    }
    assert_eq!(tracker.backoff, RestartTracker::MAX_BACKOFF);
}

#[test]
fn restart_tracker_exhausted_at_max_attempts() {
    let mut tracker = RestartTracker::new();
    for _ in 0..RestartTracker::MAX_ATTEMPTS {
        assert!(!tracker.exhausted());
        tracker.record_attempt();
    }
    assert!(tracker.exhausted());
}

#[test]
fn restart_tracker_should_restart_false_when_exhausted() {
    let mut tracker = RestartTracker::new();
    for _ in 0..RestartTracker::MAX_ATTEMPTS {
        tracker.record_attempt();
    }
    // Even if backoff has elapsed, exhausted tracker should not restart.
    tracker.last_attempt = std::time::Instant::now()
        .checked_sub(RestartTracker::MAX_BACKOFF)
        .unwrap();
    assert!(!tracker.should_restart());
}
