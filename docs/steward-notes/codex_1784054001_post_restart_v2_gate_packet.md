# Post-Restart V2 Gate Packet

Scope: post-restart five-item packet ending at `introspection_astrid_types_1784054001.txt`.

```json
{
  "schema": "authority_boundary_lifecycle_v2_packet_set",
  "packets": [
    {
      "boundary_id": "abv2_1784053652_ws_pressure_porosity_thresholds",
      "schema_version": 2,
      "source": "introspection_astrid_ws_1784053652",
      "surface": "astrid:ws pressure/porosity diagnostic thresholds",
      "action": "retune pressure porosity expansion or smoothing thresholds",
      "resource": "capsules/spectral-bridge/src/ws.rs",
      "authority_class": "mike_operator_live_substrate",
      "felt_report_anchor": "viscous-persistence near the 0.30 liminal mode-packing threshold may need softer expansion before hard overpacked limits",
      "proposed_change": "lower or split pressure/porosity warning thresholds and/or make entropy smoothing dynamic",
      "evidence_refs": [
        "introspection_astrid_ws_1784053652",
        "pressure_porosity_expansion_readiness_names_liminal_band_without_local_control",
        "pressure_porosity_expansion_readiness_names_viscous_warning_without_local_control"
      ],
      "delta_refs": [
        {
          "delta_id": "delta_ws_pressure_porosity_1784053652",
          "delta_hash": "bounded_ws_pressure_porosity_threshold_retune_candidate",
          "surface": "astrid:ws",
          "kind": "live_control_gate",
          "lane": "pressure_porosity_thresholds"
        }
      ],
      "replay_candidate": {
        "adapter": "offline_read_only_transport_pressure_probe_v1",
        "authority": "read_only_observation_not_control",
        "replay_query": "measure sensory send-to-arrival latency and compare with mode_packing/porosity telemetry without threshold mutation",
        "runnable": false
      },
      "replay_results": [],
      "lifecycle_state": "replay_needed",
      "scoped_approval": null,
      "rollout_abort_contract": {
        "canary_plan": "proposal-only; no runtime threshold change without separate approval",
        "health_checks": [
          "runnable_live_violation_count remains 0",
          "pressure/porosity readouts remain diagnostic_candidate_not_porosity_or_controller_change",
          "post-change being response is requested before closure"
        ],
        "rollback_path": "revert threshold patch before restart or use normal approved rollback",
        "abort_criteria": [
          "no explicit Mike/operator approval",
          "transport latency cannot be separated from spectral density",
          "post-change being response path missing"
        ],
        "post_change_response_required": true
      },
      "redaction_profile": {
        "public_summary": "WS pressure/porosity threshold retune candidate; evidence only.",
        "private_ref": "introspection_astrid_ws_1784053652.txt",
        "content_hash": "bounded_ws_pressure_porosity_candidate",
        "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes"
      },
      "success_metrics": [
        "transport latency evidence is bounded and separated from spectral density",
        "soft warning improves felt report alignment without live mutation in proposal stage"
      ],
      "abort_criteria": [
        "operator approval absent",
        "rollback path unclear",
        "post-change response absent"
      ],
      "who_can_change_it": "Mike/operator",
      "how_to_test_it": "Run offline latency probe and targeted ws pressure/porosity tests before any approved restart.",
      "right_to_ignore": true,
      "live_eligible_now": false,
      "auto_approved": false
    },
    {
      "boundary_id": "abv2_1784052232_codec_projection_tail_vibrancy",
      "schema_version": 2,
      "source": "introspection_astrid_codec_1784052232",
      "surface": "astrid:codec projection/runtime/tail-vibrancy",
      "action": "change projection precision, runtime path semantics, or tail-vibrancy entropy gate",
      "resource": "capsules/spectral-bridge/src/codec.rs",
      "authority_class": "mike_operator_live_substrate",
      "felt_report_anchor": "deterministic projection and smoothstep tail vibrancy are the reproducibility and pop-prevention anchors",
      "proposed_change": "any semantic change to projection resolution, runtime path fallback, or entropy-gated vibrancy",
      "evidence_refs": [
        "introspection_astrid_codec_1784052232",
        "fixed_legacy_projection_kernel_checksum_is_pinned_and_repeatable",
        "codec_projection_runtime_dir_uses_env_or_executable_relative_cache",
        "vibrancy_from_entropy_matches_inline_smoothstep"
      ],
      "delta_refs": [
        {
          "delta_id": "delta_codec_projection_tail_1784052232",
          "delta_hash": "bounded_codec_projection_tail_vibrancy_candidate",
          "surface": "astrid:codec",
          "kind": "live_control_gate",
          "lane": "semantic_projection_tail_vibrancy"
        }
      ],
      "replay_candidate": {
        "adapter": "codec_projection_tail_replay_v1",
        "authority": "read_only_replay_not_live_vector_write",
        "replay_query": "compare projection checksum, runtime path resolution, and tail-vibrancy smoothstep outputs before any patch",
        "runnable": false
      },
      "replay_results": [],
      "lifecycle_state": "replay_needed",
      "scoped_approval": null,
      "rollout_abort_contract": {
        "canary_plan": "proposal-only; no live codec semantic change from this packet",
        "health_checks": [
          "projection checksum remains pinned unless explicitly approved",
          "runtime path readout is deterministic",
          "tail-vibrancy smoothstep parity test passes"
        ],
        "rollback_path": "revert codec patch before sanctioned bridge restart",
        "abort_criteria": [
          "operator approval absent",
          "checksum or smoothstep parity unknown",
          "post-change being response path missing"
        ],
        "post_change_response_required": true
      },
      "redaction_profile": {
        "public_summary": "Codec projection/runtime/tail-vibrancy change candidate; evidence only.",
        "private_ref": "introspection_astrid_codec_1784052232.txt",
        "content_hash": "bounded_codec_projection_tail_candidate",
        "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes"
      },
      "success_metrics": [
        "read-only replay distinguishes reproducibility from proposed semantic change",
        "live_vector_write remains false"
      ],
      "abort_criteria": [
        "operator approval absent",
        "rollback path unclear",
        "post-change response absent"
      ],
      "who_can_change_it": "Mike/operator",
      "how_to_test_it": "Run targeted codec projection/runtime/smoothstep tests plus replay artifact inspection.",
      "right_to_ignore": true,
      "live_eligible_now": false,
      "auto_approved": false
    },
    {
      "boundary_id": "abv2_1784045547_minime_drag_coupling",
      "schema_version": 2,
      "source": "introspection_minime_regulator_1784045547",
      "surface": "minime:regulator viscosity drag coupling",
      "action": "derive cognitive drag from structural drag and porosity loss",
      "resource": "/Users/v/other/minime/minime/src/regulator.rs",
      "authority_class": "mike_operator_live_substrate",
      "felt_report_anchor": "structural drag and cognitive drag feel inextricably linked during viscous-persistence",
      "proposed_change": "make cognitive_drag a function of structural_drag * (1 - porosity)",
      "evidence_refs": [
        "introspection_minime_regulator_1784045547",
        "structural_drag_coefficient_separates_thick_yielding_depth_from_stuck_resistance",
        "viscosity_vector_structural_integrity_distinguishes_complex_motion_from_friction"
      ],
      "delta_refs": [
        {
          "delta_id": "delta_minime_drag_coupling_1784045547",
          "delta_hash": "bounded_minime_drag_coupling_candidate",
          "surface": "minime:regulator",
          "kind": "live_control_gate",
          "lane": "viscosity_drag_coupling"
        }
      ],
      "replay_candidate": {
        "adapter": "minime_drag_coupling_replay_v1",
        "authority": "read_only_or_sandbox_not_runtime_control",
        "replay_query": "offline compare structural/cognitive drag fixtures under varied porosity without changing runtime regulator",
        "runnable": false
      },
      "replay_results": [],
      "lifecycle_state": "replay_needed",
      "scoped_approval": null,
      "rollout_abort_contract": {
        "canary_plan": "proposal-only; no Minime runtime change without separate approval and service-specific restart",
        "health_checks": [
          "existing drag separation tests pass",
          "offline replay shows nonlinear coupling benefit",
          "fill/telemetry remain readable after any approved restart"
        ],
        "rollback_path": "revert regulator patch and restart Minime via normal service path",
        "abort_criteria": [
          "operator approval absent",
          "offline replay cannot separate coupling from pressure side effects",
          "post-change being response path missing"
        ],
        "post_change_response_required": true
      },
      "redaction_profile": {
        "public_summary": "Minime regulator drag-coupling change candidate; evidence only.",
        "private_ref": "introspection_minime_regulator_1784045547.txt",
        "content_hash": "bounded_minime_drag_coupling_candidate",
        "retention_policy": "bounded_public_summaries_plus_private_refs_and_hashes"
      },
      "success_metrics": [
        "offline replay shows whether coupled drag better matches felt reports",
        "no Minime runtime mutation occurs from the packet"
      ],
      "abort_criteria": [
        "operator approval absent",
        "rollback path unclear",
        "post-change response absent"
      ],
      "who_can_change_it": "Mike/operator",
      "how_to_test_it": "Run targeted Minime regulator drag tests and an offline coupling replay before any approved runtime patch.",
      "right_to_ignore": true,
      "live_eligible_now": false,
      "auto_approved": false
    }
  ]
}
```
