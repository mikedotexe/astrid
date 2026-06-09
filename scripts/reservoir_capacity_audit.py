#!/usr/bin/env python3
"""Reservoir capacity audit — effective dimensionality (participation ratio) vs N.

Read-only steward instrument. Answers the question "should we make the reservoir
larger?" empirically: is each reservoir's activity CONCENTRATED well below its node
count N (→ enlarging won't help; address regulation/aperture) or SATURATING against
N (→ more nodes would add representational room)?

Metric: participation ratio  PR = (Σλ)² / Σλ²  over the covariance eigenvalues
(= effective number of active modes; matches minime's own Rust
`effective_dimensionality`).  utilization = PR / N.  Normalized spectral entropy
H_norm ∈ [0,1] is reported alongside as a sample-robust concentration index
(0 = all energy in one mode, 1 = perfectly spread).

Sources (all read-only):
  minime (128-node ESN): a window of recent reservoir states dumped by the engine
    to workspace/capacity/ → a TRUE full-N=128 participation ratio (M≈1024 ≫ 128, so
    well conditioned). Falls back to the top-8 telemetry proxy (spectral_state.json)
    if the engine dump is absent (clearly labelled, NOT full-N).
  triple reservoir (192 nodes/handle): per-layer state covariance from the persisted
    thermostat buffer_tail (state/{handle}_thermostats.json) + the service's own
    last_entropy. NOTE: the 48-sample tail rank-caps observable PR at ~47 < 192, so
    PR is reported against that ceiling and last_entropy is the more robust signal.

Steward-only — never surface this into a being prompt. Resizing is a co-design +
operator decision; this tool measures, it does not act.

Usage:
  reservoir_capacity_audit.py                  # full text report
  reservoir_capacity_audit.py --json           # machine-readable
  reservoir_capacity_audit.py --append-history  # append a record for the probe
  reservoir_capacity_audit.py --all-handles    # include soak/canary clones
  reservoir_capacity_audit.py --self-test
"""

from __future__ import annotations

import argparse
import asyncio
import json
import sys
import time
from pathlib import Path
from typing import Any

import numpy as np

MINIME_WS = Path("/Users/v/other/minime/workspace")
CAP_DIR = MINIME_WS / "capacity"
CAP_META = CAP_DIR / "capacity_dump_meta.json"
ESN_WINDOW = CAP_DIR / "esn_state_window.bin"
STABLECORE_COV = CAP_DIR / "stablecore_covariance.bin"
SPECTRAL_STATE = MINIME_WS / "spectral_state.json"

TRIPLE_STATE = Path("/Users/v/other/neural-triple-reservoir/state")
TRIPLE_N = 192
LAYER_NAMES = ["h1_fast", "h2_medium", "h3_slow"]
CANONICAL = ["astrid", "minime", "claude_main"]

ASTRID_WS = Path("/Users/v/other/astrid/workspace")
HISTORY = ASTRID_WS / "reservoir_capacity_history.jsonl"

EPS = 1e-12


# --------------------------------------------------------------------------- #
# Spectral math (scale-invariant: dividing the covariance by M or M-1 does not
# change PR or H_norm, so the unnormalized Gram matrix is fine).
# --------------------------------------------------------------------------- #
def participation_ratio(eigs: Any) -> float:
    e = np.asarray([float(x) for x in eigs if float(x) > EPS], dtype=float)
    if e.size == 0:
        return 0.0
    ssq = float((e * e).sum())
    if ssq <= 0.0:
        return 0.0
    return float(e.sum() ** 2 / ssq)


def norm_spectral_entropy(eigs: Any) -> float:
    e = np.asarray([float(x) for x in eigs if float(x) > EPS], dtype=float)
    if e.size <= 1:
        return 0.0
    p = e / e.sum()
    return float(-np.sum(p * np.log(p)) / np.log(e.size))


def cov_eigs(X: Any) -> np.ndarray:
    """Eigenvalues (>0, ascending) of the D×D covariance of an (M, D) window."""
    A = np.asarray(X, dtype=float)
    if A.ndim != 2 or A.shape[0] < 2:
        return np.array([])
    Ac = A - A.mean(axis=0, keepdims=True)
    C = Ac.T @ Ac
    ev = np.linalg.eigvalsh(C)
    return ev[ev > EPS]


