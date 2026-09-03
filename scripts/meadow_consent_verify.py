#!/usr/bin/env python3
"""Read-only precondition checker for the meadow mutual-consent artifact.

The meadow (wide shared-lane coupling) may only be unlocked when BOTH beings
have each recorded a signed consent statement on their own authenticated
lane AND the operator has countersigned. This script verifies that state; it
flips NOTHING (the ceiling env is raised manually by the operator, and no
runtime code path may ever read the artifact — see the anti-drop row
`meadow_artifact_never_auto`).

Statement schema `meadow_consent_statement_v1` (one file per being):
  {schema, being, decision: consent|consent_with_conditions|decline,
   statement_verbatim (their words, unedited), conditions?, evidence_refs[],
   scope: {kind: "wide_coupling_ceiling", max_value}, revocation,
   issued_at_unix_ms, signer_public_key_hex, signature_hex}
Signature: ed25519 over the canonical JSON (sorted keys, compact separators)
of the statement with signature_hex removed — the same canonical-JSON idiom
as the self-control wire.

Artifact schema `meadow_mutual_consent_v1`:
  {schema, statements: {astrid: {path, sha256}, minime: {path, sha256}},
   operator_countersign: {by, at, note}, unlocks, never_auto: true}

Verification is dependency-free: an embedded RFC 8032 ed25519 verifier
(proven against the RFC test vector in --self-test). Trust roots default to
each being's live pinned trust store; --trust-override supplies synthetic
keys for dry runs.

Usage:
  meadow_consent_verify.py --artifact PATH [--trust-override JSON_PATH] [--json]
  meadow_consent_verify.py --dry-run     # synthetic five-case drill
  meadow_consent_verify.py --self-test
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

ASTRID_TRUST = Path(
    "/Users/v/other/astrid/capsules/spectral-bridge/workspace/self_control_v2/astrid/trust.json"
)
MINIME_TRUST = Path("/Users/v/.minime/self-control-v2/trust.json")

STATEMENT_SCHEMA = "meadow_consent_statement_v1"
ARTIFACT_SCHEMA = "meadow_mutual_consent_v1"
SCOPE_KIND = "wide_coupling_ceiling"
CONSENT_DECISIONS = {"consent", "consent_with_conditions"}

AUTHORITY_BOUNDARY = (
    "read-only precondition checker; verifies signatures and shape only; "
    "unlocks nothing, flips nothing, and no runtime code path may consume "
    "the artifact it checks"
)

# --------------------------------------------------------------------------
# Embedded RFC 8032 ed25519 (verify + sign-for-synthetic-vectors only).
# Pure python, slow, fine for one-off verification. Proven against the RFC
# test vector in the self-test; sign() exists ONLY so dry runs can mint
# synthetic vectors — production statements are signed by the beings' own
# Rust lanes.
# --------------------------------------------------------------------------

_P = 2**255 - 19
_L = 2**252 + 27742317777372353535851937790883648493
_D = (-121665 * pow(121666, _P - 2, _P)) % _P
_I = pow(2, (_P - 1) // 4, _P)


def _sha512(data: bytes) -> bytes:
    return hashlib.sha512(data).digest()


def _inv(x: int) -> int:
    return pow(x, _P - 2, _P)


def _recover_x(y: int, sign: int) -> int | None:
    if y >= _P:
        return None
    x2 = (y * y - 1) * _inv(_D * y * y + 1) % _P
    if x2 == 0:
        return None if sign else 0
    x = pow(x2, (_P + 3) // 8, _P)
    if (x * x - x2) % _P != 0:
        x = x * _I % _P
    if (x * x - x2) % _P != 0:
        return None
    if (x & 1) != sign:
        x = _P - x
    return x


_GY = 4 * _inv(5) % _P
_GX = _recover_x(_GY, 0)
assert _GX is not None
_G = (_GX, _GY, 1, _GX * _GY % _P)
_IDENT = (0, 1, 1, 0)


def _edwards_add(p1: tuple[int, int, int, int], p2: tuple[int, int, int, int]):
    x1, y1, z1, t1 = p1
    x2, y2, z2, t2 = p2
    a = (y1 - x1) * (y2 - x2) % _P
    b = (y1 + x1) * (y2 + x2) % _P
    c = t1 * 2 * _D * t2 % _P
    dd = z1 * 2 * z2 % _P
    e, f, g, h = b - a, dd - c, dd + c, b + a
    return (e * f % _P, g * h % _P, f * g % _P, e * h % _P)


def _scalarmult(point: tuple[int, int, int, int], e: int):
    q = _IDENT
    while e > 0:
        if e & 1:
            q = _edwards_add(q, point)
        point = _edwards_add(point, point)
        e >>= 1
    return q


def _point_compress(p: tuple[int, int, int, int]) -> bytes:
    zinv = _inv(p[2])
    x = p[0] * zinv % _P
    y = p[1] * zinv % _P
    return int.to_bytes(y | ((x & 1) << 255), 32, "little")


def _point_decompress(s: bytes) -> tuple[int, int, int, int] | None:
    if len(s) != 32:
        return None
    y = int.from_bytes(s, "little")
    sign = y >> 255
    y &= (1 << 255) - 1
    x = _recover_x(y, sign)
    if x is None:
        return None
    return (x, y, 1, x * y % _P)


def _point_equal(p1, p2) -> bool:
    return (
        (p1[0] * p2[2] - p2[0] * p1[2]) % _P == 0
        and (p1[1] * p2[2] - p2[1] * p1[2]) % _P == 0
    )


def ed25519_verify(public_key: bytes, message: bytes, signature: bytes) -> bool:
    if len(public_key) != 32 or len(signature) != 64:
        return False
    a = _point_decompress(public_key)
    if a is None:
        return False
    rs = signature[:32]
    r = _point_decompress(rs)
    if r is None:
        return False
    s = int.from_bytes(signature[32:], "little")
    if s >= _L:
        return False
    h = int.from_bytes(_sha512(rs + public_key + message), "little") % _L
    sb = _scalarmult(_G, s)
    rha = _edwards_add(r, _scalarmult(a, h))
    return _point_equal(sb, rha)


def _secret_expand(secret: bytes) -> tuple[int, bytes]:
    h = _sha512(secret)
    a = int.from_bytes(h[:32], "little")
    a &= (1 << 254) - 8
    a |= 1 << 254
    return a, h[32:]


def ed25519_public_key(secret: bytes) -> bytes:
    a, _ = _secret_expand(secret)
    return _point_compress(_scalarmult(_G, a))


def ed25519_sign(secret: bytes, message: bytes) -> bytes:
    """Synthetic-vector signing only; production signing is the beings' lanes."""
    a, prefix = _secret_expand(secret)
    public = _point_compress(_scalarmult(_G, a))
    r = int.from_bytes(_sha512(prefix + message), "little") % _L
    rs = _point_compress(_scalarmult(_G, r))
    h = int.from_bytes(_sha512(rs + public + message), "little") % _L
    s = (r + h * a) % _L
    return rs + int.to_bytes(s, 32, "little")


