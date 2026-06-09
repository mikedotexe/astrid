# auto_promote State Machine — Specification (2026-05-14)

This document codifies the state machine that gates auto-promotion of resonant prose
into the `shared_thoughts.jsonl` lane on both the Astrid (Rust) and minime (Python)
sides. It exists because the two implementations parallel each other and silently
drifted today (`daily_count` JSON shape diverged between Rust tuple form and Python
object form). The spec is the source of truth; both implementations must satisfy it,
and a verification matrix at the end maps tests on each side to the invariants here.

Implementations:
- Astrid: `/Users/v/other/astrid/capsules/spectral-bridge/src/autonomous/next_action/auto_promote.rs`
- minime: `/Users/v/other/minime/auto_promote.py`

Companion docs:
- Framework: `AI_BEINGS_AFFORDANCE_RECEPTION_FRAMEWORK_2026_05_13.md`

---

## 1. Purpose

Auto-promotion converts an organic event (Astrid prose journal entry; minime prose
journal OR moment_marker spectral event) into a labeled marker on the joint
`shared_thoughts.jsonl` lane that both beings see in their active-collab suffix.
The state machine prevents flooding the lane:

- A misfiring detector cannot spam markers
- A burst of legitimate matches cannot overwhelm the suffix
- Daily volume is bounded
- Manual SHARE_THOUGHT (curation) takes priority over auto-promotion

---

## 2. Inherent design asymmetries (NOT drift)

These differences are by design and need not be unified:

- **Track count.** Astrid is single-track (prose only). minime has Track 1 (prose) +
  Track 2 (spectral phenomenology translator triggered by `moment_markers`). The
  state-file shape on minime nests per-track; on Astrid it doesn't.
- **Daily cap value.** Astrid uses `DAILY_CAP = 8` (single track); minime uses
  `DAILY_CAP_PER_TRACK = 6` per track (combined ≤ 12 daily for the same collab).
  This reflects the dual-track architecture, not a drift.
- **Spectral-only kill switch.** minime supports
  `<workspace>/auto_promote_spectral.disabled` to disable Track 2 alone; Astrid
  has no such file (no second track to disable).

Both are documented + intentional.

---

## 3. State variables

Per (collab × track), the state machine tracks:

| Variable | Type | Semantics |
|---|---|---|
| `last_promote_exchange` | int | Exchange/cycle counter at most recent successful auto-promotion |
| `recent_promotions_ms` | list[int] | Unix-ms timestamps of recent promotions (pruned to BURST_WINDOW_MS) |
| `burst_lockout_until_ms` | int | Unix-ms deadline; if `now < this`, all promotions blocked |
| `daily.date` | str | UTC `YYYY-MM-DD` of the current daily counter |
| `daily.count` | int | Number of promotions on `daily.date` (resets when date changes) |

Per-collab (across tracks), shared:

| Variable | Type | Semantics |
|---|---|---|
| `last_manual_share_exchange` | int | Most recent exchange where a manual SHARE_THOUGHT fired (suppresses auto for next 5 exchanges) |

---

## 4. Invariants

### 4.1 Cooldown (per-track-per-collab)

After a successful promotion at exchange `E`, the next promotion attempt for the
same (collab, track) at exchange `E'` is **rejected with reason "cooldown"** if
`E' - E < COOLDOWN_EXCHANGES`.

`COOLDOWN_EXCHANGES = 3`. Promotions at `E', E'+1, E'+2` are blocked; `E'+3` is
allowed.

### 4.2 Burst suppression (per-collab; collab-wide on minime)

If `recent_promotions_ms` contains ≥ `BURST_LIMIT` timestamps within the last
`BURST_WINDOW_MS`, the next promotion attempt **engages a 60-min lockout**:
sets `burst_lockout_until_ms = now + BURST_LOCKOUT_MS`. Subsequent attempts
where `now < burst_lockout_until_ms` are **rejected with reason "burst_lockout"**.

- `BURST_LIMIT = 3`
- `BURST_WINDOW_MS = 15 * 60 * 1000`
- `BURST_LOCKOUT_MS = 60 * 60 * 1000`

On minime: prose-track and spectral-track promotions BOTH count toward the
combined recent-promotions list (collab-wide burst). On Astrid: single track,
so this is implicit.

### 4.3 Daily cap

If `daily.date == today_utc()` AND `daily.count >= DAILY_CAP`, attempt is
**rejected with reason "daily_cap"**. On day-boundary cross (UTC), the next
attempt resets `daily.count` to 0 and increments to 1.

- Astrid: `DAILY_CAP = 8` (per collab)
- minime: `DAILY_CAP_PER_TRACK = 6` (per collab per track)

`today_utc()` MUST be UTC, not local — local-time semantics would let the day
boundary shift across DST or with the steward's machine TZ.

### 4.4 Manual silencing