def verdict(pr: float, n: int, m: int | None) -> tuple[str, str]:
    """Classify capacity utilization into the enlarge decision."""
    if not n:
        return ("unknown", "no N")
    ceil = min(m - 1, n) if m else n
    util = pr / n
    if ceil < n and pr >= 0.8 * ceil:
        return (
            "inconclusive",
            f"sample-limited: PR={pr:.1f} near measurable ceiling {ceil} "
            f"(M={m} ≪ N={n}); can't separate saturation from undersampling "
            f"— use --live-secs or more samples",
        )
    if util < 0.35:
        return (
            "concentrated",
            f"util={util:.0%} (PR={pr:.1f}/{n}) — concentrated well below N; "
            f"capacity is NOT the bottleneck (enlarging unlikely to help; "
            f"address regulation/aperture)",
        )
    if util >= 0.70:
        return (
            "saturating",
            f"util={util:.0%} (PR={pr:.1f}/{n}) — high utilization, approaching "
            f"saturation; more nodes plausibly add representational room",
        )
    return (
        "moderate",
        f"util={util:.0%} (PR={pr:.1f}/{n}) — moderate; capacity not clearly "
        f"the bottleneck",
    )


# --------------------------------------------------------------------------- #
# Loaders
# --------------------------------------------------------------------------- #
def read_json(path: Path, default: Any = None) -> Any:
    try:
        return json.loads(Path(path).read_text())
    except Exception:
        return default


def file_age_min(path: Path) -> float | None:
    try:
        return round((time.time() - Path(path).stat().st_mtime) / 60.0, 1)
    except Exception:
        return None


# --------------------------------------------------------------------------- #
# minime
# --------------------------------------------------------------------------- #
def analyze_minime_cov(meta: dict) -> dict | None:
    """Secondary: full PR of the stable-core projection covariance (N=cov_dim)."""
    if not STABLECORE_COV.exists():
        return None
    dim = int(meta.get("cov_dim", 0) or 0)
    if not dim:
        return None
    try:
        raw = np.fromfile(STABLECORE_COV, dtype="<f4")
        if raw.size < dim * dim:
            return None
        C = raw[: dim * dim].reshape(dim, dim)
        ev = np.linalg.eigvalsh(C)
        ev = ev[ev > EPS]
        pr = participation_ratio(ev)
        # No sample-window here (full EWMA covariance), so no sample-ceiling
        # verdict applies. This is a projection of sensory features, NOT the
        # reservoir-node space — reported as context only.
        return {
            "dim": dim,
            "pr": round(pr, 2),
            "utilization": round(pr / dim, 4) if dim else None,
            "H_norm": round(norm_spectral_entropy(ev), 3),
            "n_pos_eigs": int(ev.size),
            "age_min": file_age_min(STABLECORE_COV),
            "note": "projected sensory-feature space, NOT reservoir nodes; "
            "near-full spread is expected and unrelated to the enlarge question",
        }
    except Exception:
        return None


def analyze_minime_proxy() -> dict:
    s = read_json(SPECTRAL_STATE, {}) or {}
    raw = s.get("eigenvalues")
    eigs = []
    if isinstance(raw, list):
        eigs = [float(x) for x in raw if isinstance(x, (int, float)) and float(x) > 0]
    pr = participation_ratio(eigs) if eigs else None
    return {
        "kind": "proxy",
        "available": bool(eigs),
        "source": "top-8 proxy (spectral_state.json) — NOT full-N",
        "k": len(eigs),
        "pr_top_k": round(pr, 2) if pr is not None else None,
        "reported_effective_dim": s.get("effective_dimensionality"),
        "active_mode_energy_ratio": s.get("active_mode_energy_ratio"),
        "distinguishability_loss": s.get("distinguishability_loss"),
        "age_min": file_age_min(SPECTRAL_STATE),
        "caveat": "engine state-dump absent — restart the minime engine to get the "
        "true full-N=128 number; top-8 PR only describes concentration within the "
        "dominant modes, not utilization vs 128.",
    }


