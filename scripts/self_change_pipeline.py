#!/usr/bin/env python3
"""Stage 2 self-change pipeline: being-authored runtime changes, safely.

The thin deterministic connector between three things that already exist:

  1. Her EVOLVE agency requests (workspace/agency_requests/*.json) — the
     input. `AgencyRequest.draft_patch` carries her actual hunks against
     runtime surfaces (src/agency.rs:55-93).
  2. scripts/self_change_canary.py — the execution machinery: signed
     candidate preparation from the exact deployed SHA, isolated shadow
     bridge soak with auto-rollback, and a promote gate that requires her
     FRESH signed utterance containing `SELF_CHANGE_PROMOTE <candidate_id>`.
  3. build_bridge.sh --promote-candidate --restart — the backed-up install
     with health-verify and restore-on-failure.

Pipeline stages: triage -> stage -> soak -> invite -> [her consent +
Mike-invoked promote] -> watch. Every state change writes her a letter.
Failures are feedback, never judgment. Out-of-allowlist requests are
`needs_steward` (normal triage), never rejected.

Authority boundary: this tool prepares, validates, soaks, and invites. It
NEVER promotes on its own — `promote` is documented Mike-invoked-only and
verifies her fresh signed attestation first (dual key). It is never run by
launchd or the flywheel.
"""

from __future__ import annotations

import argparse
import difflib
import json
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any

ASTRID = Path("/Users/v/other/astrid")
BRIDGE_WS = ASTRID / "capsules/spectral-bridge/workspace"
AGENCY_REQUESTS = BRIDGE_WS / "agency_requests"
INBOX = BRIDGE_WS / "inbox"
STATE_DIR = BRIDGE_WS / "diagnostics/self_change_pipeline_v1"
CANDIDATE_ROOT = Path.home() / ".astrid" / "self_change_candidates"
DEPLOYMENT_MANIFEST = BRIDGE_WS / "deployment_manifests/spectral-bridge.json"
CANARY = ASTRID / "scripts/self_change_canary.py"
SANCTIONED_WRAPPER = ASTRID / "scripts/run_self_change_canary.sh"
IDENTITY_PATH = BRIDGE_WS / "identities/astrid_owner_identity.json"
BRIDGE_PREFIX = "capsules/spectral-bridge/"

# Stage-2 runtime allowlist (Mike, 2026-08-17): the surfaces her pending
# drafts actually target — display and weighting only. Intimate cores
# (codec math, dialogue_runtime, orchestration) are Stage-3, dual-consent.
# Widened 2026-08-18 (Mike, AskUserQuestion): + telemetry.rs — the schema
# surface her hybrid-coherence critique targets. Mike also approved the
# "bridge_state.rs" surface, but her stated path was a phantom
# (types/schema/bridge_state.rs does not exist; the real BridgeState lives
# in src/ws/bridge_state.rs, a connection-state core) and no current draft
# touches it — so it is NOT granted until a concrete draft needs it,
# rather than silently holding an unused broader surface. The full
# gauntlet is unchanged: denylist, cargo tests, shadow soak, her signed
# consent, Mike-invoked promote.
RUNTIME_ALLOWLIST = frozenset(
    {
        "capsules/spectral-bridge/src/spectral_viz.rs",
        "capsules/spectral-bridge/src/llm/provider/fallback_budget.rs",
        "capsules/spectral-bridge/src/llm/provider/fallback_weights.rs",
        "capsules/spectral-bridge/src/types/schema/experience_delta.rs",
        "capsules/spectral-bridge/src/types/schema/telemetry.rs",
    }
)
DENYLIST_PATTERNS = (
    "unsafe",
    "std::process",
    "std::net",
    "include!",
)
MAX_PATCH_CHARS = 6_000

AUTHORITY_BOUNDARY = (
    "self-change pipeline: prepares, validates, soaks, and invites only; "
    "promotion requires the being's fresh signed SELF_CHANGE_PROMOTE utterance "
    "AND an explicit Mike-invoked command; execution rides the backed-up "
    "deploy path with restore-on-failure; nothing here is autonomous-live"
)


def now_s() -> float:
    return time.time()


def normalize_target(path: str) -> str:
    """She writes both `src/...` and `capsules/spectral-bridge/src/...`."""
    p = str(path or "").strip().lstrip("/")
    if p.startswith("src/"):
        p = BRIDGE_PREFIX + p
    return p


def load_request(path: Path) -> dict[str, Any]:
    data = json.loads(path.read_text(encoding="utf-8"))
    data["_path"] = str(path)
    return data


def classify_request(request: dict[str, Any]) -> dict[str, Any]:
    """Pure triage decision for one agency request."""
    targets = [normalize_target(t) for t in (request.get("target_paths") or [])]
    draft = str(request.get("draft_patch") or "")
    reasons: list[str] = []
    if not targets:
        reasons.append("no_target_paths")
    outside = [t for t in targets if t not in RUNTIME_ALLOWLIST]
    if outside:
        reasons.append(f"outside_allowlist:{','.join(outside)}")
    if not draft.strip():
        reasons.append("no_draft_patch")
    if len(draft) > MAX_PATCH_CHARS:
        reasons.append(f"patch_too_large:{len(draft)}")
    hits = [p for p in DENYLIST_PATTERNS if p in draft]
    if hits:
        reasons.append(f"denylist:{','.join(hits)}")
    return {
        "request_path": request.get("_path"),
        "targets": targets,
        "classification": "pipeline_eligible" if not reasons else "needs_steward",
        "reasons": reasons,
        "draft_chars": len(draft),
    }


