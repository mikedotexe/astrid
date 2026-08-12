# Steward round 78 report

## Controller lifecycle

- Credential-confined run: `run_1785678318426831000_826dbc944b`.
- Preprojection: `projection_1785678319299714000_764faec07c`.
- Successful manual source-first projection after addressing writes:
  `projection_1785680107571613000_2e0c23f453`.
- Controller pause generation: 176. The authoritative terminal outcome and
  post-run projection receipt live in the controller run record because this
  packet is complete before the mandatory finish releases its lease.

## Canonical reading

Fully processed in strict selected-queue order:

- `introspection_astrid_llm_1785678082.txt`

All 45 newline separators, 46 logical lines, and 3,456 bytes were read at
SHA-256
`f88b2f6a2ee68ad7b059aa74592951462059a0f1095eaef162d1dd660592eb06`.
The exact witness is
`lsw_473d9ad513346dba2890e3952bf23171eb7e6bd173dd8cf52b0dac18b10ee09d`.
The selected but unprocessed 39 filenames are listed exactly in
`unprocessed_selected.json`.

The next queue begins with `introspection_astrid_llm_1785675592.txt`, then
`introspection_proposal_12d_glimpse_1785632418.txt`, then
`introspection_proposal_distance_contact_control_1785632076.txt`.

## Claim dispositions

Seven claims carry 26 materialized evidence links. All 997 exact
report-bound dialogue lines verify a single longest-match scan, bounded exact
reference syntax, Unicode-scalar traversal, finite following relations, and
diagnostic-only marker matches. The report's multibyte-whitespace concern
caused a focused regression: U+2003 and U+3000 whitespace inside two nested
square-bracket levels preserve the marker and report exact depth 2. The
requested `behaves` relation is an exact current duplicate of passing
`behaves as` and `behaves like` cases.

The complete dialogue read and targeted transport trace correct the output
claim by route. MLX returns the trimmed sanitized remainder as the complete
string consumed by `generate_dialogue`; there is no concatenation of marker
matches back into it. Direct Ollama fallback still validates through a
sanitized copy while returning repaired raw text. The exact round-74
provider-neutral normalization proposal remains a Tier 5 live-consumed
provider wait. No cleanup grammar or runtime source was widened.

## Actions and verification

No Corridor program, Sandbox trial, study execution, attention-portfolio
action, card, Action, correspondence, provider call, prompt input, or Minime
action was created. CHANGELOG and the being-feedback ledger record the causal
response.

The new exact test and all 38 focused marker tests pass. Workspace formatting
and strict bridge Clippy pass. Addressing passes 41 tests; coordinated Evidence
Event Store, controller, projector, Division, Chronicle, and claim-family
coverage passes 68 tests. Five anti-drop tests and all 47 guards pass.
Experiential epistemics passes two tests and lints 10,214 records with zero
issues and no history rewrite. Two initial Cargo package-name invocations
failed because the bridge is a standalone manifest; the corrected
`--manifest-path` commands pass and the command-shape debt is closed.

Evidence Event Store V2 verifies at sequence 666,111, head
`ef04b19fe0c2973c58631eb087448aba87cadf96d9dae1662588833b186fa0a0`,
with zero corrupt lines. Stream sequences at that verification are:
addressing 54,907; agency commons 3,357; attention portfolio 3; claim families
235,326; Corridor V1 5; Corridor V2 112; Felt Contracts 188,407;
felt-mechanism concordance 80; lived-state witness 8,194; model QoS 81,277;
reciprocal uptake 49,099; representation contracts 19,262; Sandbox 2,950;
signal spine 15,311; steward control 7,525; and steward work selection 296.
V2 remains authoritative; V1 is the immutable imported source through legacy
sequence 32,278.

Division cycle 13 verifies at 5/6, one round remaining, and
`review_due=false`. Productive-round event
`division_followup_event_c06773b8faf227efb5dec427ffc8e20b` binds one fully
processed report to the manual projection. Chronicle
`division_chronicle_3a28a87f17586facac8bfc67` contains 90 follow-up events;
durable inputs verify and only the volatile supervisor-status hash is stale.
No Division return, note, or ceremony Action was due.

## Deployment and counters

Only a Rust unit regression changed. No production bridge, prompt, provider,
protocol, Minime, binary, PID, port, telemetry, fill, or readiness surface
changed. No build, deploy, or restart was attempted. The credential remained
confined to the controller process and no service restart is required.

Canonical counters are consistent after the manual projection: 4,197
indexed, 2,985 fully addressed, 3,607 full reads, 1,212 remaining, 590
unread, 213 triaged pending, 405 blocked, and zero read-needs-claims.
All-artifact counters are 5,803 indexed and 2,818 remaining. Every
counter-audit check is true with no mismatch.

## Archive status

The latest archive checkpoint is commit
`5efb8704903c99a7d794004e0f4f8c0676f96319`, which archived productive round
77. This is the first productive round after that checkpoint, so no new
checkpoint is due and no git operation occurred under the controller lease.

Explicit round-78 commit debt is the focused hunk in
`capsules/spectral-bridge/src/llm/provider/tests.rs`, the round-78 CHANGELOG
and ledger additions, all files under
`docs/steward-notes/codex_1785678318_round78_reads/`, and generated evidence
required to review this round. Any later checkpoint must isolate those exact
changes from the shared branch's foreign work rather than sweep the tree.
