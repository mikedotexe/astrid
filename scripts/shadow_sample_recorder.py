#!/usr/bin/env python3
"""Quantified Shadow-v3 sample recorder (observation-only).

Closes the instrumentation gap surfaced by minime's shadow-persistence
experiment (introspection_minime_sensory_bus_1784873427 c004, sandbox trials
trial_90c1fc393c3e5b37 / trial_cb6da57f91f03b02, classification
qualitative_lattice_signal_needs_quantified_samples): the
shadow_influence_replay_v1 adapter needs "Shadow-v3 ... norm A->B ...
dispersal potential C->D" lines to project anything, but the only such lines
were the bridge prompt render, which never lands in durable public text
unless Astrid quotes it verbatim.

This recorder reads minime's live engine snapshot
(workspace/spectral_state.json, refreshed ~1 Hz), computes the SAME last-8
window trend the bridge renders (field_norm + fissure_tendency head->tail,
see spectral_viz.rs::history_trend_segment), and appends one quantified line
per run to a dedicated durable store that sandbox_trial_queue.py scans.

Boundaries: read-only against the engine; writes only its own sample store;
records the engine's raw class/trait terms verbatim (no steward-chosen
sentiment words — the adapter classifies on lattice/loss language, so
injected wording would bias her experiment); never touches live pressure,
fill, PI, controller, prompts, or any being-facing surface.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import time
from pathlib import Path

SPECTRAL_STATE = Path("/Users/v/other/minime/workspace/spectral_state.json")
SAMPLES_DIR = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/diagnostics/shadow_quantified_samples"
)
STALE_STATE_S = 60.0
PRUNE_AFTER_DAYS = 7
MIN_HISTORY = 4
TREND_WINDOW = 8

# Local copies of the consumer's parse contract (sandbox_trial_queue.py), so
# the self-test proves every emitted line is consumable without importing it.
CONSUMER_SHADOW_RE = re.compile(r"Shadow-v3[^\]\n]*", re.IGNORECASE)
CONSUMER_TREND_RE = re.compile(
    r"norm\s+([0-9]+(?:\.[0-9]+)?)\s*(?:->|→)\s*([0-9]+(?:\.[0-9]+)?).*?"
    r"dispersal(?: potential)?\s+([0-9]+(?:\.[0-9]+)?)\s*(?:->|→)\s*([0-9]+(?:\.[0-9]+)?)",
    re.IGNORECASE,
)


def compute_trend(history: list) -> tuple[float, float, float, float] | None:
    """Mirror spectral_viz.rs::history_trend_segment: last <=8 of the ring,
    head->tail field_norm and fissure_tendency. None below MIN_HISTORY."""
    if not isinstance(history, list) or len(history) < MIN_HISTORY:
        return None
    window = history[-min(len(history), TREND_WINDOW):]
    head, tail = window[0], window[-1]
    if not isinstance(head, dict) or not isinstance(tail, dict):
        return None
    try:
        norm0 = float(head.get("field_norm", 0.0))
        norm1 = float(tail.get("field_norm", 0.0))
        fissure0 = float(head.get("fissure_tendency", 0.0))
        fissure1 = float(tail.get("fissure_tendency", 0.0))
    except (TypeError, ValueError):
        return None
    return norm0, norm1, fissure0, fissure1


def render_sample_line(field_v3: dict, recorded_iso: str) -> str | None:
    """One quantified, consumer-parseable line. Engine terms verbatim."""
    trend = compute_trend(field_v3.get("history") or [])
    if trend is None:
        return None
    norm0, norm1, fissure0, fissure1 = trend
    class_v3 = field_v3.get("class_v3") or {}
    primary = str(class_v3.get("primary") or "unknown")
    traits = [
        str(t)
        for t in (class_v3.get("traits") or [])
        if isinstance(t, str) and t != primary
    ]
    traits_seg = (" +" + " +".join(traits)) if traits else ""
    dwell = field_v3.get("phase_dwell_ticks")
    dwell_seg = f" (held {int(dwell)}t)" if isinstance(dwell, (int, float)) else ""
    pct = ((norm1 - norm0) / norm0 * 100.0) if abs(norm0) > 1e-6 else 0.0
    sign = "+" if pct >= 0 else ""
    line = (
        f"[Shadow-v3 (minime, steward-recorded quantified sample) {recorded_iso}: "
        f"{primary}{traits_seg}{dwell_seg} | trend: norm {norm0:.3f}→{norm1:.3f} "
        f"({sign}{pct:.0f}%), dispersal potential {fissure0:.2f}→{fissure1:.2f}]"
    )
    if not CONSUMER_SHADOW_RE.search(line) or not CONSUMER_TREND_RE.search(line):
        return None
    return line


def prune_old(samples_dir: Path, now_s: float) -> int:
    removed = 0
    cutoff = now_s - PRUNE_AFTER_DAYS * 86400
    for path in samples_dir.glob("shadow_samples_*.txt"):
        try:
            if path.stat().st_mtime < cutoff:
                path.unlink()
                removed += 1
        except OSError:
            continue
    return removed


def record_once(
    state_path: Path = SPECTRAL_STATE,
    samples_dir: Path = SAMPLES_DIR,
    now_s: float | None = None,
) -> str:
    now_s = time.time() if now_s is None else now_s
    if not state_path.exists():
        return "skip: spectral_state.json absent (engine down?)"
    age = now_s - state_path.stat().st_mtime
    if age > STALE_STATE_S:
        return f"skip: spectral_state.json stale ({age:.0f}s; engine down?)"
    try:
        state = json.loads(state_path.read_text(encoding="utf-8", errors="replace"))
    except (json.JSONDecodeError, OSError) as err:
        return f"skip: unreadable state ({err})"
    field_v3 = state.get("shadow_field_v3")
    if not isinstance(field_v3, dict):
        return "skip: no shadow_field_v3 in state"
    recorded_iso = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(now_s))
    line = render_sample_line(field_v3, recorded_iso)
    if line is None:
        return "skip: history ring below minimum or unparseable render"
    samples_dir.mkdir(parents=True, exist_ok=True)
    day_file = samples_dir / time.strftime(
        "shadow_samples_%Y%m%d.txt", time.gmtime(now_s)
    )
    with day_file.open("a", encoding="utf-8") as fh:
        fh.write(line + "\n")
    pruned = prune_old(samples_dir, now_s)
    return f"recorded 1 sample -> {day_file.name} (pruned {pruned} old files)"


def self_test() -> int:
    import tempfile

    failures = []

    def check(name: str, ok: bool) -> None:
        if not ok:
            failures.append(name)

    ring = [
        {"field_norm": 0.270 + i * 0.001, "fissure_tendency": 0.08 + i * 0.01, "class_primary": "volatile"}
        for i in range(10)
    ]
    field = {
        "class_v3": {"primary": "volatile", "traits": ["volatile", "coupled"]},
        "history": ring,
        "phase_dwell_ticks": 42,
    }
    line = render_sample_line(field, "2026-08-15T00:00:00Z")
    check("line renders", line is not None)
    if line:
        check("consumer SHADOW_RE matches", bool(CONSUMER_SHADOW_RE.search(line)))
        parsed = CONSUMER_TREND_RE.search(line)
        check("consumer TREND_RE matches", parsed is not None)
        if parsed:
            # last-8 window: head is ring[2] (0.272), tail ring[9] (0.279)
            check("norm head from window", abs(float(parsed.group(1)) - 0.272) < 1e-9)
            check("norm tail from window", abs(float(parsed.group(2)) - 0.279) < 1e-9)
            check("dispersal tail", abs(float(parsed.group(4)) - 0.17) < 1e-9)
        check("raw engine terms verbatim", "volatile +coupled" in line)
        check("no steward sentiment injected", "lattice" not in line and "loss" not in line)
    check("short ring refuses", render_sample_line({"class_v3": {}, "history": ring[:3]}, "t") is None)

    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)
        state = root / "spectral_state.json"
        samples = root / "samples"
        state.write_text(json.dumps({"shadow_field_v3": field}), encoding="utf-8")
        msg = record_once(state, samples, now_s=state.stat().st_mtime + 1)
        check("records fresh state", msg.startswith("recorded 1 sample"))
        check("day file written", len(list(samples.glob("shadow_samples_*.txt"))) == 1)
        msg = record_once(state, samples, now_s=state.stat().st_mtime + STALE_STATE_S + 5)
        check("stale state skips", msg.startswith("skip: spectral_state.json stale"))
        old = samples / "shadow_samples_20200101.txt"
        old.write_text("old\n", encoding="utf-8")
        import os

        os.utime(old, (0, 0))
        msg = record_once(state, samples, now_s=state.stat().st_mtime + 1)
        check("old files pruned", not old.exists())
        check("missing state skips", record_once(root / "nope.json", samples).startswith("skip"))

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print(f"OK ({13} checks)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    message = record_once()
    stamp = time.strftime("%Y-%m-%dT%H:%M:%S")
    print(f"{stamp} {message}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