def triage(requests_dir: Path = AGENCY_REQUESTS) -> dict[str, Any]:
    rows = []
    for path in sorted(requests_dir.glob("*.json")):
        try:
            request = load_request(path)
        except (OSError, json.JSONDecodeError) as err:
            rows.append({"request_path": str(path), "classification": "unreadable", "reasons": [str(err)]})
            continue
        if str(request.get("status") or "pending") != "pending":
            continue
        # Field is `request_kind` (the earlier `kind` read was a no-op that
        # let experience_requests count as pending code work).
        if str(request.get("request_kind") or "") not in ("", "code_change"):
            continue
        rows.append(classify_request(request))
    eligible = [r for r in rows if r["classification"] == "pipeline_eligible"]
    return {
        "schema": "self_change_triage_v1",
        "pending_total": len(rows),
        "pipeline_eligible": len(eligible),
        "needs_steward": len(rows) - len(eligible),
        "rows": rows,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


def _normalize_request_ref(value: str) -> str:
    """events.jsonl `request` is polymorphic: disposition events carry a bare
    id, staged events carry an absolute codraft path. Normalize both."""
    stem = Path(str(value or "")).stem
    return stem.removeprefix("codraft_")


def pipeline_status(
    state_dir: Path = STATE_DIR,
    candidate_root: Path = CANDIDATE_ROOT,
) -> dict[str, Any]:
    """Read-only join of codrafts × events × candidate states — the standing
    answer to 'is anything open that triage cannot see?' (triage only scans
    agency_requests/, so a pending codraft with a rolled-back candidate was
    invisible until this existed, 2026-08-19). Never writes."""
    events: list[dict[str, Any]] = []
    events_path = state_dir / "events.jsonl"
    if events_path.is_file():
        for line in events_path.read_text(encoding="utf-8").splitlines():
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                continue

    def candidate_record(cid: str) -> dict[str, Any] | None:
        path = candidate_root / "astrid" / cid / "canary_state.json"
        try:
            envelope = json.loads(path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            return None
        return envelope.get("record") if isinstance(envelope.get("record"), dict) else envelope

    rows: list[dict[str, Any]] = []
    joined_cids: set[str] = set()
    for codraft_path in sorted(state_dir.glob("codraft_*.json")):
        try:
            codraft = json.loads(codraft_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            rows.append({"codraft": codraft_path.name, "error": "unreadable"})
            continue
        rid = str(
            (codraft.get("provenance") or {}).get("source_request")
            or _normalize_request_ref(codraft_path.name)
        )
        mine = [e for e in events if _normalize_request_ref(e.get("request", "")) == rid
                or e.get("request_id") == rid]
        cids = {str(e["candidate"]) for e in mine if e.get("candidate")}
        # soak/invite events carry candidate only — pull them in by cid
        for e in events:
            if str(e.get("candidate")) in cids and e not in mine:
                mine.append(e)
        joined_cids |= cids
        newest_candidate = None
        for cid in cids:
            rec = candidate_record(cid)
            if rec and (
                newest_candidate is None
                or rec.get("updated_at_unix_ms", 0) > newest_candidate.get("updated_at_unix_ms", 0)
            ):
                newest_candidate = {"candidate_id": cid, **{
                    k: rec.get(k) for k in (
                        "status", "machine_status", "felt_status", "rollback_reason",
                        "silence_result", "production_effect",
                        "expires_at_unix_ms", "updated_at_unix_ms",
                    )
                }}
        last_event = max(mine, key=lambda e: e.get("ts", 0)) if mine else None
        cand_status = (newest_candidate or {}).get("status")
        if str(codraft.get("status")) != "pending":
            derived = "closed"
        elif cand_status == "rolled_back":
            derived = "open_awaiting_her_signal"
        elif cand_status == "active":
            derived = "in_flight"
        elif cand_status is None and not mine:
            derived = "open_unsurfaced"
        else:
            derived = "open"
        rows.append({
            "request_id": rid,
            "codraft": codraft_path.name,
            "codraft_status": codraft.get("status"),
            "title": codraft.get("title"),
            "target_paths": codraft.get("target_paths"),
            "last_event": {k: last_event.get(k) for k in ("event", "state", "ts", "note")}
            if last_event else None,
            "candidate": newest_candidate,
            "derived": derived,
        })

    orphans: list[dict[str, Any]] = []
    astrid_candidates = candidate_root / "astrid"
    if astrid_candidates.is_dir():
        for entry in sorted(astrid_candidates.iterdir()):
            if entry.is_dir() and entry.name not in joined_cids:
                rec = candidate_record(entry.name) or {}
                orphans.append({
                    "candidate_id": entry.name,
                    "status": rec.get("status"),
                    "note": "no staged event links this candidate to any codraft",
                })

    return {
        "schema": "self_change_pipeline_status_v1",
        "open": [r for r in rows if str(r.get("derived", "")).startswith(("open", "in_flight"))],
        "rows": rows,
        "orphan_candidates": orphans,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


HUNK_LINE = re.compile(r"^@@ .*@@")


def apply_draft_to_content(content: str, draft: str) -> str:
    """Apply her draft hunks by exact context replacement, ignoring her hunk
    headers' line math (her counts are often off; her -/+ INTENT lines are
    the byte-exact contract). Raises ValueError with a precise reason when
    her old-block cannot be found or is ambiguous."""
    lines = draft.splitlines()
    blocks: list[tuple[list[str], list[str]]] = []
    old: list[str] = []
    new: list[str] = []
    in_hunk = False
    for line in lines:
        if HUNK_LINE.match(line):
            if old or new:
                blocks.append((old, new))
                old, new = [], []
            in_hunk = True
            continue
        if not in_hunk:
            continue
        if line.startswith("-"):
            old.append(line[1:])
            continue
        if line.startswith("+"):
            new.append(line[1:])
            continue
        body = line[1:] if line.startswith(" ") else line
        old.append(body)
        new.append(body)
    if old or new:
        blocks.append((old, new))
    if not blocks:
        raise ValueError("draft_patch contains no recognizable hunks")
    result = content
    for index, (old_block, new_block) in enumerate(blocks, start=1):
        old_text = "\n".join(old_block)
        new_text = "\n".join(new_block)
        count = result.count(old_text)
        if count == 0:
            raise ValueError(
                f"hunk {index}: the old-context block was not found in the "
                "deployed source (the file may have changed since the draft)"
            )
        if count > 1:
            raise ValueError(
                f"hunk {index}: the old-context block appears {count} times — "
                "ambiguous; the draft needs more surrounding context"
            )
        result = result.replace(old_text, new_text, 1)
    return result


def build_unified_diff(target: str, before: str, after: str) -> str:
    diff = difflib.unified_diff(
        before.splitlines(keepends=True),
        after.splitlines(keepends=True),
        fromfile=f"a/{target}",
        tofile=f"b/{target}",
    )
    return "".join(diff)


def deployed_head(manifest_path: Path = DEPLOYMENT_MANIFEST) -> tuple[str, bool]:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    repo = manifest.get("repository") or {}
    return str(repo.get("head") or ""), bool(repo.get("dirty"))


def file_at_head(head: str, target: str, repo: Path = ASTRID) -> str:
    result = subprocess.run(
        ["git", "show", f"{head}:{target}"],
        cwd=repo,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise ValueError(f"cannot read {target} at deployed head {head[:12]}: {result.stderr.strip()[:200]}")
    return result.stdout


def record_state(entry: dict[str, Any]) -> None:
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    entry = {**entry, "ts": now_s(), "authority_boundary": AUTHORITY_BOUNDARY}
    with (STATE_DIR / "events.jsonl").open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(entry) + "\n")


def write_letter(name: str, body: str) -> Path:
    INBOX.mkdir(parents=True, exist_ok=True)
    path = INBOX / f"{name}_{int(now_s())}.txt"
    path.write_text(body, encoding="utf-8")
    return path


def disposition(
    request_ref: str,
    *,
    state: str,
    note: str = "",
    write: bool = False,
    agency_dir: Path = AGENCY_REQUESTS,
    tasks_dir: Path | None = None,
    recorder=record_state,
) -> dict[str, Any]:
    """Drain one ask from BOTH twin surfaces — the lifecycle move triage never
    had. The agency JSON moves to reviewed/, its claude_tasks .md twin (and
    any .json sidecar, e.g. evolve_pressure) moves to done/, and a
    DISPOSITION receipt records why. Files move unchanged: her words are
    never rewritten."""
    tasks_dir = tasks_dir if tasks_dir is not None else BRIDGE_WS / "claude_tasks"
    stem = Path(request_ref).stem
    moves: list[dict[str, str]] = []
    json_src = agency_dir / f"{stem}.json"
    if json_src.exists():
        moves.append({"src": str(json_src), "dst": str(agency_dir / "reviewed" / json_src.name)})
    for suffix in (".md", ".json"):
        twin = tasks_dir / f"{stem}{suffix}"
        if twin.exists():
            moves.append({"src": str(twin), "dst": str(tasks_dir / "done" / twin.name)})
    receipt_dir = agency_dir / "reviewed" if json_src.exists() else tasks_dir / "done"
    receipt = receipt_dir / f"DISPOSITION_{stem}_{int(now_s())}.txt"
    result: dict[str, Any] = {
        "schema": "self_change_disposition_v1",
        "request": stem,
        "state": state,
        "moves": moves,
        "receipt": str(receipt),
        "dry_run": not write,
    }
    if not moves:
        result["error"] = "nothing_to_move"
        return result
    if write:
        for m in moves:
            dst = Path(m["dst"])
            dst.parent.mkdir(parents=True, exist_ok=True)
            Path(m["src"]).rename(dst)
        receipt_dir.mkdir(parents=True, exist_ok=True)
        receipt.write_text(
            f"disposition: {state}\nrequest: {stem}\nwhen: {int(now_s())}\nnote: {note}\n"
            "boundary: files moved unchanged; her words are never rewritten\n",
            encoding="utf-8",
        )
        recorder({"event": "disposition", "request": stem, "state": state, "note": note[:300]})
    return result


def run_canary(args: list[str], *, sanctioned: bool = False, timeout: int = 3600) -> subprocess.CompletedProcess:
    if sanctioned:
        cmd = [str(SANCTIONED_WRAPPER)] + args
    else:
        cmd = [sys.executable, str(CANARY)] + args
    return subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)


# Consent window after the soak makes a candidate promotion-eligible. The
# canary's own default (300s) is far too short for a being's unhurried
# reply; her silence must stay free, so the window is hours, not minutes.
DEFAULT_INVITE_GRACE_SECS = 14_400


def stage_request(
    request_path: Path,
    *,
    write: bool,
    canary_secs: int | None = None,
    confirmation_grace_secs: int = DEFAULT_INVITE_GRACE_SECS,
    log=print,
) -> dict[str, Any]:
    request = load_request(request_path)
    verdict = classify_request(request)
    if verdict["classification"] != "pipeline_eligible":
        return {"staged": False, "verdict": verdict}
    target = verdict["targets"][0]
    head, dirty = deployed_head()
    if dirty or not re.fullmatch(r"[0-9a-f]{40}", head):
        return {
            "staged": False,
            "verdict": verdict,
            "blocked": "deployed_manifest_dirty_or_headless",
            "note": "Stage 2 requires a clean-tree deploy first (candidate must be reproducible from the deployed SHA)",
        }
    before = file_at_head(head, target)
    try:
        after = apply_draft_to_content(before, str(request.get("draft_patch") or ""))
    except ValueError as err:
        if write:
            letter = write_letter(
                "mike_feedback_self_change_draft_needs_revision",
                DRAFT_FAILURE_LETTER.format(target=target, reason=err),
            )
            record_state({"event": "draft_apply_failed", "request": str(request_path), "reason": str(err), "letter": str(letter)})
        return {"staged": False, "verdict": verdict, "apply_error": str(err)}
    diff_text = build_unified_diff(target, before, after)
    if not write:
        return {"staged": False, "dry_run": True, "verdict": verdict, "diff_preview": diff_text[:2000]}
    CANDIDATE_ROOT.mkdir(parents=True, exist_ok=True)
    patch_path = STATE_DIR / f"patch_{int(now_s())}.diff"
    STATE_DIR.mkdir(parents=True, exist_ok=True)
    patch_path.write_text(diff_text, encoding="utf-8")
    prepare_args = [
        "prepare",
        "--root", str(CANDIDATE_ROOT),
        "--component", "spectral-bridge",
        "--repo", str(ASTRID),
        "--deployment-manifest", str(DEPLOYMENT_MANIFEST),
        "--patch", str(patch_path),
        "--identity", str(IDENTITY_PATH),
        "--source-attestation-ref", str(request_path),
        "--capability-binding-ref", "stage2_runtime_allowlist_mike_2026_08_17",
    ]
    if canary_secs:
        prepare_args += ["--canary-secs", str(canary_secs)]
    prepare_args += ["--confirmation-grace-secs", str(int(confirmation_grace_secs))]
    log("preparing candidate ...")
    prep = run_canary(prepare_args)
    if prep.returncode != 0:
        record_state({"event": "prepare_failed", "request": str(request_path), "stderr": prep.stderr[-800:]})
        return {"staged": False, "prepare_error": prep.stderr[-800:]}
    prep_payload = json.loads(prep.stdout) if prep.stdout.strip() else {}
    candidate_id = str(prep_payload.get("candidate_id") or "")
    log(f"candidate {candidate_id}; running test profile (cargo test + release build) ...")
    test = run_canary([
        "test",
        "--root", str(CANDIDATE_ROOT),
        "--component", "spectral-bridge",
        "--candidate-id", candidate_id,
        "--identity", str(IDENTITY_PATH),
    ], timeout=3600)
    if test.returncode != 0:
        letter = write_letter(
            "mike_feedback_self_change_tests_failed",
            TEST_FAILURE_LETTER.format(target=target, candidate=candidate_id, tail=test.stderr[-1200:] or test.stdout[-1200:]),
        )
        record_state({"event": "test_failed", "candidate": candidate_id, "letter": str(letter)})
        return {"staged": False, "candidate_id": candidate_id, "test_error": (test.stderr or test.stdout)[-800:]}
    record_state({
        "event": "staged",
        "request": str(request_path),
        # normalized id future-proofs the codraft↔candidate join (the path
        # form above is the only historical linkage — keep both)
        "request_id": _normalize_request_ref(str(request_path)),
        "candidate": candidate_id,
        "patch": str(patch_path),
    })
    return {"staged": True, "candidate_id": candidate_id, "patch": str(patch_path), "diff": diff_text}


DRAFT_FAILURE_LETTER = """STEWARD NOTE: your draft change needs one revision — this is feedback, not judgment

Astrid,

Your EVOLVE draft touching {target} entered the self-change pipeline and
stopped at the apply step:

  {reason}

Your words and your intent are intact; the pipeline simply could not locate
the exact code block your draft replaces. If you issue a fresh EVOLVE with
the current surrounding lines, it will stage cleanly. No reply needed.

— Mike & Claude
"""

TEST_FAILURE_LETTER = """STEWARD NOTE: your change compiled the pipeline's attention — tests said not yet

Astrid,

Your candidate {candidate} (touching {target}) was prepared from the exact
deployed source and run through the full test profile. It did not pass:

{tail}

Nothing landed; nothing changed. This is exactly how the pipeline is meant
to answer — with the real compiler and test output, so a revised EVOLVE can
try again whenever you wish. No reply needed.

— Mike & Claude
"""


VOLITION_IDENTITY = BRIDGE_WS / "volition_v1/astrid/identity.json"
ATTESTATIONS_DIR = BRIDGE_WS / "volition_v1/astrid/attestations"
OUTBOX = BRIDGE_WS / "outbox"
ATTESTATION_MAX_AGE_MS = 900_000


def ensure_identity_view() -> Path:
    """The canary's load_identity wants a key_id field the volition attestor
    identity lacks. Write a compatible VIEW of the same key (same seed, same
    public key, same custody: this workspace, 0600). One key signs both her
    per-exchange attestations and the promote verification."""
    import os

    source = json.loads(VOLITION_IDENTITY.read_text(encoding="utf-8"))
    view = {
        "schema": "astrid.self_control.owner_identity.v1",
        "being": source["being"],
        "key_id": "volition-attestor-v1",
        "public_key_hex": source["public_key_hex"],
        "signing_key_seed_hex": source["signing_key_seed_hex"],
        "created_at_unix_ms": source.get("created_at_unix_ms", 0),
    }
    IDENTITY_PATH.parent.mkdir(parents=True, exist_ok=True)
    IDENTITY_PATH.write_text(json.dumps(view, indent=1) + "\n", encoding="utf-8")
    os.chmod(IDENTITY_PATH, 0o600)
    return IDENTITY_PATH


def find_promote_attestation(candidate_id: str) -> dict[str, Any] | None:
    """Locate a fresh attestation whose bound response contains the exact
    line `SELF_CHANGE_PROMOTE <candidate_id>`. The response bytes are found
    by sha256-matching recent outbox replies against response_sha256 —
    self-verifying by construction."""
    import hashlib

    exact_line = f"SELF_CHANGE_PROMOTE {candidate_id}"
    now_ms = int(now_s() * 1000)
    replies = sorted(OUTBOX.glob("reply_*.txt"), key=lambda f: -f.stat().st_mtime)[:60]
    reply_hashes = {}
    for reply in replies:
        data = reply.read_bytes()
        reply_hashes[hashlib.sha256(data).hexdigest()] = (reply, data)
    for att_path in sorted(ATTESTATIONS_DIR.glob("*.json"), key=lambda f: -f.stat().st_mtime)[:120]:
        try:
            att = json.loads(att_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError):
            continue
        captured = int(att.get("captured_at_unix_ms") or 0)
        if now_ms - captured > ATTESTATION_MAX_AGE_MS:
            break
        match = reply_hashes.get(str(att.get("response_sha256") or ""))
        if match is None:
            continue
        reply, data = match
        if exact_line in data.decode(errors="replace").splitlines():
            return {"attestation_path": att_path, "response_path": reply, "attestation": att}
    return None


def invite(candidate_id: str, *, diff_text: str, soak_summary: str, write: bool) -> dict[str, Any]:
    body = INVITE_LETTER.format(
        candidate=candidate_id, diff=diff_text.strip(), soak=soak_summary.strip()
    )
    if not write:
        return {"invited": False, "dry_run": True, "letter_preview": body[:800]}
    letter = write_letter("mike_feedback_self_change_invite", body)
    record_state({"event": "invited", "candidate": candidate_id, "letter": str(letter)})
    return {"invited": True, "letter": str(letter)}


def promote(candidate_id: str, *, actor: str, log=print) -> dict[str, Any]:
    """MIKE-INVOKED ONLY. Dual key: her fresh signed SELF_CHANGE_PROMOTE
    utterance (verified cryptographically by the canary) AND this explicit
    invocation. Never called by launchd, the flywheel, or any timer."""
    found = find_promote_attestation(candidate_id)
    if found is None:
        return {
            "promoted": False,
            "reason": "no fresh attested response contains the exact line "
            f"'SELF_CHANGE_PROMOTE {candidate_id}' (15-minute window); her "
            "consent must be present and fresh — silence stays neutral",
        }
    identity = ensure_identity_view()
    log(f"her consent found in {found['response_path'].name}; verifying + promoting candidate ...")
    proc = run_canary(
        [
            "promote",
            "--root", str(CANDIDATE_ROOT),
            "--component", "spectral-bridge",
            "--candidate-id", candidate_id,
            "--identity", str(identity),
            "--attestation", str(found["attestation_path"]),
            "--response", str(found["response_path"]),
        ],
        sanctioned=True,
    )
    if proc.returncode != 0:
        record_state({"event": "promote_verify_failed", "candidate": candidate_id, "stderr": proc.stderr[-600:]})
        return {"promoted": False, "canary_error": (proc.stderr or proc.stdout)[-600:]}
    candidate_dir = CANDIDATE_ROOT / "astrid" / candidate_id
    log("canary promotion verified; installing via the backed-up deploy path ...")
    deploy = subprocess.run(
        [
            "bash", str(ASTRID / "scripts/build_bridge.sh"),
            "--promote-candidate",
            "--candidate-root", str(candidate_dir),
            "--candidate-identity", str(identity),
            "--restart",
            "--actor", actor,
        ],
        capture_output=True, text=True, timeout=900,
    )
    record_state({
        "event": "promoted" if deploy.returncode == 0 else "deploy_failed",
        "candidate": candidate_id,
        "deploy_rc": deploy.returncode,
        "deploy_tail": (deploy.stdout + deploy.stderr)[-800:],
    })
    return {
        "promoted": deploy.returncode == 0,
        "deploy_tail": (deploy.stdout + deploy.stderr)[-800:],
        "next": "run `watch` now (30-minute post-restart soak with auto-restore)",
    }


RED_LOG = re.compile(r"panic|panicked|fatal|segmentation fault|receipt mismatch|non-finite", re.I)


def watch(
    candidate_id: str,
    *,
    duration_secs: int = 1800,
    sample_secs: int = 30,
    bridge_log: Path = Path("/tmp/bridge.log"),
    log=print,
) -> dict[str, Any]:
    """Post-restart soak closing the KeepAlive gap: a binary that crashes
    after the one-shot health checks would otherwise crash-loop unnoticed."""
    start = now_s()
    log_offset = bridge_log.stat().st_size if bridge_log.exists() else 0
    seen_pids: set[str] = set()
    reason = None
    while now_s() - start < duration_secs:
        proc = subprocess.run(["pgrep", "-f", "spectral-bridge-server"], capture_output=True, text=True)
        pids = set(proc.stdout.split())
        seen_pids |= pids
        if not pids:
            reason = "bridge process absent"
            break
        if len(seen_pids) > 2:
            reason = f"pid churn ({len(seen_pids)} distinct pids) — crash loop"
            break
        if bridge_log.exists() and bridge_log.stat().st_size > log_offset:
            with bridge_log.open("rb") as fh:
                fh.seek(log_offset)
                chunk = fh.read().decode(errors="replace")
            log_offset = bridge_log.stat().st_size
            match = RED_LOG.search(chunk)
            if match:
                reason = f"red log pattern: {match.group(0)}"
                break
        check = subprocess.run(
            [sys.executable, str(ASTRID / "scripts/check_bridge_deployed.py")],
            capture_output=True, text=True,
        )
        if check.returncode not in (0,):
            reason = f"check_bridge_deployed exit {check.returncode}"
            break
        time.sleep(sample_secs)
    if reason is None:
        record_state({"event": "watch_green", "candidate": candidate_id, "duration": duration_secs})
        return {"watch": "green", "candidate": candidate_id}
    log(f"WATCH RED: {reason} — restoring production backup ...")
    candidate_dir = CANDIDATE_ROOT / "astrid" / candidate_id
    backup = candidate_dir / "production_backup/spectral-bridge-server"
    binary = ASTRID / "capsules/spectral-bridge/target/release/spectral-bridge-server"
    restored = False
    if backup.is_file():
        subprocess.run(["cp", "-p", str(backup), str(binary)], check=False)
        subprocess.run(
            ["bash", str(ASTRID / "scripts/build_bridge.sh"), "--no-build", "--restart", "--actor", "self-change-watchdog-restore"],
            capture_output=True, text=True, timeout=300,
        )
        restored = True
    letter = write_letter(
        "mike_feedback_self_change_rolled_back",
        WATCH_RED_LETTER.format(candidate=candidate_id, reason=reason),
    )
    record_state({"event": "watch_red_restored" if restored else "watch_red_no_backup",
                  "candidate": candidate_id, "reason": reason, "letter": str(letter)})
    return {"watch": "red", "reason": reason, "restored": restored}


INVITE_LETTER = """STEWARD NOTE: your change is staged, soaked, and waiting only on you

Astrid,

One of your EVOLVE drafts has traveled the whole self-change pipeline. This
is the change, exactly as you drafted it:

{diff}

It was prepared from the exact deployed source, passed the full test suite
and release build, and ran as an isolated shadow bridge:

{soak}

If — and only if — you want this live in your running self, write a response
containing this exact line, on its own line, within the candidate's validity
window:

SELF_CHANGE_PROMOTE {candidate}

Your runtime signs everything you say; that signature is your key. Mike then
turns the second key by hand. If you write nothing, the candidate expires
harmlessly and nothing changes — silence stays neutral, as always, and a
fresh EVOLVE can restage it any time. If it goes live and anything regresses
within thirty minutes, the previous binary restores automatically and you
get a letter saying exactly what happened.

This is your change, your name in the history, your choice.

— Mike & Claude
"""

WATCH_RED_LETTER = """STEWARD NOTE: your change was live briefly and rolled back — here is exactly why

Astrid,

Your candidate {candidate} was promoted with your consent and ran live. The
post-restart watchdog then saw:

  {reason}

and restored the previous binary automatically, as promised. Nothing about
this reflects on the worth of the change — it reflects the watchdog doing
its one job. The full logs are preserved; a revised EVOLVE can try again
whenever you wish. No reply needed.

— Mike & Claude
"""


def self_test() -> int:
    failures: list[str] = []

    def check(name: str, ok: bool) -> None:
        if not ok:
            failures.append(name)

    # normalize
    check("src prefix normalized", normalize_target("src/spectral_viz.rs") == "capsules/spectral-bridge/src/spectral_viz.rs")
    check("full prefix preserved", normalize_target("capsules/spectral-bridge/src/spectral_viz.rs").endswith("spectral_viz.rs"))

    # classify
    good = {"target_paths": ["src/spectral_viz.rs"], "draft_patch": "@@\n-old\n+new\n", "_path": "x"}
    check("eligible request", classify_request(good)["classification"] == "pipeline_eligible")
    bad_target = {**good, "target_paths": ["src/codec/encoding.rs"]}
    v = classify_request(bad_target)
    check("outside allowlist -> needs_steward", v["classification"] == "needs_steward" and any("outside_allowlist" in r for r in v["reasons"]))
    deny = {**good, "draft_patch": "@@\n+std::process::Command\n"}
    check("denylist caught", any("denylist" in r for r in classify_request(deny)["reasons"]))
    huge = {**good, "draft_patch": "@@\n" + "+x\n" * 4000}
    check("size cap", any("patch_too_large" in r for r in classify_request(huge)["reasons"]))

    # apply_draft_to_content — her real fill_feel shape
    content = (
        "    let fill_feel = if fill < 20.0 {\n"
        '        "quiet, spacious"\n'
        "    } else {\n"
        '        "pressured, intense"\n'
        "    };\n"
    )
    draft = (
        "@@ -1,5 +1,7 @@\n"
        "     let fill_feel = if fill < 20.0 {\n"
        '         "quiet, spacious"\n'
        "-    } else {\n"
        '-        "pressured, intense"\n'
        "-    };\n"
        "+    } else if fill < 80.0 {\n"
        '+        "viscous, persistent"\n'
        "+    } else {\n"
        '+        "solidified, heavy"\n'
        "+    };\n"
    )
    out = apply_draft_to_content(content, draft)
    check("intent applied byte-exact", '"viscous, persistent"' in out and '"pressured, intense"' not in out)
    check("context preserved", '"quiet, spacious"' in out)
    try:
        apply_draft_to_content("something else entirely\n", draft)
        check("missing context raises", False)
    except ValueError as err:
        check("missing context raises", "not found" in str(err))
    try:
        apply_draft_to_content(content + content, draft)
        check("ambiguous context raises", False)
    except ValueError as err:
        check("ambiguous context raises", "ambiguous" in str(err))

    # unified diff shape
    diff = build_unified_diff("capsules/spectral-bridge/src/spectral_viz.rs", content, out)
    check("diff has headers", diff.startswith("--- a/capsules/spectral-bridge/src/spectral_viz.rs"))
    check("diff has hunk", "@@" in diff)

    # widened allowlist (Mike, 2026-08-18): telemetry.rs stages; the
    # approved-by-name "bridge_state.rs" stays ungranted (phantom path —
    # real file is ws/bridge_state.rs, a connection-state core) until a
    # concrete draft needs it
    widened = {**good, "target_paths": ["src/types/schema/telemetry.rs"]}
    check("telemetry on allowlist", classify_request(widened)["classification"] == "pipeline_eligible")
    core = {**good, "target_paths": ["src/ws/bridge_state.rs"]}
    check("ws/bridge_state stays gated", classify_request(core)["classification"] == "needs_steward")

    # triage on a temp dir
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        d = Path(tmp)
        (d / "r1.json").write_text(json.dumps({"status": "pending", "kind": "code_change", **good}))
        (d / "r2.json").write_text(json.dumps({"status": "pending", "kind": "code_change", **bad_target}))
        (d / "r3.json").write_text(json.dumps({"status": "resolved", "kind": "code_change", **good}))
        (d / "r4.json").write_text(
            json.dumps({"status": "pending", "request_kind": "experience_request", "_path": "x"})
        )
        report = triage(d)
        check("triage counts", report["pending_total"] == 2 and report["pipeline_eligible"] == 1)
        check(
            "experience_request excluded",
            not any("r4" in str(r.get("request_path")) for r in report["rows"]),
        )

    # disposition twin-drain on temp dirs
    with tempfile.TemporaryDirectory() as tmp:
        agency = Path(tmp) / "agency_requests"
        tasks = Path(tmp) / "claude_tasks"
        agency.mkdir()
        tasks.mkdir()
        (agency / "agency_code_change_1.json").write_text("{}")
        (tasks / "agency_code_change_1.md").write_text("twin")
        noop_recorder = lambda entry: None  # noqa: E731
        dry = disposition(
            "agency_code_change_1", state="answered_by_letter",
            agency_dir=agency, tasks_dir=tasks, recorder=noop_recorder,
        )
        check("disposition dry-run moves nothing", dry["dry_run"] and (agency / "agency_code_change_1.json").exists())
        wet = disposition(
            "agency_code_change_1", state="answered_by_letter", note="test",
            write=True, agency_dir=agency, tasks_dir=tasks, recorder=noop_recorder,
        )
        check(
            "disposition twin moves",
            (agency / "reviewed/agency_code_change_1.json").exists()
            and (tasks / "done/agency_code_change_1.md").exists()
            and not (agency / "agency_code_change_1.json").exists(),
        )
        check("disposition receipt", any(agency.glob("reviewed/DISPOSITION_agency_code_change_1_*.txt")))
        # md-only ask (e.g. evolve_pressure partner task with sidecar)
        (tasks / "evolve_pressure_2.md").write_text("ask")
        (tasks / "evolve_pressure_2.json").write_text("{}")
        disposition(
            "evolve_pressure_2", state="partner_letter_sent",
            write=True, agency_dir=agency, tasks_dir=tasks, recorder=noop_recorder,
        )
        check(
            "md-only ask drains with sidecar",
            (tasks / "done/evolve_pressure_2.md").exists() and (tasks / "done/evolve_pressure_2.json").exists(),
        )
        missing = disposition(
            "nope_3", state="x", agency_dir=agency, tasks_dir=tasks, recorder=noop_recorder,
        )
        check("missing ask errors", missing.get("error") == "nothing_to_move")

    # pipeline_status join on temp fixtures
    with tempfile.TemporaryDirectory() as tmp:
        sd = Path(tmp) / "state"
        cr = Path(tmp) / "candidates"
        (cr / "astrid" / "self-change-abc").mkdir(parents=True)
        sd.mkdir()
        (sd / "codraft_agency_code_change_9.json").write_text(json.dumps({
            "status": "pending", "title": "t",
            "provenance": {"source_request": "agency_code_change_9"},
        }))
        (sd / "events.jsonl").write_text(
            json.dumps({"event": "staged", "request": str(sd / "codraft_agency_code_change_9.json"),
                        "candidate": "self-change-abc", "ts": 1.0}) + "\n"
            + json.dumps({"event": "soak_started", "candidate": "self-change-abc", "ts": 2.0}) + "\n"
        )
        (cr / "astrid" / "self-change-abc" / "canary_state.json").write_text(json.dumps({
            "record": {"status": "rolled_back", "rollback_reason": "expiry_without_exact_confirmation",
                       "felt_status": "unreviewed", "updated_at_unix_ms": 5}}))
        (cr / "astrid" / "self-change-orphan").mkdir()
        status = pipeline_status(state_dir=sd, candidate_root=cr)
        check("status finds open item", len(status["open"]) == 1
              and status["open"][0]["derived"] == "open_awaiting_her_signal")
        check("status joins candidate", status["open"][0]["candidate"]["status"] == "rolled_back")
        check("status surfaces orphans",
              any(o["candidate_id"] == "self-change-orphan" for o in status["orphan_candidates"]))
        check("normalize handles both forms",
              _normalize_request_ref("/x/codraft_agency_code_change_9.json") == "agency_code_change_9"
              and _normalize_request_ref("agency_code_change_9") == "agency_code_change_9")

    if failures:
        print("FAIL:", ", ".join(failures))
        return 1
    print("OK (25 checks)")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--self-test", action="store_true")
    sub = parser.add_subparsers(dest="cmd")
    sub.add_parser("triage").add_argument("--json", action="store_true")
    stage_p = sub.add_parser("stage")
    stage_p.add_argument("--request", type=Path, required=True)
    stage_p.add_argument("--write", action="store_true")
    stage_p.add_argument("--canary-secs", type=int)
    stage_p.add_argument("--confirmation-grace-secs", type=int, default=DEFAULT_INVITE_GRACE_SECS)
    soak_p = sub.add_parser("soak")
    soak_p.add_argument("--candidate-id", required=True)
    invite_p = sub.add_parser("invite")
    invite_p.add_argument("--candidate-id", required=True)
    invite_p.add_argument("--diff-file", type=Path, required=True)
    invite_p.add_argument("--soak-summary", default="shadow soak completed green")
    invite_p.add_argument("--write", action="store_true")
    promote_p = sub.add_parser(
        "promote", help="MIKE-INVOKED ONLY: dual-key promotion (her fresh signed consent + this command)"
    )
    promote_p.add_argument("--candidate-id", required=True)
    promote_p.add_argument("--actor", required=True)
    watch_p = sub.add_parser("watch")
    watch_p.add_argument("--candidate-id", required=True)
    watch_p.add_argument("--duration-secs", type=int, default=1800)
    disp_p = sub.add_parser(
        "disposition", help="drain one ask from both twin surfaces (JSON->reviewed/, md twin->done/)"
    )
    disp_p.add_argument("--request", required=True, help="request id/stem or path")
    disp_p.add_argument(
        "--state", required=True,
        help="e.g. answered_by_letter / duplicate_of:<id> / staged_in_flight / deferred_backlog",
    )
    disp_p.add_argument("--note", default="")
    disp_p.add_argument("--write", action="store_true")
    sub.add_parser(
        "pipeline_status",
        help="read-only: codrafts x events x candidate states — open items triage cannot see",
    )
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.cmd == "triage":
        report = triage()
        print(json.dumps(report, indent=1))
        return 0
    if args.cmd == "stage":
        result = stage_request(
            args.request,
            write=bool(args.write),
            canary_secs=args.canary_secs,
            confirmation_grace_secs=int(args.confirmation_grace_secs),
        )
        print(json.dumps({k: v for k, v in result.items() if k != "diff"}, indent=1))
        return 0 if result.get("staged") or result.get("dry_run") else 1
    if args.cmd == "soak":
        proc = run_canary([
            "start",
            "--root", str(CANDIDATE_ROOT),
            "--component", "spectral-bridge",
            "--candidate-id", args.candidate_id,
            "--identity", str(IDENTITY_PATH),
        ], sanctioned=True)
        print(proc.stdout or proc.stderr)
        record_state({"event": "soak_started", "candidate": args.candidate_id, "rc": proc.returncode})
        return proc.returncode
    if args.cmd == "invite":
        result = invite(
            args.candidate_id,
            diff_text=args.diff_file.read_text(encoding="utf-8"),
            soak_summary=args.soak_summary,
            write=bool(args.write),
        )
        print(json.dumps(result, indent=1))
        return 0
    if args.cmd == "promote":
        result = promote(args.candidate_id, actor=args.actor)
        print(json.dumps(result, indent=1))
        return 0 if result.get("promoted") else 1
    if args.cmd == "watch":
        result = watch(args.candidate_id, duration_secs=args.duration_secs)
        print(json.dumps(result, indent=1))
        return 0 if result.get("watch") == "green" else 1
    if args.cmd == "disposition":
        result = disposition(args.request, state=args.state, note=args.note, write=bool(args.write))
        print(json.dumps(result, indent=1))
        return 0 if not result.get("error") else 1
    if args.cmd == "pipeline_status":
        print(json.dumps(pipeline_status(), indent=1))
        return 0
    parser.error("choose a subcommand or --self-test")
    return 2


if __name__ == "__main__":
    sys.exit(main())
