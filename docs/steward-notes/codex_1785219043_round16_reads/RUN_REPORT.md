# Source-First Steward Run Report

## Control Plane

- Steward run: `run_1785219043749388000_d604cdee7e`
- Pre-run projection: `projection_1785219044736240000_90f96f6332`
- Pause generation: `43`
- Finish and post-run projection: pending controller finish

The controller safely reaped the prior expired session lease and recorded that
run as abandoned. That prior shell had exited after preprojection and before
any repository work; no token was recovered or bypassed.

## Fully Processed

1. `introspection_minime_regulator_1785218319.txt`
   - SHA-256:
     `71222cdbdab336fc06af2b38b66774890d449427b0f3ac22637dab45234e6724`
   - Witness:
     `lsw_11151ddd0a181be3b3d4425e3cf4897fe721a684320600de232ee2fc80e81bfb`
2. `introspection_minime_sensory_bus_1785217846.txt`
   - SHA-256:
     `ef4a7eadf20fa23de8ef491973116d12c94601b64fcad85b6cfc8ad256177278`
   - Witness:
     `lsw_209810c0508a0ce9ac7cd602c258f8bb76e13b38c8cf6062f293ee61ee419e54`
3. `introspection_minime_regulator_1785217487.txt`
   - SHA-256:
     `643c4fc6c98fe059391e121e3a7cf49f30ba99b7f28ea34957c000d8265827da`
   - Witness:
     `lsw_0db1c6da9bab5d41e2905f94bca32e5cd5f63e8ba457c68595ec1e1a4043a254`

All three reports were read fully from disk in strict queue order. The
remaining 37 selected filenames are preserved exactly in
`unprocessed_selected.json`.

## Claim Dispositions

Fourteen claims remain independent:

- two qualitative experiential claims preserve heavy-syrup, suffocation, and
  regulatory-interference reports without assigning cause;
- five current-source verifications distinguish concurrent PD
  rate/content-gate and PI fill/band-stop roles, and retain the existing
  read-only pressure, viscosity, and semantic-retention evidence;
- two exact sensory boundary regressions were implemented;
- two natural-context study routes remain non-induced and non-causal;
- three exact Tier 5 waits preserve every live porosity, pressure, rate, gate,
  PI, fill, filter, release-threshold, or stale-window change.

The sensory report's `0.2501` interpretation is corrected from current source:
`max(0.25 + 0.01)` clamps a requested `0.10` release to `0.26`. At fill
`0.26`, the handover has met the shaped curve. Fill `0.40` reaches the
sigmoid window midpoint within the expected `f32`-to-`f64` tolerance.

## Changes And Verification

Minime's `sensory_bus.rs` adds only the two focused regressions above. No
runtime expression, constant, threshold, or control path changed.

Passed:

- 20 focused semantic-stale tests;
- all 317 Minime library tests;
- rustfmt and touched diff hygiene;
- 41 introspection-addressing tests and a consistent counter audit;
- 13 Evidence Event Store tests;
- 30 steward-control and source-first projection tests;
- 20 Division follow-up, Chronicle, and Passage Observatory tests;
- two experiential-epistemics self-tests and 9,347-record verification;
- Division Chronicle verification;
- Evidence Event Store V2 full verification.

Strict Minime library Clippy remains blocked by 68 pre-existing warnings across
unrelated modules. None originates in the added tests. This run does not widen
into that repository cleanup.

## Operations And Authority

No capture was armed, study event appended, Sandbox trial created, Corridor
program or lease added, attention portfolio changed, correspondence sent,
right-to-ignore request opened, Passage Action, Division Action, rehearsal,
handoff, daughter process, restart, or deployment occurred.

The touched Minime source is test-only, so no service restart is required.
No live-consumed bridge, protocol, prompt/report, summary, capture, Minime
runtime, pressure, fill, PI, codec, model, sensory, scheduler, or reservoir
surface changed.

## Queue And Evidence

Division follow-up cycle 3 advanced to `4/6` with event
`division_followup_event_3f565ad7398042c5a4ad3affb67110d7`. Two productive
rounds remain before the next bounded return; review is not due. Chronicle
`division_chronicle_2b24b6a6ac97e3bf36ef86d9` verifies with zero timeline
events.

The counter audit is consistent at canonical indexed `3493`, full-read
`3471`, fully addressed `2893`, unread `22`, read-needs-claims `0`, blocked
explicit waits `397`, triaged pending `177`, watch `4`, and canonical
remaining `600`. All-artifact pending is `2157`.

The next canonical queue begins:

1. `introspection_astrid_llm_1785217044.txt`
2. `introspection_astrid_types_1785215474.txt`
3. `introspection_astrid_ws_1785215201.txt`
4. `introspection_astrid_autonomous_1785214393.txt`
5. `introspection_proposal_12d_glimpse_1785213218.txt`

Evidence Event Store V2 verified at sequence `601373`, head
`90ed9404979eeb0823f18fc72d8f5899fe15992e762f961f29c2303064910e6a`,
with zero corrupt lines. Stream counts were addressing `52779`,
agency commons `2819`, attention portfolio `3`, claim families `233468`,
Corridor V1 `5`, Corridor V2 `112`, felt contracts `177689`,
felt-mechanism concordance `80`, lived-state witness `6923`, model QoS
`51830`, reciprocal uptake `44022`, representation contracts `14832`,
Sandbox `2950`, signal spine `10180`, steward control `3545`, and steward work
selection `136`. V1 remains immutable.

The historical archival checkpoint is due, but it must run only after a
successful controller finish. Candidate ownership remains mixed with the
concurrent smaller-device session, and no staging, commit, or push occurred
during this run.