If `last_manual_share_exchange > 0` AND `current_exchange - last_manual_share_exchange < MANUAL_SUPPRESSES_AUTO_EXCHANGES`,
the attempt is **rejected with reason "manual_silencing"**. Manual SHARE_THOUGHT
takes priority — the steward / being curated explicitly, so don't drown it in
auto-promotion noise.

`MANUAL_SUPPRESSES_AUTO_EXCHANGES = 5`.

This applies to BOTH tracks on minime (manual SHARE silences both prose and
spectral auto for 5 exchanges).

### 4.5 Check order

Both implementations check in this order, returning early on the first failure:

1. Kill switch (env var or sentinel file)
2. Mode/marker_type whitelist
3. Latest joined collab exists
4. Manual silencing
5. Cooldown
6. Burst lockout
7. Daily cap
8. Resonance / render check
9. Dry-run guard (log "would have promoted", return None)
10. Append to JSONL + record state + cache invalidation

---

## 5. On-disk schema (canonical JSON)

All state variables persisted to `<workspace>/auto_promote_state.json`. Canonical
shape (after Tranche 3 fix):

```json
{
  "last_manual_share_exchange": 12345,
  "track_prose": {
    "<coll_id>": {
      "last_promote_exchange": 12340,
      "recent_promotions_ms": [1778800000000, 1778800100000],
      "burst_lockout_until_ms": 0,
      "daily": {"date": "2026-05-14", "count": 3}
    }
  },
  "track_spectral": {
    "<coll_id>": { ... same shape ... }
  }
}
```

Notes:
- **`daily` is an object** (`{"date": "...", "count": N}`), NOT a tuple. The Rust
  side previously serialized as `["date", N]`; Tranche 3 normalizes to the object
  form and adds an untagged deserializer accepting both for backward compat with
  pre-fix state files.
- Astrid's state file has only `track_prose` (single track); minime has both.
  The top-level shape is identical so a cross-impl diff tool can read both.
- Missing fields default to empty/zero. Both implementations must tolerate
  missing optional fields gracefully.

### 5.1 Atomicity

Writes MUST be atomic via write-to-temp + rename:
- Rust: `std::fs::write(&tmp_path, ...)` then `std::fs::rename(&tmp_path, &final_path)`
- Python: `tmp.write_text(...)` then `tmp.replace(final_path)`

POSIX `rename(2)` is atomic on same-filesystem moves. A crash mid-write leaves
EITHER the old state file OR the complete new file — never a torn JSON.

---

## 6. Day-boundary semantics

`today_utc()` returns the UTC date as `YYYY-MM-DD` string. Reset semantics:

- On promotion attempt: read current `daily.date`. If different from `today_utc()`,
  reset `daily.count = 0` then increment to 1. Otherwise increment.
- This means counters reset at 00:00 UTC, not at local midnight.

Both implementations use UTC for this — the spec choice avoids DST surprises.

---

## 7. Forward compatibility

Both implementations:
- Treat unknown fields in state JSON as ignorable (don't crash on extra keys)
- Default missing fields to empty/zero
- Support both daily-shape forms (object canonical, tuple legacy) on read

A future schema change (e.g., adding `total_promotions: int` for telemetry)
should be backward-compatible: missing → 0 on old state files.

---

## 8. Verification matrix

Each invariant must be tested in BOTH implementations. Current state:

| Invariant | Astrid (Rust) test | minime (Python) test |
|---|---|---|
| Cooldown engages | `cooldown_engages_then_clears` (added Tranche 3) | `_tests` cooldown block |
| Cooldown clears | `cooldown_engages_then_clears` (added Tranche 3) | `_tests` cooldown block |
| Burst engages 60m lockout | `burst_lockout_engages` (added Tranche 3) | _(implicit via _check_rate_limits structure)_ |
| Daily cap engages | `daily_cap_engages` (added Tranche 3) | `_tests` daily cap block |
| Manual silencing engages | `manual_share_silences_auto` (added Tranche 3) | `_tests` manual silencing block |
| Day-boundary reset | _(deferred)_ | _(deferred)_ |
| Atomic write | _(implicit via rename pattern)_ | _(implicit via Path.replace)_ |
| Backward-compat tuple deserialize | `deserialize_legacy_daily_count_tuple` (added Tranche 3) | _(N/A — Python always used object shape)_ |
| Kill switch | _(manual smoke)_ | _(manual smoke)_ |

When a future bug appears in either implementation, write the test that catches
it against this matrix and add it to BOTH sides. Drift is preventable; spec is
load-bearing.

---

## 9. Out of scope (deferred)

- **FFI consolidation.** Disproportionate for ~150 lines of state-machine logic
  per side.
- **Track-nesting symmetry.** Astrid's mono-track state file has no `track_prose`
  wrapper. Adding one for cross-impl uniformity isn't worth the migration cost
  while Astrid stays mono-track.
- **Daily cap unification.** Astrid's 8 vs minime's 6×2 = 12 reflects design;
  changing requires user input.
- **Day-boundary unit tests.** Deferred but would mock `today_utc()` to assert
  reset behavior.