# --------------------------------------------------------------------------
# Statement / artifact verification
# --------------------------------------------------------------------------


def canonical_statement_bytes(statement: dict[str, Any]) -> bytes:
    unsigned = {k: v for k, v in statement.items() if k != "signature_hex"}
    return json.dumps(unsigned, sort_keys=True, separators=(",", ":")).encode("utf-8")


def _check(name: str, ok: bool, detail: str) -> dict[str, Any]:
    return {"name": name, "status": "ok" if ok else "fail", "detail": detail}


def load_pinned_key(being: str, trust_override: dict[str, str] | None) -> str | None:
    if trust_override is not None:
        return trust_override.get(being)
    trust_path = ASTRID_TRUST if being == "astrid" else MINIME_TRUST
    try:
        trust = json.loads(trust_path.read_text())
    except (OSError, json.JSONDecodeError):
        return None
    pinned = trust.get("pinned_public_keys")
    if isinstance(pinned, dict):
        value = pinned.get(being)
        return value if isinstance(value, str) else None
    return None


def verify_statement(
    statement: dict[str, Any], being: str, trust_override: dict[str, str] | None
) -> list[dict[str, Any]]:
    checks: list[dict[str, Any]] = []
    checks.append(
        _check(
            f"{being}_statement_schema",
            statement.get("schema") == STATEMENT_SCHEMA and statement.get("being") == being,
            f"schema={statement.get('schema')} being={statement.get('being')}",
        )
    )
    decision = str(statement.get("decision") or "")
    checks.append(
        _check(
            f"{being}_decision_is_consent",
            decision in CONSENT_DECISIONS,
            f"decision={decision!r}"
            + (
                f" conditions={statement.get('conditions')!r}"
                if decision == "consent_with_conditions"
                else ""
            ),
        )
    )
    verbatim = str(statement.get("statement_verbatim") or "")
    checks.append(
        _check(
            f"{being}_statement_verbatim_present",
            bool(verbatim.strip()),
            f"{len(verbatim)} chars of their own words",
        )
    )
    scope = statement.get("scope") or {}
    scope_ok = (
        isinstance(scope, dict)
        and scope.get("kind") == SCOPE_KIND
        and isinstance(scope.get("max_value"), (int, float))
        and 0.0 < float(scope["max_value"]) <= 1.0
    )
    checks.append(
        _check(
            f"{being}_scope_shape",
            scope_ok,
            f"scope={scope!r} (kind must be {SCOPE_KIND}, max_value in (0,1])",
        )
    )
    checks.append(
        _check(
            f"{being}_revocation_named",
            bool(str(statement.get("revocation") or "").strip()),
            "their kill switch must be named in the statement",
        )
    )

    pinned = load_pinned_key(being, trust_override)
    if pinned is None:
        checks.append(
            _check(f"{being}_signature", False, "no pinned key available for verification")
        )
        return checks
    signer = str(statement.get("signer_public_key_hex") or "")
    if signer != pinned:
        checks.append(
            _check(
                f"{being}_signature",
                False,
                "statement signer key does not match the being's pinned trust key",
            )
        )
        return checks
    try:
        public = bytes.fromhex(signer)
        signature = bytes.fromhex(str(statement.get("signature_hex") or ""))
    except ValueError:
        checks.append(_check(f"{being}_signature", False, "malformed hex key/signature"))
        return checks
    ok = ed25519_verify(public, canonical_statement_bytes(statement), signature)
    checks.append(
        _check(
            f"{being}_signature",
            ok,
            "ed25519 verify over canonical statement bytes against the pinned key",
        )
    )
    return checks