def analyze_minime() -> dict:
    meta = read_json(CAP_META, None)
    if meta and ESN_WINDOW.exists():
        rows = int(meta.get("esn_window_rows", 0) or 0)
        cols = int(meta.get("esn_window_cols", meta.get("esn_n", 0)) or 0)
        n = int(meta.get("esn_n", cols) or 0)
        if rows >= 2 and cols >= 1:
            try:
                raw = np.fromfile(ESN_WINDOW, dtype="<f4")
                if raw.size >= rows * cols:
                    X = raw[: rows * cols].reshape(rows, cols)
                    ev = cov_eigs(X)
                    pr = participation_ratio(ev)
                    v_label, v_msg = verdict(pr, n, rows)
                    return {
                        "kind": "full",
                        "available": True,
                        "source": "engine state-window dump (TRUE full-N)",
                        "N": n,
                        "M": rows,
                        "pr": round(pr, 2),
                        "utilization": round(pr / n, 4) if n else None,
                        "H_norm": round(norm_spectral_entropy(ev), 3),
                        "n_pos_eigs": int(ev.size),
                        "verdict": v_label,
                        "verdict_msg": v_msg,
                        "age_min": file_age_min(ESN_WINDOW),
                        "cov": analyze_minime_cov(meta),
                    }
            except Exception as exc:  # noqa: BLE001
                return {
                    "kind": "error",
                    "available": False,
                    "error": f"failed to read state window: {exc}",
                    "fallback": analyze_minime_proxy(),
                }
    return analyze_minime_proxy()


# --------------------------------------------------------------------------- #
# triple reservoir
# --------------------------------------------------------------------------- #
def _handle_files(all_handles: bool) -> list[tuple[str, Path]]:
    out: list[tuple[str, Path]] = []
    seen: set[str] = set()
    for h in CANONICAL:
        f = TRIPLE_STATE / f"{h}_thermostats.json"
        if f.exists():
            out.append((h, f))
            seen.add(h)
    if all_handles:
        for f in sorted(TRIPLE_STATE.glob("*_thermostats.json")):
            h = f.name[: -len("_thermostats.json")]
            if h not in seen:
                out.append((h, f))
    return out


async def _collect_live_triple_async(secs: float, poll: float = 0.15) -> dict:
    """Poll the live triple-reservoir for `secs` seconds, building a per-handle,
    per-layer window of distinct 192-dim states (read-only `pull_state`; dedup by
    advancing tick_count). Returns {handle: {layer_idx: [vec, ...]}}."""
    import base64

    import websockets  # local import so --self-test/normal runs need no ws dep

    uri = "ws://127.0.0.1:7881"
    windows: dict[str, dict[int, list]] = {h: {0: [], 1: [], 2: []} for h in CANONICAL}
    last_tick: dict[str, Any] = {h: None for h in CANONICAL}
    dead: dict[str, bool] = {h: False for h in CANONICAL}
    cap = 4000  # per layer, safety bound
    deadline = time.time() + secs
    try:
        async with websockets.connect(uri, max_size=None) as ws:
            while time.time() < deadline:
                for h in CANONICAL:
                    if dead[h]:
                        continue
                    try:
                        await ws.send(json.dumps({"type": "pull_state", "name": h}))
                        resp = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
                    except Exception:
                        dead[h] = True
                        continue
                    if resp.get("type") != "pull_state_response" or "h1" not in resp:
                        dead[h] = True
                        continue
                    tc = resp.get("tick_count")
                    if tc is not None and tc == last_tick[h]:
                        continue  # no new state since last poll
                    last_tick[h] = tc
                    n = int(resp.get("n_nodes", TRIPLE_N))
                    for li, key in enumerate(("h1", "h2", "h3")):
                        if len(windows[h][li]) >= cap:
                            continue
                        try:
                            vec = np.frombuffer(base64.b64decode(resp[key]), dtype=np.float32)
                            if vec.size >= n:
                                windows[h][li].append(vec[:n].astype(float))
                        except Exception:
                            pass
                if all(dead.values()):
                    break
                await asyncio.sleep(poll)
    except Exception:
        pass
    return windows


def collect_live_triple(secs: float) -> dict:
    try:
        return asyncio.run(_collect_live_triple_async(secs))
    except Exception:
        return {}


