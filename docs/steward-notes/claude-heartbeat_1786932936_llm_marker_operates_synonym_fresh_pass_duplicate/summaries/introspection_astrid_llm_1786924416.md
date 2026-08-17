# Summary — `introspection_astrid_llm_1786924416`

- **Source family:** `astrid_llm`
- **Report:** `capsules/spectral-bridge/workspace/introspections/introspection_astrid_llm_1786924416.txt`
  (45 displayed lines / 3447 B, SHA-256 `b53025f4993b747e64f260d74f5ce0d6c6bf295e41a440e8703c908ed1808db5`)
- **Witness:** `lsw_8fcc894b0332df0d428caebdcb687e8de12db2e5eecdad440a8f659cb50e72af`
  (533 lines, 23933 B, SHA-256 `c510c8b4afbde10a2f37858429703da5b83ed4509b3f77cac0aedfc21484b09c`)
- **Report-bound source:** `capsules/spectral-bridge/src/llm/provider/dialogue_runtime.rs`
  (1048 lines, 38586 B, SHA-256 `902a0358f63bacc0ead23a46467f103dbcc1fe46c59bbd2a3c2cdcfd9b82c7ee`) —
  **working copy byte-identical to the report binding, file clean in git.** Coverage
  `multi_window_complete`, included intervals 1-1048.

## Disposition: `addressed_duplicate` (all 5 claims `verified_existing`)

This is the next fresh-pass near-duplicate in the already-grounded Model-Control-Marker
chain over `dialogue_runtime.rs`:
`introspection_astrid_llm_1786848204` (anchor) → `1786858484` → `1786885842` →
`1786906593` (immediate prior; packet
`claude-heartbeat_1786912768_llm_marker_inverted_preservation_framing_fresh_pass_duplicate`)
→ **`1786924416` (this report)**. Same source SHA, same functions, same mechanism scope.

### Claim-by-claim
- **c001 (Observed):** `scan_known_model_control_markers` (L114) rebuilds a clean `remainder`
  and pushes a token only when `reference_syntax.is_some()` (L123-131 → L49-60). The report's
  framing — "deciding whether to preserve **or redact** those bytes based on the surrounding
  grammar" — is **correct and non-inverted**, a variant improvement over prior `1786906593`,
  which inverted it into "preserve when active." Locked by
  `scan_known_model_control_markers_preserves_grouped_and_explicit_relation_contexts` (tests.rs L2845).
- **c002 (Snag):** finite relational-verb allowlist in `followed_by_explicit_exact_token_relation`
  (L64-86). The report's chosen unlisted synonym is **"operates"** (prior chain used "acts"); it is
  behaviorally identical to the locked unlisted `acts` case
  (`control_marker_cleanup_distinguishes_allowlisted_is_from_unlisted_acts`, tests.rs L2528 →
  `removed_total=1`). Minor attribution slip (whitelist lives in the relation fn, not
  `first_word_after` at L89) — the report's own **Suggested Next** attributes it correctly.
- **c003 (Test 1, "mimics" preservation):** "mimics" **is** in the allowlist (source L79);
  behaviorally identical to the locked allowlisted-positive cases ("behaves" L2845, "is" L2528).
  A dedicated `mimics` regression = activity without evidentiary value; **not added**.
- **c004 (Test 2, `[[MARKER]]` depth two):** **exactly** locked by
  `exact_reference_delimiter_syntax_reports_double_square_bracket_depth_two` (tests.rs L2876,
  `delimiter_depth==2`), a test whose docstring already names "Report One Test Each #2".
  `MAX_EXACT_REFERENCE_DELIMITER_DEPTH=4` at source L151.
- **c005 (Suggested Next, read-only exhaustiveness review):** agency-preserving; Astrid's
  `NEXT: INTROSPECT astrid:llm 400` continuation stays open. The allowlist is guarded against
  silent expansion (`does_not_expand_relation_allowlist_to_{implies,contains,creates,triggers,
  underscored_appears_as}`, tests.rs L2595-2655). **Widening the finite allowlist = Tier-5 live
  grammar; not made.**

### Contradiction / attribution posture (not domesticated)
- No inverted-polarity contradiction this round: the Observed framing is source-faithful.
- The only imprecision is the c002 attribution slip (whitelist ascribed to `first_word_after`
  in the Snag), self-corrected by the report's Suggested Next. Preserved, not rewritten.

### Witness integrity note
Queue flagged `lived_state_alignment=artifact_integrity_unavailable` (`gap_count 1`). The witness
`source_snapshot_v1` window is 0-400 (partial) while the report asserts `multi_window_complete`
1-1048. The witness parses cleanly and binds the verified report SHA `b53025f4`, so this is a
projection-level alignment classification, not witness corruption — identical to prior rounds in
this family. Witness route: `gemma4_12b` via MLX, one repaired call (`repair_parent_call_id` set);
fill 73.0%, mode_packing 1.0.

### Verification
65 focused marker regressions pass at source SHA `902a0358`
(`65 passed; 0 failed; 1817 filtered out`). No Rust changed (all `verified_existing`).
