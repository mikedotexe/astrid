# Run 15 Source-First Packet

- Steward run: `run_1784619792838718000_ca9a5531b5`
- Canonical packet size: 20 full reads in queue order
- Claims: 81 total; 47 verified, 19 sandbox-routed, 13 authority-gated, 2 implemented
- Unprocessed selected reports: none
- Implementation: distinguish semantic-heartbeat rescue admission from actual bounded-channel enqueue and expose bounded timing/outcome evidence
- Live behavior: unchanged
- Authority: all promoted work remains `evidence_only` or `approval_pending`; all live eligibility, auto-approval, approval-grant, and source-edit authority markers remain false
- Alignment: bridge restart required after tests, shared with inherited provider-cleanup restart debt
- Verification: bridge 1,598/1,598 with host permissions; strict Clippy and formatting clean; Minime regulator 162/162; targeted Recess 13/13; 271/271 required evidence and flywheel self-tests
- Deployment: release build succeeded; host install/restart was blocked because the shared dirty source set exceeds this run's narrow review authority. Exact debt is in `restart_debt.json`.

The exact report order and content identities are in `full_read_manifest.json`. Per-report bounded summaries and claim files retain every extracted claim; `evidence_links.json` binds each claim to source, test, sandbox route, implementation, or explicit authority rationale.