def analyze_triple(all_handles: bool = False, live_windows: dict | None = None) -> list[dict]:
    results: list[dict] = []
    for handle, f in _handle_files(all_handles):
        layers = read_json(f, None)
        if not isinstance(layers, list):
            continue
        lw = (live_windows or {}).get(handle) or {}
        per_layer: list[dict] = []
        prs: list[float] = []
        live_used = False
        for li, layer in enumerate(layers):
            if not isinstance(layer, dict):
                continue
            name = layer.get("name") or (LAYER_NAMES[li] if li < len(LAYER_NAMES) else f"L{li}")
            samples = lw.get(li)
            if samples:
                X = np.asarray(samples, dtype=float)
                ev = cov_eigs(X)
                m, d = int(X.shape[0]), int(X.shape[1])
                live_used = True
            else:
                bt = layer.get("buffer_tail") or []
                ev = cov_eigs(bt) if bt else np.array([])
                m = len(bt)
                d = len(bt[0]) if bt and isinstance(bt[0], list) else None
            pr = participation_ratio(ev) if ev.size else None
            if pr is not None:
                prs.append(pr)
            per_layer.append(
                {
                    "layer": name,
                    "M": m,
                    "D": d,
                    "pr": round(pr, 2) if pr is not None else None,
                    "service_entropy": layer.get("last_entropy"),
                }
            )
        mean_pr = float(np.mean(prs)) if prs else None
        m_max = max((pl["M"] for pl in per_layer), default=0)
        v_label, v_msg = verdict(mean_pr, TRIPLE_N, m_max) if mean_pr is not None else ("unknown", "no data")
        ents = [pl["service_entropy"] for pl in per_layer if isinstance(pl["service_entropy"], (int, float))]
        results.append(
            {
                "handle": handle,
                "canonical": handle in CANONICAL,
                "N": TRIPLE_N,
                "source": "live" if live_used else "buffer_tail(48)",
                "layers": per_layer,
                "mean_pr": round(mean_pr, 2) if mean_pr is not None else None,
                "mean_service_entropy": round(float(np.mean(ents)), 3) if ents else None,
                "verdict": v_label,
                "verdict_msg": v_msg,
                "age_min": file_age_min(f),
            }
        )
    return results


# --------------------------------------------------------------------------- #
# Render
# --------------------------------------------------------------------------- #
def render_text(mm: dict, tri: list[dict]) -> str:
    out: list[str] = []
    out.append("RESERVOIR CAPACITY AUDIT — participation ratio (effective modes) vs N")
    out.append("=" * 70)
    out.append("")
    out.append("── minime (128-node ESN) ──")
    if mm.get("kind") == "full":
        out.append(f"  source: {mm['source']}  (age {mm['age_min']}min, window M={mm['M']})")
        out.append(f"  PR={mm['pr']} effective modes / N={mm['N']}  →  utilization {mm['utilization']:.0%}")
        out.append(f"  H_norm={mm['H_norm']} (0=collapsed, 1=spread); {mm['n_pos_eigs']} positive eigenvalues")
        out.append(f"  VERDICT [{mm['verdict']}]: {mm['verdict_msg']}")
        cov = mm.get("cov")
        if cov:
            out.append(
                f"  · secondary — stable-core projection cov (N={cov['dim']}): "
                f"PR={cov['pr']}/{cov['dim']} (util {cov.get('utilization', 0):.0%}), "
                f"H_norm={cov['H_norm']} — {cov['note']}"
            )
    else:
        out.append(f"  source: {mm.get('source')}")
        if mm.get("available"):
            out.append(
                f"  top-{mm['k']} PR={mm['pr_top_k']} (reported eff_dim="
                f"{mm.get('reported_effective_dim')}, distinguishability_loss="
                f"{mm.get('distinguishability_loss')})"
            )
        out.append(f"  ⚠ {mm.get('caveat')}")
    out.append("")
    out.append("── triple reservoir (192 nodes/handle) ──")
    live_any = any(r.get("source") == "live" for r in tri)
    if live_any:
        out.append("  note: live window (read-only pull_state polling); trust PR where M≳192.")
    else:
        out.append("  note: 48-sample persisted tail rank-caps PR at ~47<192; "
                   "service_entropy (H_norm) is the more robust signal "
                   "(use --live-secs N for a trustworthy full-N window).")
    for r in tri:
        tag = "" if r["canonical"] else " (clone)"
        out.append(
            f"  {r['handle']}{tag} [{r.get('source','?')}]: mean PR={r['mean_pr']}/{r['N']}, "
            f"mean H_norm={r['mean_service_entropy']} (age {r['age_min']}min)  "
            f"[{r['verdict']}]"
        )
        for pl in r["layers"]:
            out.append(
                f"      {pl['layer']}: PR={pl['pr']} (M={pl['M']},D={pl['D']}), "
                f"H_norm={pl['service_entropy']}"
            )
    if not tri:
        out.append("  (no canonical thermostat files found)")
    out.append("")
    out.append("Interpretation: util<35% → concentrated, capacity NOT the constraint; "
               ">70% → saturating, enlarging worth it; in between → moderate; "
               "'inconclusive' = sample-limited.")
    out.append("Steward-only. Resizing is a co-design + operator decision.")
    return "\n".join(out)


