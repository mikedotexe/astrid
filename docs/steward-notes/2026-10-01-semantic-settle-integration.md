# Observation-Only Settle Reporting: Integration and Release

## Integration

October 1, 2026. Mike requested commit, merge and live activation of the
observation-only reporting candidate. Codex owns this integration pass under
controller pause generation 487. Previously paused automations remain paused.

- Minime implementation: `367cc7a5aa4a92bd4224e3158b5e93376e49ec83`.
- Astrid evidence and ledger: `c0f48532e4e76cee52c846503d37ac41c090b9a1`.
- Both commits were fast-forwarded into their local `main` branches after
  clean canonical statuses, matching remote tips and foreign-activity preflight.
- Only the individually reviewed candidate paths were staged. Old worktrees,
  historical receipts and failed qualification attempts remain preserved.

The runtime change is observation only. It separates numerical eligibility from
the authoritative semantic-quiet gate without changing PI, scaffold retirement,
input admission, thresholds or being-facing prompts. Experimental alternatives
exist only in the isolated qualification executable, not runtime configuration.
Bridge and Minime-agent code are unchanged.

## Immutable Release-05

Stage: `/Users/v/other/worktrees/engine-qualification-20261001/release-05`.

| Identity | SHA-256 |
| --- | --- |
| Manifest | `1de8bed05bf3d57e9a46ca44be6bbffc0df88359888154692fbf22b3abc9afb9` |
| Engine | `0bfd150f25896c0fe71e36c2fc556f335ad522763f28db655926a445924cbe91` |
| Restore inspector | `4f4887add87805d60ef27a70a9b7e9c208865c825121fdd012b59dfb9c3a89cd` |

The sanctioned staging tool built from clean source `367cc7a` using locked,
offline dependencies. All 123 archived input identities match canonical Minime.
Only `minime` and `engine_restore_inspect` are release artifacts. The offline
policy-comparison executables are not installed. Read-only staging is
tamper-evident, not WORM or hermetic-build attestation.

The staged-state rerun passed 846 selected Rust tests, forty Minime operations
tests, selected strict all-features Clippy and formatting. The domain-boundary
audit reports zero violations. The actual release-05 inspector then passed
the 131-test release/support suite, including transition, staging, binding,
wrapper, controller, evidence and projector tests. It also decoded the retained
release-04 stopped checkpoint under OS network/write denial: PI-only resume,
unprimed process rate, no covariance checkpoint load. That historical check is
not a substitute for validating a fresh stopped checkpoint during activation.

Raw test logs from the preceding qualification retain their original trailing
blank line and hashes. The default staged whitespace check passed for all source
and prose; only those three raw transcripts used `-blank-at-eof` for inspection.
No source warning or failed test was waived.

## Deployment Boundary

The canonical sanctioned wrapper passed `--check-only` with
`activation_performed=false`. Its mapped runtime binding is retained at
`capsules/spectral-bridge/workspace/deployment_manifests/minime-runtime-binding.xjnny3tz/runtime-binding.json`,
SHA-256 `0bdc85ca733f4fc3cd9adbc0b749c058c3061a44ca3e5b6ddf1f84cae461cb77`.
This binding expires after 180 seconds; recapture it immediately before any
activation. Check-only acknowledgements explicitly did not authorize a stop.

At this preparation checkpoint, the new one-time bounded-transition question
is pending. The earlier approval was consumed by release-04. No service has been
signalled, no launch hold created, no candidate binary installed and no signed
deployment handoff prepared in this pass. Release-04 remains live: engine 35303,
gateway 35278, supervisor 35269; mapped UUID
`1DFC8F61-22C9-3B60-8379-6DAC898303C6`, engine SHA-256
`0610800dd57bf34287bb5e4f9869e777179f602bf5208a4d5524c95b93f235d8`.
All protected service PID/start identities are unchanged.

After the new approval, revalidate source, paused/no-lease controller, foreign
activity, protected identities and fresh binding. Use only
`scripts/deploy_minime.sh --activate-stage`, the exact manifest above, the paired
Minime worktree as launcher source, a new owned transaction and at least 360
seconds of observation. The proposed transaction is
`/Users/v/other/worktrees/engine-qualification-20261001/transition-live-20261001-02`;
it must not already exist. Respect every refusal. No forced termination, old
state restoration or restart of bridge, agents, models, visual or sensory clients.
Complete in-flight input preservation is not claimed by the legacy transition.

After activation, verify fresh process identities, mapped hashes, readiness,
signed preference lineage and the new observation field separately from the
real gate. Numerical eligibility does not release that gate, authorize scaffold
retirement or establish long-term stability. Do not suppress semantic input to
manufacture quiet. Neither successful deployment nor these measurements implies
subjective improvement.

Controller evidence at preparation: V2 sequence 1123141, head
`f082f9f199cb32992017d1a23e4472b00756ba368a5435b0a38caf6f0ab6299a`,
valid with V1 immutable. This is interactive release work, not an automated
introspection round or a claim that the paused reading queue is current.