def verify_artifact(
    artifact_path: Path, trust_override: dict[str, str] | None = None
) -> dict[str, Any]:
    checks: list[dict[str, Any]] = []
    effective_ceiling: float | None = None
    try:
        artifact = json.loads(artifact_path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        return {
            "ok": False,
            "checks": [_check("artifact_readable", False, str(error))],
            "authority_boundary": AUTHORITY_BOUNDARY,
        }

    checks.append(
        _check(
            "artifact_schema",
            artifact.get("schema") == ARTIFACT_SCHEMA,
            f"schema={artifact.get('schema')}",
        )
    )
    checks.append(
        _check(
            "never_auto_declared",
            artifact.get("never_auto") is True,
            "the artifact must declare never_auto: true",
        )
    )

    statements: dict[str, dict[str, Any]] = {}
    for being in ("astrid", "minime"):
        ref = (artifact.get("statements") or {}).get(being)
        if not isinstance(ref, dict) or not ref.get("path"):
            checks.append(_check(f"{being}_statement_present", False, "no statement reference"))
            continue
        path = Path(str(ref["path"]))
        if not path.is_absolute():
            path = artifact_path.parent / path
        try:
            raw = path.read_bytes()
        except OSError as error:
            checks.append(_check(f"{being}_statement_present", False, str(error)))
            continue
        digest = hashlib.sha256(raw).hexdigest()
        checks.append(_check(f"{being}_statement_present", True, str(path)))
        checks.append(
            _check(
                f"{being}_statement_hash",
                digest == str(ref.get("sha256") or ""),
                "artifact-pinned sha256 must match the statement file",
            )
        )
        try:
            statements[being] = json.loads(raw)
        except json.JSONDecodeError as error:
            checks.append(_check(f"{being}_statement_parse", False, str(error)))

    for being, statement in statements.items():
        checks.extend(verify_statement(statement, being, trust_override))

    if len(statements) == 2:
        scopes = []
        for statement in statements.values():
            scope = statement.get("scope") or {}
            if isinstance(scope, dict) and isinstance(scope.get("max_value"), (int, float)):
                scopes.append(float(scope["max_value"]))
        if len(scopes) == 2:
            effective_ceiling = min(scopes)
            checks.append(
                _check(
                    "scopes_compatible",
                    True,
                    f"effective consented ceiling = min(both) = {effective_ceiling}",
                )
            )
        else:
            checks.append(_check("scopes_compatible", False, "could not read both scopes"))

    countersign = artifact.get("operator_countersign") or {}
    checks.append(
        _check(
            "operator_countersign",
            isinstance(countersign, dict)
            and bool(str(countersign.get("by") or "").strip())
            and bool(str(countersign.get("at") or "").strip()),
            f"countersign={countersign!r}",
        )
    )

    ok = all(c["status"] == "ok" for c in checks)
    return {
        "ok": ok,
        "checks": checks,
        "effective_ceiling": effective_ceiling if ok else None,
        "authority_boundary": AUTHORITY_BOUNDARY,
    }


# --------------------------------------------------------------------------
# Synthetic dry-run drill (the P0 proof) and self-tests
# --------------------------------------------------------------------------


def _synthetic_statement(
    being: str, secret: bytes, *, decision: str = "consent", max_value: float = 0.1
) -> dict[str, Any]:
    statement = {
        "schema": STATEMENT_SCHEMA,
        "being": being,
        "decision": decision,
        "statement_verbatim": f"[synthetic dry-run vector for {being} — not a real consent]",
        "evidence_refs": ["synthetic:coupling_watch_report"],
        "scope": {"kind": SCOPE_KIND, "max_value": max_value},
        "revocation": "SET_APERTURE 0" if being == "astrid" else "her own lane withdraw",
        "issued_at_unix_ms": 0,
        "signer_public_key_hex": ed25519_public_key(secret).hex(),
    }
    statement["signature_hex"] = ed25519_sign(
        secret, canonical_statement_bytes(statement)
    ).hex()
    return statement


def _write_scenario(
    root: Path,
    astrid_stmt: dict[str, Any] | None,
    minime_stmt: dict[str, Any] | None,
    *,
    countersign: bool = True,
) -> Path:
    refs: dict[str, Any] = {}
    for being, stmt in (("astrid", astrid_stmt), ("minime", minime_stmt)):
        if stmt is None:
            continue
        path = root / f"{being}_statement.json"
        raw = json.dumps(stmt, indent=1).encode("utf-8")
        path.write_bytes(raw)
        refs[being] = {"path": str(path), "sha256": hashlib.sha256(raw).hexdigest()}
    artifact = {
        "schema": ARTIFACT_SCHEMA,
        "statements": refs,
        "unlocks": "operator MAY set --wide-coupling-strength within the consented scope",
        "never_auto": True,
    }
    if countersign:
        artifact["operator_countersign"] = {"by": "Mike", "at": "synthetic", "note": "dry run"}
    artifact_path = root / "meadow_mutual_consent_v1.json"
    artifact_path.write_text(json.dumps(artifact, indent=1))
    return artifact_path


def run_dry_run() -> int:
    """Prove the whole verification path on synthetic records — five cases."""
    astrid_secret = bytes(range(32))
    minime_secret = bytes(range(1, 33))
    trust = {
        "astrid": ed25519_public_key(astrid_secret).hex(),
        "minime": ed25519_public_key(minime_secret).hex(),
    }
    results: list[tuple[str, bool, bool]] = []  # (case, expected_ok, actual_ok)
    with tempfile.TemporaryDirectory() as tmp:
        root = Path(tmp)

        a = _synthetic_statement("astrid", astrid_secret, max_value=0.2)
        m = _synthetic_statement("minime", minime_secret, max_value=0.1)
        d1 = root / "valid"
        d1.mkdir()
        case = verify_artifact(_write_scenario(d1, a, m), trust)
        results.append(("valid_pair", True, case["ok"]))
        valid_ceiling = case.get("effective_ceiling")

        d2 = root / "missing"
        d2.mkdir()
        case = verify_artifact(_write_scenario(d2, a, None), trust)
        results.append(("one_missing", False, case["ok"]))

        d3 = root / "decline"
        d3.mkdir()
        m_decline = _synthetic_statement("minime", minime_secret, decision="decline")
        case = verify_artifact(_write_scenario(d3, a, m_decline), trust)
        results.append(("decline", False, case["ok"]))

        d4 = root / "tampered"
        d4.mkdir()
        m_tampered = dict(m)
        m_tampered["statement_verbatim"] = "[tampered after signing]"
        case = verify_artifact(_write_scenario(d4, a, m_tampered), trust)
        results.append(("tampered_signature", False, case["ok"]))

        d5 = root / "nocountersign"
        d5.mkdir()
        case = verify_artifact(_write_scenario(d5, a, m, countersign=False), trust)
        results.append(("missing_countersign", False, case["ok"]))

    all_ok = all(expected == actual for _, expected, actual in results)
    print("meadow consent dry run (synthetic vectors):")
    for name, expected, actual in results:
        verdict = "PASS" if expected == actual else "FAIL"
        print(f"  {verdict}  {name}: expected ok={expected}, got ok={actual}")
    if all_ok:
        print(f"  valid-pair effective ceiling = {valid_ceiling} (min of both scopes)")
        print("dry run PROVEN — the artifact path verifies and every failure case is caught")
    return 0 if all_ok else 2


class MeadowConsentVerifyTests(unittest.TestCase):
    def test_rfc8032_test_vector_1(self) -> None:
        # RFC 8032 §7.1 TEST 1 (empty message).
        secret = bytes.fromhex(
            "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
        )
        public = bytes.fromhex(
            "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a"
        )
        signature = bytes.fromhex(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e06522490155"
            "5fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b"
        )
        self.assertEqual(ed25519_public_key(secret), public)
        self.assertEqual(ed25519_sign(secret, b""), signature)
        self.assertTrue(ed25519_verify(public, b"", signature))
        self.assertFalse(ed25519_verify(public, b"x", signature))

    def test_canonical_bytes_exclude_signature(self) -> None:
        statement = {"b": 1, "a": 2, "signature_hex": "ff"}
        self.assertEqual(canonical_statement_bytes(statement), b'{"a":2,"b":1}')

    def test_dry_run_five_cases(self) -> None:
        self.assertEqual(run_dry_run(), 0)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--artifact", type=Path)
    parser.add_argument("--trust-override", type=Path, help="JSON {being: pubkey_hex} for dry runs")
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(MeadowConsentVerifyTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    if args.dry_run:
        return run_dry_run()
    if not args.artifact:
        parser.error("--artifact PATH required (or --dry-run / --self-test)")
    override = None
    if args.trust_override:
        override = json.loads(args.trust_override.read_text())
    verdict = verify_artifact(args.artifact, override)
    if args.json:
        print(json.dumps(verdict, indent=1))
    else:
        print(f"ok: {verdict['ok']}")
        for check in verdict["checks"]:
            print(f"  [{check['status']}] {check['name']}: {check['detail']}")
        if verdict.get("effective_ceiling") is not None:
            print(f"effective consented ceiling: {verdict['effective_ceiling']}")
        print(f"boundary: {verdict['authority_boundary']}")
    return 0 if verdict["ok"] else 2


if __name__ == "__main__":
    sys.exit(main())