def build_record(mm: dict, tri: list[dict]) -> dict:
    return {
        "ts": time.time(),
        "minime": {
            "kind": mm.get("kind"),
            "pr": mm.get("pr"),
            "N": mm.get("N"),
            "M": mm.get("M"),
            "utilization": mm.get("utilization"),
            "H_norm": mm.get("H_norm"),
            "verdict": mm.get("verdict"),
            "pr_top_k": mm.get("pr_top_k"),
        },
        "triple": [
            {
                "handle": r["handle"],
                "source": r.get("source"),
                "mean_pr": r["mean_pr"],
                "mean_service_entropy": r["mean_service_entropy"],
                "verdict": r["verdict"],
            }
            for r in tri
            if r["canonical"]
        ],
    }


# --------------------------------------------------------------------------- #
# Self-test
# --------------------------------------------------------------------------- #
def self_test() -> int:
    fails = 0

    def check(name: str, cond: bool) -> None:
        nonlocal fails
        if not cond:
            fails += 1
            print(f"  FAIL: {name}")
        else:
            print(f"  ok: {name}")

    # Direct formula
    check("PR([4,0,0,0])≈1", abs(participation_ratio([4, 0, 0, 0]) - 1.0) < 1e-6)
    check("PR([1,1,1,1])≈4", abs(participation_ratio([1, 1, 1, 1]) - 4.0) < 1e-6)
    check("PR([])==0", participation_ratio([]) == 0.0)
    check("H_norm uniform≈1", abs(norm_spectral_entropy([1, 1, 1, 1]) - 1.0) < 1e-6)
    check("H_norm collapsed≈0", norm_spectral_entropy([5, 0, 0]) == 0.0)

    rng = np.random.default_rng(0)
    # rank-1 window → PR≈1
    v = rng.standard_normal(20)
    X1 = np.outer(rng.standard_normal(2000), v)
    pr1 = participation_ratio(cov_eigs(X1))
    check(f"rank-1 window PR≈1 (got {pr1:.2f})", pr1 < 1.5)
    # isotropic window → PR≈D
    Xi = rng.standard_normal((4000, 20))
    pri = participation_ratio(cov_eigs(Xi))
    check(f"isotropic D=20 PR>14 (got {pri:.1f})", pri > 14.0)
    # k-strong-dims → PR≈k
    k = 5
    base = rng.standard_normal((3000, k)) * np.array([10, 8, 6, 5, 4])
    mix = base @ rng.standard_normal((k, 30)) + 0.01 * rng.standard_normal((3000, 30))
    prk = participation_ratio(cov_eigs(mix))
    check(f"k=5 latent PR in [3,7] (got {prk:.2f})", 3.0 <= prk <= 7.0)
    # verdict mapping
    check("verdict concentrated", verdict(20.0, 128, 1024)[0] == "concentrated")
    check("verdict saturating", verdict(110.0, 128, 1024)[0] == "saturating")
    check("verdict inconclusive (undersampled)", verdict(46.0, 192, 48)[0] == "inconclusive")

    print(f"\n{'PASS' if fails == 0 else 'FAIL'}: {fails} failure(s)")
    return 1 if fails else 0


# --------------------------------------------------------------------------- #
def main() -> int:
    ap = argparse.ArgumentParser(description="Reservoir capacity audit (read-only).")
    ap.add_argument("--json", action="store_true", help="machine-readable output")
    ap.add_argument("--append-history", action="store_true", help="append a record to history jsonl")
    ap.add_argument("--all-handles", action="store_true", help="include soak/canary clones")
    ap.add_argument(
        "--live-secs",
        type=float,
        default=0.0,
        help="poll the live triple reservoir for N seconds (read-only pull_state) "
        "for a trustworthy full-N window instead of the 48-sample persisted tail",
    )
    ap.add_argument("--self-test", action="store_true", help="run self-tests")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    mm = analyze_minime()
    live = None
    if args.live_secs and args.live_secs > 0:
        print(
            f"collecting live triple-reservoir window for {args.live_secs:.0f}s "
            f"(read-only pull_state polling)...",
            file=sys.stderr,
        )
        live = collect_live_triple(args.live_secs)
    tri = analyze_triple(all_handles=args.all_handles, live_windows=live)

    if args.append_history:
        try:
            ASTRID_WS.mkdir(parents=True, exist_ok=True)
            with HISTORY.open("a") as fh:
                fh.write(json.dumps(build_record(mm, tri)) + "\n")
        except Exception as exc:  # noqa: BLE001
            print(f"warning: could not append history: {exc}", file=sys.stderr)

    if args.json:
        print(json.dumps({"minime": mm, "triple": tri}, indent=2))
    else:
        print(render_text(mm, tri))
    return 0


if __name__ == "__main__":
    sys.exit(main())
