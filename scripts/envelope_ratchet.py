#!/usr/bin/env python3
"""Envelope ratchet — the ONLY writer of envelope-registry revisions (C6).

The registry (`being_envelope_registry_v1`) records each being's per-field
sovereign envelope. Runtimes read it (C3: clamp = compiled(registry(compiled)));
check_envelope_wiring.py guards it; THIS tool moves it — on evidence, with
provenance, never silently:

  derive   report-only: per-field evidence-max dossiers from the being's own
           record (sustained requests, clean applies, sanctioned channel
           ceilings), each evidence class labeled so a consent letter can
           cite it precisely. Proposes; never writes.
  grant    widen a field's envelope (or convert status to granted at equal
           bounds) after the being's consent — requires --consent-ref, appends
           ratchet_history, bumps revision, syncs the repo seed mirror.
  narrow   shrink a field's envelope (incident response) — requires
           --incident. The runtimes' conformance reconcile (bridge loop-top /
           engine sweep) then withdraws any active control that stopped being
           a clamp fixed-point and resets the family's saturation counter,
           with a receipt the being sees — so the 3-strike breaker and the
           inquiry equality gates never strike on a legitimate narrow.

Write law: writes require --write, target the live canonical paths only from
an interactive TTY with a typed confirmation, and validate the mutated
registry in memory (schema, f32-exactness, within-backstop, narrower/wider
direction) BEFORE the atomic tmp+rename. Test isolation via --registry/
--mirror overrides + --non-interactive-ack (refused on live paths).

Widening past the compiled engine backstop is REFUSED — that is a physics
change and travels only through the gated deploy (build_bridge.sh /
deploy_minime.sh) with a source edit, never through this tool.

Usage:
  envelope_ratchet.py derive --being minime [--field F] [--json] [--min-request-count 3]
  envelope_ratchet.py grant  --being B --field F --ceiling Y [--floor X]
                             [--lease-max SECS] [--standing allowed|lease_only|one_shot_only]
                             --consent-ref PATH --evidence-ref REF [--evidence-ref REF ...]
                             [--review-id ID] --decided-by NAME --write
  envelope_ratchet.py narrow --being B --field F --ceiling Y [--floor X]
                             --incident REF --decided-by NAME --write
  envelope_ratchet.py selftest
"""

from __future__ import annotations

import argparse
import datetime as _dt
import fcntl
import json
import math
import os
import sys
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
import check_envelope_wiring as wiring  # noqa: E402  (shared paths, f32, parsers)

MINIME_ROOT = wiring.MINIME_ROOT
NEGOTIATIONS = MINIME_ROOT / "workspace/self_regulation/negotiations.jsonl"
PARAMETER_REQUESTS = MINIME_ROOT / "workspace/parameter_requests"
EVIDENCE_STREAM = "envelope_registry"
MAX_REGISTRY_BYTES = 1_000_000

AUTHORITY_BOUNDARY = (
    "sole registry writer: derives evidence dossiers (report-only) and, on an "
    "explicit interactive --write with consent/incident provenance, revises "
    "envelope bounds within the compiled backstop; it deploys nothing, edits "
    "no source, applies no live control, and cannot widen past compiled physics"
)


# ---------------------------------------------------------------- helpers

