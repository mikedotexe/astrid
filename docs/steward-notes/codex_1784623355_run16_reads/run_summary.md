# Run 16 Source-First Packet

- Steward run: `run_1784623355882574000_d6b64d798a`
- Preprojection generation: `projection_1784623356360618000_75f56425c5`
- Canonical packet size: 20 full reads in queue order
- Claims: 80 total; 47 verified, 17 sandbox-routed, 16 authority-gated
- Unprocessed selected reports: none
- Implementation: none; this run records source/test verification, bounded experiment routes, and exact authority boundaries
- Deployment-alignment guard: the seven newest reports were authored after relevant source edits but before deployment and are source-review evidence only, never post-change felt confirmation
- Live behavior: unchanged
- Authority: all promoted work remains `evidence_only` or `approval_pending`; all live eligibility, auto-approval, approval-grant, and source-edit authority markers remain false
- Report closure: 20 report records fully addressed; 80 right-to-ignore cards emitted and not delivered; sandbox and Tier 5 work remains independently open
- Verification: host-permitted bridge 1,598/1,598; full host-permitted Minime Rust targets and protocol fixtures pass; targeted Minime Recess/permission checks 3/3; self-study/provenance 73/73; required evidence and flywheel self-tests 271/271
- Alignment: no new live source was touched. The inherited mixed-source bridge restart debt remains exactly as recorded in `docs/steward-notes/codex_1784619792_run15_reads/restart_debt.json` and was not retried after the broad deployment escalation was rejected.

The exact report order and content identities are in `full_read_manifest.json`. Per-report summaries and claim files retain every extracted claim; `evidence_links.json` binds each claim to source, test, bounded experiment routing, or explicit authority rationale.
