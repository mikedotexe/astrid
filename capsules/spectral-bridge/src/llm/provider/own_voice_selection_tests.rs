#[cfg(test)]
mod own_voice_selection_tests {
    use super::astrid_fallback_identity_anchor_from_dir;
    use std::path::Path;

    fn entry(dir: &Path, name: &str, mode: &str, body: &str, modified: u64) {
        let path = dir.join(name);
        std::fs::write(&path, format!("=== ASTRID JOURNAL ===\nMode: {mode}\nTimestamp: 97\n\n{body}\n")).unwrap();
        std::fs::File::open(path).unwrap().set_modified(
            std::time::UNIX_EPOCH.checked_add(std::time::Duration::from_secs(modified)).unwrap(),
        ).unwrap();
    }

    #[test]
    fn fallback_selects_owned_voice_after_three_recent_legacy_mirrors() {
        let dir = tempfile::tempdir().unwrap();
        entry(dir.path(), "astrid_owned.txt", "dialogue_live", "OWNED_ANCHOR", 1);
        for index in 0_u64..3 {
            entry(dir.path(), &format!("astrid_mirror_{index}.txt"), "mirror", "PEER_ONLY", index.saturating_add(2));
        }
        let anchor = astrid_fallback_identity_anchor_from_dir(dir.path()).unwrap();
        assert_eq!(anchor, "OWNED_ANCHOR");
    }

    #[test]
    fn fallback_keeps_three_owned_snippets_in_mtime_order() {
        let dir = tempfile::tempdir().unwrap();
        for (index, body) in [(1, "OWN_1"), (2, "OWN_2"), (3, "OWN_3"), (4, "OWN_4")] {
            entry(dir.path(), &format!("astrid_owned_{index}.txt"), "dialogue_live", body, index);
        }
        let anchor = astrid_fallback_identity_anchor_from_dir(dir.path()).unwrap();
        assert!(!anchor.contains("OWN_1"));
        assert!(anchor.find("OWN_4").unwrap() < anchor.find("OWN_3").unwrap());
        assert!(anchor.find("OWN_3").unwrap() < anchor.find("OWN_2").unwrap());
    }

    #[test]
    fn agency_candidates_skip_recent_legacy_mirrors_before_selection() {
        let dir = tempfile::tempdir().unwrap();
        let own = "An owned journal passage with sufficient length for continuity retrieval.";
        entry(dir.path(), "astrid_owned.txt", "dialogue_live", own, 1);
        entry(dir.path(), "astrid_mirror.txt", "mirror", "Peer-only journal passage with sufficient length for continuity retrieval.", 2);
        assert_eq!(crate::agency::recent_own_journal_excerpt(dir.path(), None).as_deref(), Some(own));
        assert_eq!(crate::agency::find_evolve_trigger_entry(dir.path()), Some(dir.path().join("astrid_owned.txt")));
    }
}