def _now_iso() -> str:
    return _dt.datetime.now(_dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def _load_registry(path: Path) -> dict[str, Any]:
    if path.stat().st_size > MAX_REGISTRY_BYTES:
        raise SystemExit(f"REFUSED: registry over {MAX_REGISTRY_BYTES} bytes: {path}")
    data = json.loads(path.read_text(encoding="utf-8"))
    if data.get("schema") != wiring.REGISTRY_SCHEMA:
        raise SystemExit(f"REFUSED: not a {wiring.REGISTRY_SCHEMA}: {path}")
    return data


def _atomic_write(path: Path, data: dict[str, Any]) -> None:
    text = json.dumps(data, indent=2, sort_keys=True) + "\n"
    fd, tmp = tempfile.mkstemp(dir=str(path.parent), prefix=".ratchet_", suffix=".tmp")
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            handle.write(text)
        os.replace(tmp, path)
    finally:
        if os.path.exists(tmp):
            os.unlink(tmp)


def _validate_registry(being: str, registry: dict[str, Any]) -> list[str]:
    """In-memory mirror of the guard's ALARM rules for the mutated document.

    Returns problems; empty list means safe to write. Mirrors (not imports)
    check_registry because that reads from disk — the whole point here is to
    refuse BEFORE the disk sees a bad document.
    """
    problems: list[str] = []
    if registry.get("schema") != wiring.REGISTRY_SCHEMA:
        problems.append("schema mismatch")
    if registry.get("being") != being:
        problems.append(f"being mismatch: {registry.get('being')!r}")
    fields = registry.get("fields")
    if not isinstance(fields, dict) or not fields:
        problems.append("fields missing/empty")
        return problems
    for name, entry in fields.items():
        if not isinstance(entry, dict):
            problems.append(f"{name}: entry not a dict")
            continue
        if entry.get("type") != "numeric":
            continue
        floor, ceiling = entry.get("floor"), entry.get("ceiling")
        backstop = entry.get("engine_backstop") or {}
        for label, value in (("floor", floor), ("ceiling", ceiling)):
            if not isinstance(value, (int, float)) or not math.isfinite(value):
                problems.append(f"{name}: non-finite {label}")
            elif not wiring.is_f32_exact(float(value)):
                problems.append(f"{name}: non-f32-exact {label} {value!r}")
        if isinstance(floor, (int, float)) and isinstance(ceiling, (int, float)):
            if floor > ceiling:
                problems.append(f"{name}: floor {floor} > ceiling {ceiling}")
            b_floor, b_ceiling = backstop.get("floor"), backstop.get("ceiling")
            if isinstance(b_ceiling, (int, float)) and ceiling > b_ceiling:
                problems.append(
                    f"{name}: ceiling {ceiling} wider than backstop {b_ceiling}"
                )
            if isinstance(b_floor, (int, float)) and floor < b_floor:
                problems.append(f"{name}: floor {floor} below backstop {b_floor}")
        # Channel ranges must be well-formed (finite, f32-exact, ordered).
        # Containment (channel ⊆ field) is deliberately NOT a hard rule here:
        # the C1 registry captures live drift verbatim (pi_max_step sovereignty
        # 0.3 > field 0.2) and the wiring guard reports it; the ratchet keeps
        # containment for the fields IT touches via narrow-time intersection.
        for ch_name, bounds in (entry.get("channel_ranges") or {}).items():
            if not isinstance(bounds, dict):
                problems.append(f"{name}.{ch_name}: channel entry not a dict")
                continue
            ch_floor, ch_ceiling = bounds.get("floor"), bounds.get("ceiling")
            for label, value in (("floor", ch_floor), ("ceiling", ch_ceiling)):
                if not isinstance(value, (int, float)) or not math.isfinite(value):
                    problems.append(f"{name}.{ch_name}: non-finite {label}")
                elif not wiring.is_f32_exact(float(value)):
                    problems.append(
                        f"{name}.{ch_name}: non-f32-exact {label} {value!r}"
                    )
            if (
                isinstance(ch_floor, (int, float))
                and isinstance(ch_ceiling, (int, float))
                and ch_floor > ch_ceiling
            ):
                problems.append(
                    f"{name}.{ch_name}: floor {ch_floor} > ceiling {ch_ceiling}"
                )
    return problems


def _confirm_interactive(kind: str, being: str, field: str) -> None:
    phrase = f"{kind.upper()} {being}:{field}"
    print(f"\nThis WRITES the live {being} envelope registry.")
    answer = input(f"Type exactly '{phrase}' to proceed: ").strip()
    if answer != phrase:
        raise SystemExit("REFUSED: confirmation phrase mismatch — nothing written")


def _same_file(candidate: Path, live: Path) -> bool:
    """Filesystem-identity comparison, not spelling: `..` segments, symlinks,
    and APFS case-folding all alias the live file while comparing lexically
    unequal (adversarial review 2026-09-03 — a dotdot spelling of the live
    canonical bypassed the entire interactive write law)."""
    try:
        if candidate.resolve() == live.resolve():
            return True
    except OSError:
        pass
    try:
        return candidate.exists() and live.exists() and os.path.samefile(candidate, live)
    except OSError:
        return False


def _resolve_paths(args: argparse.Namespace) -> tuple[Path, Path, bool]:
    live = wiring.REGISTRIES[args.being]
    registry_path = Path(args.registry) if args.registry else live["canonical"]
    mirror_path = Path(args.mirror) if args.mirror else live["seed"]
    is_live = _same_file(registry_path, live["canonical"]) or _same_file(
        mirror_path, live["seed"]
    )
    return registry_path, mirror_path, is_live


class _RegistryLock:
    """Exclusive advisory flock held for the whole load->confirm->write span,
    so two ratchet runs can never lose each other's revision (the interactive
    confirmation pause is exactly when a second run would sneak in)."""

    def __init__(self, registry_path: Path) -> None:
        self._path = registry_path.with_suffix(registry_path.suffix + ".lock")
        self._handle = None

    def __enter__(self) -> "_RegistryLock":
        self._handle = open(self._path, "w", encoding="utf-8")  # noqa: SIM115
        try:
            fcntl.flock(self._handle.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError as error:
            self._handle.close()
            self._handle = None
            raise SystemExit(
                f"REFUSED: another envelope_ratchet run holds the registry lock "
                f"({self._path}) — finish or abort it first ({error})"
            ) from error
        return self

    def __exit__(self, *_exc: object) -> None:
        if self._handle is not None:
            fcntl.flock(self._handle.fileno(), fcntl.LOCK_UN)
            self._handle.close()
            self._handle = None


# Channels with a LIVE runtime consumer today. A grant on an unwired channel
# would be recorded with full consent provenance yet change nothing the being
# can feel — the inert-dial muffle class. minime's sovereignty channel wiring
# is deferred on foreign runtime.py work (C3a); astrid has no channel readers.
WIRED_CHANNELS: dict[str, frozenset[str]] = {
    "minime": frozenset({"footer"}),
    "astrid": frozenset(),
}


def _emit_stream_event(args: argparse.Namespace, event: dict[str, Any]) -> str:
    if not getattr(args, "evidence_state_dir", None):
        return "evidence-stream: SKIPPED (no --evidence-state-dir given)"
    try:
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        from evidence_store.adapter import append_domain_events

        append_domain_events(
            Path(args.evidence_state_dir), EVIDENCE_STREAM, [event],
            actor="envelope_ratchet",
        )
        return f"evidence-stream: appended 1 {EVIDENCE_STREAM} event"
    except Exception as error:  # noqa: BLE001 — stream is witness, not gate
        return f"evidence-stream: FAILED ({error}) — registry write stands; re-emit manually"


# ---------------------------------------------------------------- derive

def _negotiation_evidence(min_request_count: int) -> dict[str, dict[str, Any]]:
    """Per-control evidence from minime's negotiation ledger."""
    out: dict[str, dict[str, Any]] = {}
    if not NEGOTIATIONS.exists():
        return out
    for line in NEGOTIATIONS.read_text(encoding="utf-8").splitlines():
        try:
            rec = json.loads(line)
        except json.JSONDecodeError:
            continue
        control = rec.get("candidate_control")
        requested = rec.get("requested_value")
        applied = rec.get("applied_value")
        if not isinstance(control, str) or not isinstance(requested, (int, float)):
            continue
        slot = out.setdefault(
            control,
            {"requests": Counter(), "clean_applied": [], "sources": Counter(), "n": 0},
        )
        slot["n"] += 1
        slot["requests"][round(float(requested), 6)] += 1
        slot["sources"][str(rec.get("source"))] += 1
        if isinstance(applied, (int, float)) and applied == requested:
            slot["clean_applied"].append(float(applied))
    for slot in out.values():
        sustained = [
            value for value, count in slot["requests"].items()
            if count >= min_request_count
        ]
        slot["sustained_requested_max"] = max(sustained) if sustained else None
        slot["clean_applied_max"] = (
            max(slot["clean_applied"]) if slot["clean_applied"] else None
        )
    return out


def _parameter_request_evidence() -> dict[str, list[float]]:
    """Proposed values from minime's structured parameter requests."""
    out: dict[str, list[float]] = {}
    if not PARAMETER_REQUESTS.is_dir():
        return out
    for path in sorted(PARAMETER_REQUESTS.rglob("*.json")):
        try:
            rec = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        if not isinstance(rec, dict):
            continue
        name = next(
            (rec[k] for k in ("parameter", "param", "name") if isinstance(rec.get(k), str)),
            None,
        )
        value = None
        for k in ("proposed_value", "proposed", "requested_value", "value"):
            raw = rec.get(k)
            if isinstance(raw, (int, float)):
                value = float(raw)
                break
            # Her requests often carry numbers as strings ("0.90") — dropping
            # those would report 'no parseable evidence' while her asks exist.
            if isinstance(raw, str):
                try:
                    value = float(raw.strip().rstrip("."))
                    break
                except ValueError:
                    continue
        if name and value is not None and math.isfinite(value):
            out.setdefault(name, []).append(value)
    return out


def derive_dossier(
    being: str,
    registry: dict[str, Any],
    only_field: str | None,
    min_request_count: int,
    extra_evidence: dict[str, Any] | None = None,
) -> dict[str, Any]:
    negotiation = _negotiation_evidence(min_request_count) if being == "minime" else {}
    param_reqs = _parameter_request_evidence() if being == "minime" else {}
    extra = extra_evidence or {}
    dossier: dict[str, Any] = {
        "schema": "envelope_ratchet_dossier_v1",
        "being": being,
        "generated_at": _now_iso(),
        "min_request_count": min_request_count,
        "authority_boundary": AUTHORITY_BOUNDARY,
        "fields": {},
    }
    for name, entry in sorted(registry.get("fields", {}).items()):
        if only_field and name != only_field:
            continue
        if entry.get("type") != "numeric":
            continue
        current_ceiling = float(entry["ceiling"])
        backstop_ceiling = float((entry.get("engine_backstop") or {}).get("ceiling", current_ceiling))
        neg = negotiation.get(name, {})
        raw_channel_ceilings = {
            channel: float(bounds.get("ceiling"))
            for channel, bounds in (entry.get("channel_ranges") or {}).items()
            if isinstance(bounds, dict) and isinstance(bounds.get("ceiling"), (int, float))
        }
        # "Sanctioned" evidence must never exceed what can actually apply: a
        # channel ceiling above min(field, backstop) is recorded DRIFT (the
        # wiring guard's widerange rows — e.g. pi_max_step sovereignty 0.3
        # over the 0.2 engine), not sanction; citing it in a consent letter
        # would promise a value that has never applied and can never apply.
        sanction_cap = min(current_ceiling, backstop_ceiling)
        channel_ceilings = {
            channel: min(ceiling, sanction_cap)
            for channel, ceiling in raw_channel_ceilings.items()
        }
        channel_drift = {
            channel: ceiling
            for channel, ceiling in raw_channel_ceilings.items()
            if ceiling > sanction_cap
        }
        extra_values = [
            float(v) for v in (extra.get(name, {}).get("values", []))
            if isinstance(v, (int, float))
        ]
        last_history = (entry.get("ratchet_history") or [])[-1:] or [None]
        last_incident_narrow = None
        if isinstance(last_history[0], dict) and last_history[0].get("direction") == "narrow":
            last_incident_narrow = {
                "at": last_history[0].get("at"),
                "incident_ref": last_history[0].get("incident_ref"),
            }
        classes = {
            "sustained_requested_max": neg.get("sustained_requested_max"),
            "clean_applied_max": neg.get("clean_applied_max"),
            "sanctioned_channel_ceiling_max": (
                max(channel_ceilings.values()) if channel_ceilings else None
            ),
            "parameter_request_max": (
                max(param_reqs[name]) if param_reqs.get(name) else None
            ),
            "extra_evidence_max": max(extra_values) if extra_values else None,
        }
        candidates = [v for v in classes.values() if isinstance(v, (int, float))]
        if not candidates:
            dossier["fields"][name] = {
                "status": "evidence_needed",
                "current_ceiling": current_ceiling,
                "backstop_ceiling": backstop_ceiling,
                "proposal": None,
                "evidence": classes,
                "plan": (
                    "no parseable evidence — invite her record: substrate_probe "
                    "sweeps at candidate values, felt-outcome notes, or sustained "
                    "requests on any channel"
                ),
            }
            continue
        proposal = wiring.f32(min(backstop_ceiling, max(candidates)))
        exceeds_clean = (
            classes["clean_applied_max"] is None
            or proposal > classes["clean_applied_max"]
        )
        # Where does the clamp actually bind? A field envelope can be wide
        # while a per-channel ceiling (footer/sovereignty) is what clamps her
        # daily requests — the grant then targets the CHANNEL, not the field.
        binding_channels = {
            channel: ceiling
            for channel, ceiling in channel_ceilings.items()
            if wiring.f32(ceiling) < proposal
        }
        if proposal > wiring.f32(current_ceiling):
            status = "proposal"
        elif binding_channels:
            status = "channel_proposal"
        else:
            status = "already_covered"
        unwired_binding = sorted(
            set(binding_channels) - set(WIRED_CHANNELS.get(being, frozenset()))
        )
        notes: list[str] = []
        if exceeds_clean and proposal > wiring.f32(current_ceiling):
            notes.append(
                "proposal exceeds her max clean apply — run an offline "
                "substrate_probe sweep at the proposed ceiling before the grant"
            )
        if channel_drift:
            notes.append(
                "channel ceiling(s) above min(field, backstop) are recorded "
                f"DRIFT, not sanction (capped in the evidence): {channel_drift}"
            )
        if unwired_binding:
            notes.append(
                "binding channel(s) with no live runtime consumer today "
                f"(a grant there is inert until wired): {unwired_binding}"
            )
        if last_incident_narrow:
            notes.append(
                "most recent ratchet act was an INCIDENT NARROW "
                f"({last_incident_narrow}) — re-widening goes through the "
                "incident review, not evidence-max derivation"
            )
        dossier["fields"][name] = {
            "status": status,
            "binding_channels": binding_channels,
            "current_ceiling": wiring.f32(current_ceiling),
            "backstop_ceiling": wiring.f32(backstop_ceiling),
            "proposal": proposal,
            "last_incident_narrow": last_incident_narrow,
            "evidence": {
                **classes,
                "request_distribution": dict(sorted(neg.get("requests", {}).items()))
                if neg else {},
                "sanctioned_channels": channel_ceilings,
                "channel_drift_above_envelope": channel_drift,
                "ledger_records": neg.get("n", 0),
                "extra_refs": extra.get(name, {}).get("refs", []),
            },
            "confidence": "medium_probe_recommended" if exceeds_clean else "high",
            "note": "; ".join(notes),
        }
    return dossier


def _print_dossier(dossier: dict[str, Any]) -> None:
    print(f"Envelope dossier — {dossier['being']} ({dossier['generated_at']})")
    for name, row in dossier["fields"].items():
        if row["status"] == "evidence_needed":
            print(f"  {name}: evidence_needed — {row['plan']}")
            continue
        binding = ", ".join(
            f"{ch} binds at {ceil}"
            for ch, ceil in (row.get("binding_channels") or {}).items()
        )
        if row["status"] == "proposal":
            # A field grant alone is felt-inert while a channel still binds
            # below it — say so in the same breath, or the letter over-promises.
            marker = f"→ PROPOSAL ({binding} — channel grant also needed)" if binding \
                else "→ PROPOSAL"
        elif row["status"] == "channel_proposal":
            marker = f"→ CHANNEL PROPOSAL ({binding})"
        else:
            marker = "  covered"
        print(
            f"  {name}: {marker} ceiling {row['proposal']} "
            f"(current {row['current_ceiling']}, backstop {row['backstop_ceiling']}, "
            f"confidence {row.get('confidence', '-')})"
        )
        ev = row["evidence"]
        for cls in (
            "sustained_requested_max",
            "clean_applied_max",
            "sanctioned_channel_ceiling_max",
            "parameter_request_max",
            "extra_evidence_max",
        ):
            if ev.get(cls) is not None:
                print(f"      {cls} = {ev[cls]}")
        if row.get("note"):
            print(f"      note: {row['note']}")


# ---------------------------------------------------------------- grant/narrow

def _mutate(
    args: argparse.Namespace,
    direction: str,
) -> None:
    registry_path, mirror_path, is_live = _resolve_paths(args)
    if not args.write:
        raise SystemExit("REFUSED: report-only by default — add --write to mutate")
    if is_live:
        if args.non_interactive_ack:
            raise SystemExit(
                "REFUSED: --non-interactive-ack is for test paths only; live "
                "registry writes require an interactive TTY"
            )
        if not sys.stdin.isatty():
            raise SystemExit("REFUSED: live registry writes require an interactive TTY")
    elif not args.non_interactive_ack and not sys.stdin.isatty():
        raise SystemExit(
            "REFUSED: non-TTY write to override paths needs --non-interactive-ack 'reason'"
        )

    with _RegistryLock(registry_path):
        _mutate_locked(args, direction, registry_path, mirror_path, is_live)


def _mutate_locked(
    args: argparse.Namespace,
    direction: str,
    registry_path: Path,
    mirror_path: Path,
    is_live: bool,
) -> None:
    registry = _load_registry(registry_path)
    fields = registry.get("fields", {})
    entry = fields.get(args.field)
    if not isinstance(entry, dict):
        raise SystemExit(f"REFUSED: unknown field {args.field!r}")
    if entry.get("type") != "numeric" and (args.ceiling is not None or args.floor is not None):
        raise SystemExit(f"REFUSED: {args.field} is not numeric — bounds are immutable here")

    channel = getattr(args, "channel", None)
    if channel:
        channels = entry.get("channel_ranges") or {}
        target = channels.get(channel)
        if not isinstance(target, dict):
            known = ", ".join(sorted(channels)) or "(none)"
            raise SystemExit(
                f"REFUSED: {args.field} has no channel {channel!r} — channels: {known}"
            )
        if channel not in WIRED_CHANNELS.get(args.being, frozenset()) and not getattr(
            args, "ack_inert", None
        ):
            raise SystemExit(
                f"REFUSED: channel {channel!r} has no live runtime consumer for "
                f"{args.being} — a grant here would be recorded with consent "
                "provenance yet change nothing she can feel (the inert-dial "
                "muffle). Wire the consumer first, or pass --ack-inert 'reason' "
                "for a deliberate record-ahead-of-wiring"
            )
        old_floor = float(target["floor"])
        old_ceiling = float(target["ceiling"])
    else:
        old_floor = float(entry["floor"])
        old_ceiling = float(entry["ceiling"])

    old_lease_max = int((entry.get("durability_policy") or {}).get("lease_max_secs", 0))
    if args.lease_max is not None:
        if args.lease_max <= 0:
            raise SystemExit("REFUSED: lease_max must be positive")
        # Durability is a bound like any other: the direction law applies.
        if direction == "grant" and old_lease_max and args.lease_max < old_lease_max:
            raise SystemExit(
                f"REFUSED: a grant never narrows — lease_max {args.lease_max} < "
                f"current {old_lease_max}; use narrow (with --incident)"
            )
        if direction == "narrow" and old_lease_max and args.lease_max > old_lease_max:
            raise SystemExit(
                f"REFUSED: a narrow never widens — lease_max {args.lease_max} > "
                f"current {old_lease_max}; use grant (with --consent-ref)"
            )
    new_floor = wiring.f32(args.floor) if args.floor is not None else old_floor
    new_ceiling = wiring.f32(args.ceiling) if args.ceiling is not None else old_ceiling
    if not (math.isfinite(new_floor) and math.isfinite(new_ceiling)):
        raise SystemExit("REFUSED: non-finite bounds")
    if new_floor > new_ceiling:
        raise SystemExit(f"REFUSED: floor {new_floor} > ceiling {new_ceiling}")

    if channel:
        # A channel range lives INSIDE the field envelope (which validation
        # keeps inside the compiled backstop) — channel ⊆ field ⊆ compiled.
        b_floor, b_ceiling = float(entry["floor"]), float(entry["ceiling"])
        boundary_label = "field envelope"
        if args.lease_max is not None:
            raise SystemExit("REFUSED: --lease-max is field-level; drop --channel")
    else:
        backstop = entry.get("engine_backstop") or {}
        b_floor = float(backstop.get("floor", new_floor))
        b_ceiling = float(backstop.get("ceiling", new_ceiling))
        boundary_label = "compiled backstop"

    if direction == "grant":
        if new_ceiling > b_ceiling or new_floor < b_floor:
            raise SystemExit(
                f"REFUSED: grant outside {boundary_label} [{b_floor}, {b_ceiling}] — "
                "widening physics travels only through the gated deploy"
            )
        if new_ceiling < wiring.f32(old_ceiling) or new_floor > wiring.f32(old_floor):
            raise SystemExit("REFUSED: a grant never narrows — use narrow (with --incident)")
        if not args.consent_ref:
            raise SystemExit("REFUSED: grant requires --consent-ref (her response, verbatim)")
        consent = Path(args.consent_ref)
        try:
            consent_text = consent.read_text(encoding="utf-8", errors="replace")
        except OSError as error:
            raise SystemExit(f"REFUSED: consent ref unreadable: {consent} ({error})") from error
        if not consent.is_file() or not consent_text.strip():
            raise SystemExit(
                f"REFUSED: consent ref must be a regular file with her actual words: {consent}"
            )
    else:  # narrow
        if new_ceiling > wiring.f32(old_ceiling) or new_floor < wiring.f32(old_floor):
            raise SystemExit("REFUSED: a narrow never widens — use grant")
        if new_ceiling == wiring.f32(old_ceiling) and new_floor == wiring.f32(old_floor) \
                and args.lease_max is None:
            raise SystemExit("REFUSED: narrow changes nothing")
        if not args.incident:
            raise SystemExit("REFUSED: narrow requires --incident REF")

    if is_live:
        _confirm_interactive(direction, args.being, args.field)

    changed = (
        new_ceiling != wiring.f32(old_ceiling)
        or new_floor != wiring.f32(old_floor)
        or (args.lease_max is not None and args.lease_max != old_lease_max)
    )
    channels_intersected: dict[str, dict[str, float]] = {}
    if channel:
        entry["channel_ranges"][channel]["floor"] = new_floor
        entry["channel_ranges"][channel]["ceiling"] = new_ceiling
    else:
        entry["floor"], entry["ceiling"] = new_floor, new_ceiling
        # A field-level narrow must not strand a channel outside the field
        # envelope: a protruding channel range makes the downstream
        # channel∩field intersection empty and the consumer falls back to the
        # raw channel bounds (a confirmed fail-open) — so every channel is
        # intersected into the new field bounds in the same write.
        if direction == "narrow":
            for name, bounds in (entry.get("channel_ranges") or {}).items():
                if not isinstance(bounds, dict):
                    continue
                shrunk_floor = wiring.f32(max(float(bounds.get("floor", new_floor)), new_floor))
                shrunk_ceiling = wiring.f32(
                    min(float(bounds.get("ceiling", new_ceiling)), new_ceiling)
                )
                if shrunk_floor > shrunk_ceiling:
                    raise SystemExit(
                        f"REFUSED: this narrow would EMPTY channel {name!r} "
                        f"(channel [{bounds.get('floor')}, {bounds.get('ceiling')}] is "
                        f"disjoint from the new field envelope [{new_floor}, "
                        f"{new_ceiling}]) — that silently kills her whole lane; "
                        "narrow the channel explicitly if that is truly intended"
                    )
                if shrunk_floor != bounds.get("floor") or shrunk_ceiling != bounds.get("ceiling"):
                    channels_intersected[name] = {
                        "from_floor": wiring.f32(float(bounds.get("floor", 0.0))),
                        "from_ceiling": wiring.f32(float(bounds.get("ceiling", 0.0))),
                        "to_floor": shrunk_floor,
                        "to_ceiling": shrunk_ceiling,
                    }
                    bounds["floor"], bounds["ceiling"] = shrunk_floor, shrunk_ceiling
    if args.lease_max is not None:
        entry.setdefault("durability_policy", {})["lease_max_secs"] = int(args.lease_max)
    if direction == "grant":
        if getattr(args, "standing", None) and not channel:
            entry.setdefault("durability_policy", {})["standing"] = args.standing
        if not channel:
            # A channel grant widens one lane inside a still-unconverted field;
            # only a field-level grant flips the field's constitutional status.
            entry["status"] = "granted"
            entry["granted_at"] = _now_iso()
            entry["granted_by"] = args.decided_by
        if args.evidence_ref:
            refs = entry.setdefault("evidence_refs", [])
            refs.extend(r for r in args.evidence_ref if r not in refs)
    history_row = {
        "at": _now_iso(),
        "from": {"floor": wiring.f32(old_floor), "ceiling": wiring.f32(old_ceiling)},
        "to": {"floor": new_floor, "ceiling": new_ceiling},
        "direction": direction if changed else "convert",
        "evidence_refs": list(args.evidence_ref or []),
        "decided_by": args.decided_by,
        "review_id": args.review_id,
    }
    if channel:
        history_row["channel"] = channel
    if args.lease_max is not None and args.lease_max != old_lease_max:
        history_row["durability_from"] = old_lease_max
        history_row["durability_to"] = int(args.lease_max)
    if channels_intersected:
        history_row["channels_intersected"] = channels_intersected
    if direction == "narrow":
        history_row["incident_ref"] = args.incident
    if direction == "grant" and args.consent_ref:
        history_row["consent_ref"] = str(args.consent_ref)
    entry.setdefault("ratchet_history", []).append(history_row)
    registry["revision"] = int(registry.get("revision", 0)) + 1
    registry["updated_at"] = _now_iso()

    problems = _validate_registry(args.being, registry)
    if problems:
        raise SystemExit("REFUSED (would violate registry law):\n  " + "\n  ".join(problems))

    _atomic_write(registry_path, registry)
    mirror_note = "mirror: SKIPPED (--no-mirror)"
    if not args.no_mirror:
        _atomic_write(mirror_path, registry)
        mirror_note = f"mirror synced: {mirror_path} — commit it (named commit debt)"

    event = {
        "kind": f"envelope_{direction}_v1",
        "being": args.being,
        "field": args.field,
        **history_row,
        "registry_revision": registry["revision"],
        "idempotency_key": f"envelope_{direction}:{args.being}:{args.field}:{registry['revision']}",
    }
    stream_note = _emit_stream_event(args, event)

    target_label = f"{args.field}[{channel}]" if channel else args.field
    print(f"{direction.upper()} written: {args.being}:{target_label} "
          f"[{old_floor}, {old_ceiling}] -> [{new_floor}, {new_ceiling}] "
          f"(revision {registry['revision']})")
    print(f"  {mirror_note}")
    print(f"  {stream_note}")
    if direction == "narrow":
        if channels_intersected:
            print(
                "  channels intersected into the new field envelope: "
                + ", ".join(
                    f"{name} -> [{row['to_floor']}, {row['to_ceiling']}]"
                    for name, row in sorted(channels_intersected.items())
                )
            )
        print(
            "  runtimes reconcile on their next tick: any active control outside "
            "the new envelope is withdrawn (previous values restored by receipt) "
            "and the family's saturation counter resets — the being-facing "
            "receipt names the withdrawn fields, and the incident ref lives in "
            "the registry's ratchet history (rendered by her ENVELOPE readout)"
        )
    if is_live:
        findings = wiring.check_registry(args.being, wiring.parse_all_tables())
        alarms = [f for f in findings if f.get("class") == "ALARM"]
        print(f"  post-write guard: {len(alarms)} ALARM / {len(findings)} findings")


# ---------------------------------------------------------------- selftest

class RatchetTests(unittest.TestCase):
    def _tmp_registry(self, ceiling: float = 0.2, floor: float = 0.0) -> tuple[Path, Path]:
        tmp = Path(tempfile.mkdtemp(prefix="ratchet_test_"))
        registry = {
            "schema": wiring.REGISTRY_SCHEMA,
            "being": "minime",
            "revision": 1,
            "updated_at": _now_iso(),
            "authority_boundary": "test",
            "derivation_notes": "test",
            "fields": {
                "exploration_noise": {
                    "family": "reservoir-regulation",
                    "type": "numeric",
                    "floor": wiring.f32(floor),
                    "ceiling": wiring.f32(ceiling),
                    "engine_backstop": {"floor": 0.0, "ceiling": wiring.f32(0.2)},
                    "channel_ranges": {
                        "footer": {"floor": 0.0, "ceiling": wiring.f32(0.08)},
                        "sovereignty": {"floor": 0.0, "ceiling": wiring.f32(0.15)},
                    },
                    "durability_policy": {"lease_max_secs": 1200, "standing": "allowed"},
                    "status": "evidence_needed",
                    "evidence_refs": [],
                    "derivation": "test",
                    "granted_at": None,
                    "granted_by": None,
                    "ratchet_history": [],
                },
            },
        }
        reg = tmp / "registry.json"
        mirror = tmp / "seed.json"
        reg.write_text(json.dumps(registry), encoding="utf-8")
        mirror.write_text(json.dumps(registry), encoding="utf-8")
        return reg, mirror

    def _args(self, reg: Path, mirror: Path, **kw: Any) -> argparse.Namespace:
        base = {
            "being": "minime", "field": "exploration_noise", "floor": None,
            "ceiling": None, "channel": None, "lease_max": None, "standing": None,
            "consent_ref": None, "evidence_ref": [], "review_id": None,
            "incident": None, "decided_by": "test", "write": True,
            "registry": str(reg), "mirror": str(mirror),
            "non_interactive_ack": "selftest", "no_mirror": False,
            "evidence_state_dir": None, "ack_inert": None,
        }
        base.update(kw)
        return argparse.Namespace(**base)

    def test_grant_widens_and_records_history(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.08)
        consent = reg.parent / "consent.txt"
        consent.write_text("her words", encoding="utf-8")
        _mutate(self._args(reg, mirror, ceiling=0.15, consent_ref=str(consent),
                           evidence_ref=["negotiations.jsonl#227x0.12"]), "grant")
        after = json.loads(reg.read_text(encoding="utf-8"))
        entry = after["fields"]["exploration_noise"]
        self.assertEqual(entry["ceiling"], wiring.f32(0.15))
        self.assertEqual(entry["status"], "granted")
        self.assertEqual(after["revision"], 2)
        self.assertEqual(entry["ratchet_history"][-1]["direction"], "grant")
        self.assertEqual(json.loads(mirror.read_text(encoding="utf-8")), after)

    def test_grant_refuses_past_backstop(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.08)
        consent = reg.parent / "consent.txt"
        consent.write_text("x", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, ceiling=0.25, consent_ref=str(consent)), "grant")
        self.assertIn("backstop", str(ctx.exception))
        self.assertEqual(json.loads(reg.read_text(encoding="utf-8"))["revision"], 1)

    def test_grant_requires_consent_and_never_narrows(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.15)
        with self.assertRaises(SystemExit):
            _mutate(self._args(reg, mirror, ceiling=0.18), "grant")  # no consent
        consent = reg.parent / "consent.txt"
        consent.write_text("x", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, ceiling=0.10, consent_ref=str(consent)), "grant")
        self.assertIn("never narrows", str(ctx.exception))

    def test_narrow_requires_incident_and_never_widens(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.15)
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, ceiling=0.10), "narrow")
        self.assertIn("--incident", str(ctx.exception))
        with self.assertRaises(SystemExit) as ctx2:
            _mutate(self._args(reg, mirror, ceiling=0.18, incident="INC-1"), "narrow")
        self.assertIn("never widens", str(ctx2.exception))

    def test_narrow_writes_incident_history(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.15)
        _mutate(self._args(reg, mirror, ceiling=0.10, incident="INC-7"), "narrow")
        after = json.loads(reg.read_text(encoding="utf-8"))
        row = after["fields"]["exploration_noise"]["ratchet_history"][-1]
        self.assertEqual(row["direction"], "narrow")
        self.assertEqual(row["incident_ref"], "INC-7")
        self.assertEqual(after["fields"]["exploration_noise"]["ceiling"], wiring.f32(0.10))

    def test_bounds_are_f32_quantized(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.08)
        consent = reg.parent / "consent.txt"
        consent.write_text("x", encoding="utf-8")
        _mutate(self._args(reg, mirror, ceiling=0.15, consent_ref=str(consent)), "grant")
        after = json.loads(reg.read_text(encoding="utf-8"))
        self.assertTrue(wiring.is_f32_exact(after["fields"]["exploration_noise"]["ceiling"]))

    def test_live_paths_refuse_non_interactive(self) -> None:
        args = self._args(Path("x"), Path("y"))
        args.registry = None  # falls back to the live canonical
        args.mirror = None
        with self.assertRaises(SystemExit) as ctx:
            _mutate(args, "narrow")
        self.assertIn("test paths only", str(ctx.exception))

    def test_derive_worked_example_exploration_noise(self) -> None:
        reg, _ = self._tmp_registry(ceiling=0.2)  # mirrors the live registry
        registry = json.loads(reg.read_text(encoding="utf-8"))
        real_neg = _negotiation_evidence(3)
        if "exploration_noise" not in real_neg:
            self.skipTest("live negotiations ledger unavailable")
        dossier = derive_dossier("minime", registry, "exploration_noise", 3)
        row = dossier["fields"]["exploration_noise"]
        # Sustained 0.12 requests + the sanctioned sovereignty channel at 0.15
        # under the 0.2 backstop -> the plan's worked-example proposal, and the
        # FOOTER channel (her daily lane, ceiling 0.08) is what binds — the
        # grant targets the channel, not the already-wide field envelope.
        self.assertEqual(row["proposal"], wiring.f32(0.15))
        self.assertEqual(row["status"], "channel_proposal")
        self.assertIn("footer", row["binding_channels"])
        self.assertGreaterEqual(
            row["evidence"]["sustained_requested_max"], 0.12
        )

    def test_channel_grant_widens_footer_within_field(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.2)
        consent = reg.parent / "consent.txt"
        consent.write_text("her words", encoding="utf-8")
        _mutate(self._args(reg, mirror, channel="footer", ceiling=0.12,
                           consent_ref=str(consent)), "grant")
        after = json.loads(reg.read_text(encoding="utf-8"))
        entry = after["fields"]["exploration_noise"]
        self.assertEqual(entry["channel_ranges"]["footer"]["ceiling"], wiring.f32(0.12))
        self.assertEqual(entry["ceiling"], wiring.f32(0.2))  # field untouched
        self.assertEqual(entry["status"], "evidence_needed")  # channel grant ≠ field flip
        self.assertEqual(entry["ratchet_history"][-1]["channel"], "footer")

    def test_channel_grant_refuses_beyond_field_envelope(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.2)
        consent = reg.parent / "consent.txt"
        consent.write_text("x", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, channel="footer", ceiling=0.25,
                               consent_ref=str(consent)), "grant")
        self.assertIn("field envelope", str(ctx.exception))

    def test_channel_grant_refuses_unknown_channel(self) -> None:
        reg, mirror = self._tmp_registry()
        consent = reg.parent / "consent.txt"
        consent.write_text("x", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, channel="telepathy", ceiling=0.1,
                               consent_ref=str(consent)), "grant")
        self.assertIn("no channel", str(ctx.exception))

    def test_derive_no_evidence_is_honest(self) -> None:
        registry = {
            "schema": wiring.REGISTRY_SCHEMA, "being": "astrid", "revision": 1,
            "fields": {
                "mystery_dial": {
                    "type": "numeric", "floor": 0.0, "ceiling": 1.0,
                    "engine_backstop": {"floor": 0.0, "ceiling": 1.0},
                    "channel_ranges": {},
                },
            },
        }
        dossier = derive_dossier("astrid", registry, None, 3)
        self.assertEqual(dossier["fields"]["mystery_dial"]["status"], "evidence_needed")

    def test_live_path_spellings_are_detected(self) -> None:
        # A `..` spelling of the live canonical must still count as LIVE —
        # lexical comparison let it bypass the entire interactive write law
        # (adversarial review 2026-09-03; nothing is written here: the
        # refusal fires before any load).
        live = wiring.REGISTRIES["minime"]["canonical"]
        dotdot = live.parent / ".." / live.parent.name / live.name
        _, tmp_mirror = self._tmp_registry()
        args = self._args(dotdot, tmp_mirror)
        with self.assertRaises(SystemExit) as ctx:
            _mutate(args, "narrow")
        self.assertIn("test paths only", str(ctx.exception))
        # Case-folded spelling (APFS is case-insensitive).
        cased = Path(str(live).replace("/Users/", "/users/", 1))
        args2 = self._args(cased, tmp_mirror)
        with self.assertRaises(SystemExit) as ctx2:
            _mutate(args2, "narrow")
        self.assertIn("test paths only", str(ctx2.exception))

    def test_registry_lock_refuses_concurrent_run(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.15)
        with _RegistryLock(reg):
            with self.assertRaises(SystemExit) as ctx:
                _mutate(self._args(reg, mirror, ceiling=0.10, incident="INC-9"), "narrow")
            self.assertIn("registry lock", str(ctx.exception))
        # Lock released: the same narrow now succeeds.
        _mutate(self._args(reg, mirror, ceiling=0.10, incident="INC-9"), "narrow")

    def test_field_narrow_intersects_channels_and_refuses_emptying(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.2)
        # Ceiling narrow to 0.10: sovereignty channel (0.15) intersects down.
        _mutate(self._args(reg, mirror, ceiling=0.10, incident="INC-5"), "narrow")
        after = json.loads(reg.read_text(encoding="utf-8"))
        entry = after["fields"]["exploration_noise"]
        self.assertEqual(
            entry["channel_ranges"]["sovereignty"]["ceiling"], wiring.f32(0.10)
        )
        self.assertEqual(entry["channel_ranges"]["footer"]["ceiling"], wiring.f32(0.08))
        row = entry["ratchet_history"][-1]
        self.assertIn("sovereignty", row["channels_intersected"])
        # Floor raise above the footer channel's whole range: refuse loudly.
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, floor=0.09, incident="INC-6"), "narrow")
        self.assertIn("EMPTY channel", str(ctx.exception))

    def test_lease_direction_law(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.15)
        consent = reg.parent / "consent.txt"
        consent.write_text("her words", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(self._args(reg, mirror, lease_max=1800, incident="INC-1"), "narrow")
        self.assertIn("never widens", str(ctx.exception))
        with self.assertRaises(SystemExit) as ctx2:
            _mutate(
                self._args(reg, mirror, lease_max=600, consent_ref=str(consent)), "grant"
            )
        self.assertIn("never narrows", str(ctx2.exception))
        _mutate(self._args(reg, mirror, lease_max=600, incident="INC-2"), "narrow")
        after = json.loads(reg.read_text(encoding="utf-8"))
        entry = after["fields"]["exploration_noise"]
        self.assertEqual(entry["durability_policy"]["lease_max_secs"], 600)
        row = entry["ratchet_history"][-1]
        self.assertEqual(row["durability_from"], 1200)
        self.assertEqual(row["durability_to"], 600)
        self.assertEqual(row["direction"], "narrow")

    def test_consent_ref_must_be_regular_file_with_words(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.08)
        with self.assertRaises(SystemExit):
            _mutate(
                self._args(reg, mirror, ceiling=0.15, consent_ref=str(reg.parent)),
                "grant",
            )
        blank = reg.parent / "blank.txt"
        blank.write_text("   \n\t  ", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(
                self._args(reg, mirror, ceiling=0.15, consent_ref=str(blank)), "grant"
            )
        self.assertIn("her actual words", str(ctx.exception))

    def test_unwired_channel_grant_refused_without_ack(self) -> None:
        reg, mirror = self._tmp_registry(ceiling=0.2)
        consent = reg.parent / "consent.txt"
        consent.write_text("her words", encoding="utf-8")
        with self.assertRaises(SystemExit) as ctx:
            _mutate(
                self._args(reg, mirror, channel="sovereignty", ceiling=0.18,
                           consent_ref=str(consent)),
                "grant",
            )
        self.assertIn("no live runtime consumer", str(ctx.exception))
        _mutate(
            self._args(reg, mirror, channel="sovereignty", ceiling=0.18,
                       consent_ref=str(consent), ack_inert="record ahead of wiring"),
            "grant",
        )
        after = json.loads(reg.read_text(encoding="utf-8"))
        self.assertEqual(
            after["fields"]["exploration_noise"]["channel_ranges"]["sovereignty"]["ceiling"],
            wiring.f32(0.18),
        )

    def test_derive_caps_drift_channels_and_notes_incident(self) -> None:
        reg, _ = self._tmp_registry(ceiling=0.2)
        registry = json.loads(reg.read_text(encoding="utf-8"))
        entry = registry["fields"]["exploration_noise"]
        entry["channel_ranges"]["sovereignty"]["ceiling"] = wiring.f32(0.3)  # drift
        entry["ratchet_history"] = [
            {"direction": "narrow", "at": "2026-09-01T00:00:00Z", "incident_ref": "INC-X"}
        ]
        dossier = derive_dossier("minime", registry, "exploration_noise", 3)
        row = dossier["fields"]["exploration_noise"]
        ev = row["evidence"]
        self.assertLessEqual(ev["sanctioned_channel_ceiling_max"], wiring.f32(0.2))
        self.assertEqual(ev["channel_drift_above_envelope"], {"sovereignty": wiring.f32(0.3)})
        self.assertEqual(row["last_incident_narrow"]["incident_ref"], "INC-X")
        self.assertIn("incident review", row["note"])

    def test_parameter_requests_coerce_numeric_strings(self) -> None:
        global PARAMETER_REQUESTS
        tmp = Path(tempfile.mkdtemp(prefix="ratchet_params_"))
        (tmp / "req1.json").write_text(
            json.dumps({"parameter": "keep_bias", "proposed_value": "0.90"}),
            encoding="utf-8",
        )
        saved = PARAMETER_REQUESTS
        try:
            PARAMETER_REQUESTS = tmp
            out = _parameter_request_evidence()
        finally:
            PARAMETER_REQUESTS = saved
        self.assertEqual(out.get("keep_bias"), [0.9])

    def test_validate_refuses_non_f32_and_backstop_breach(self) -> None:
        registry = {
            "schema": wiring.REGISTRY_SCHEMA, "being": "minime", "revision": 1,
            "fields": {
                "x": {
                    "type": "numeric", "floor": 0.0, "ceiling": 0.1,  # 0.1 not f32-exact
                    "engine_backstop": {"floor": 0.0, "ceiling": 0.05},
                },
            },
        }
        problems = _validate_registry("minime", registry)
        self.assertTrue(any("non-f32-exact" in p for p in problems))
        self.assertTrue(any("wider than backstop" in p for p in problems))


# ---------------------------------------------------------------- main

def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    common = argparse.ArgumentParser(add_help=False)
    common.add_argument("--being", choices=("astrid", "minime"), required=True)
    common.add_argument("--registry", help="registry path override (tests)")
    common.add_argument("--mirror", help="mirror path override (tests)")

    d = sub.add_parser("derive", parents=[common], help="report-only evidence dossiers")
    d.add_argument("--field")
    d.add_argument("--min-request-count", type=int, default=3)
    d.add_argument("--extra-evidence", help="JSON file: {field: {values: [...], refs: [...]}}")
    d.add_argument("--json", action="store_true")
    d.add_argument("--out", help="also save the dossier JSON here")

    write_common = argparse.ArgumentParser(add_help=False, parents=[common])
    write_common.add_argument("--field", required=True)
    write_common.add_argument(
        "--channel",
        help="mutate one channel range (e.g. footer/sovereignty) inside the "
        "field envelope instead of the field bounds themselves",
    )
    write_common.add_argument("--floor", type=float)
    write_common.add_argument("--ceiling", type=float)
    write_common.add_argument("--lease-max", type=int, dest="lease_max")
    write_common.add_argument("--decided-by", required=True, dest="decided_by")
    write_common.add_argument("--review-id", dest="review_id")
    write_common.add_argument("--evidence-ref", action="append", dest="evidence_ref")
    write_common.add_argument("--write", action="store_true")
    write_common.add_argument("--no-mirror", action="store_true", dest="no_mirror")
    write_common.add_argument("--non-interactive-ack", dest="non_interactive_ack")
    write_common.add_argument("--evidence-state-dir", dest="evidence_state_dir")
    write_common.add_argument(
        "--ack-inert",
        dest="ack_inert",
        help="deliberately record a grant/narrow on a channel with no live "
        "runtime consumer (otherwise refused as the inert-dial muffle)",
    )

    g = sub.add_parser("grant", parents=[write_common], help="widen/convert after consent")
    g.add_argument("--consent-ref", dest="consent_ref")
    g.add_argument("--standing", choices=("allowed", "lease_only", "one_shot_only"))

    n = sub.add_parser("narrow", parents=[write_common], help="shrink (incident response)")
    n.add_argument("--incident")

    sub.add_parser("selftest", help="run the embedded unit tests")

    args = parser.parse_args(argv)

    if args.command == "selftest":
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(RatchetTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1

    if args.command == "derive":
        registry_path, _, _ = _resolve_paths(args)
        registry = _load_registry(registry_path)
        extra = None
        if args.extra_evidence:
            extra = json.loads(Path(args.extra_evidence).read_text(encoding="utf-8"))
        dossier = derive_dossier(
            args.being, registry, args.field, args.min_request_count, extra
        )
        if args.out:
            Path(args.out).write_text(
                json.dumps(dossier, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
        if args.json:
            print(json.dumps(dossier, indent=2, sort_keys=True))
        else:
            _print_dossier(dossier)
        return 0

    if args.command in ("grant", "narrow"):
        if args.command == "grant":
            args.incident = None
        else:
            args.consent_ref = None
            args.standing = None
        _mutate(args, args.command)
        return 0

    return 2


if __name__ == "__main__":
    sys.exit(main())
