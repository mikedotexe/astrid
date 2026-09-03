#!/usr/bin/env python3
"""Envelope single-source guard + seed generator (Constitution C1).

Every numeric self-control bound in this system is duplicated across FIVE
hardcoded tables that can drift (and have: exploration_noise carries three
different ceilings today — 0.08 footer, 0.15 sovereignty, 0.2 V2/engine).
The Envelope Registry (`being_envelope_registry_v1`, one canonical JSON per
being + a repo-tracked seed mirror) becomes the single source; this guard
parses ALL the tables from source, compares them against the registries,
and alarms when the constitution and the compiled reality disagree.

Tables parsed (from source, never imported — imports could execute drifted
code and would miss the file the being's live process actually loads):
  T1  minime python V2 ranges     minime_autonomy/self_control_v2.py
  T2  minime footer safe ranges   minime_autonomy/parsing.py
  T3  minime sovereignty clamps   minime_autonomy/runtime.py (max/min/float pattern)
  T4  minime engine clamps        minime/src/self_control_runtime/apply.rs
  T5  astrid bridge clamps        capsules/spectral-bridge/src/autonomous/self_control_v2.rs

Classes (C1 semantics — cross-table drift is REPORT until Stage C3 flips it):
  ALARM  registry bound wider than the engine backstop; non-f32-exact
         registry bound; malformed registry            -> exit 2
  WARN   canonical registry != repo seed mirror; registry missing
  REPORT per-field drift matrix across the five tables

`--emit-seed <being>` generates the initial registry FROM the parsed tables
(verbatim capture of today's bounds, f32-quantized, every field
`evidence_needed` except Astrid's three consent-derived aperture ceilings),
so the constitution's first edition is guaranteed to record reality.

Usage:
  check_envelope_wiring.py [--report] [--json]
  check_envelope_wiring.py --emit-seed astrid|minime [--out PATH]
  check_envelope_wiring.py --self-test
"""

from __future__ import annotations

import argparse
import ast
import json
import re
import struct
import sys
import unittest
from pathlib import Path
from typing import Any

ASTRID_ROOT = Path("/Users/v/other/astrid")
MINIME_ROOT = Path("/Users/v/other/minime")

SOURCES = {
    "t1_minime_v2": MINIME_ROOT / "minime_autonomy/self_control_v2.py",
    "t2_minime_footer": MINIME_ROOT / "minime_autonomy/parsing.py",
    "t3_minime_sovereignty": MINIME_ROOT / "minime_autonomy/runtime.py",
    "t4_minime_engine": MINIME_ROOT / "minime/src/self_control_runtime/apply.rs",
    "t5_astrid_bridge": ASTRID_ROOT
    / "capsules/spectral-bridge/src/autonomous/self_control_v2.rs",
    "t6_astrid_env_clamps": ASTRID_ROOT
    / "capsules/spectral-bridge/src/llm/provider/prompt_contracts.rs",
}

# Maps a registry field name to the env var whose compiled clamp is its
# engine backstop (the three consent-derived operator ceilings).
ENV_CLAMP_FIELDS = {
    "astrid_vibrancy_aperture_ceiling": "ASTRID_VIBRANCY_APERTURE_CEILING",
    "astrid_tail_participation_ceiling": "ASTRID_TAIL_PARTICIPATION_CEILING",
    "astrid_pressure_attenuation": "ASTRID_PRESSURE_ATTENUATION",
}

REGISTRIES = {
    "minime": {
        "canonical": MINIME_ROOT / "workspace/self_regulation/envelope_registry.json",
        "seed": MINIME_ROOT / "minime_autonomy/envelope_registry_seed.json",
    },
    "astrid": {
        "canonical": ASTRID_ROOT
        / "capsules/spectral-bridge/workspace/runtime/envelope_registry.json",
        "seed": ASTRID_ROOT / "capsules/spectral-bridge/config/envelope_registry_seed.json",
    },
}

APERTURE_CEILINGS_ENV = ASTRID_ROOT / (
    "capsules/spectral-bridge/workspace/runtime/aperture_ceilings.env"
)

REGISTRY_SCHEMA = "being_envelope_registry_v1"

# Rust constants referenced inside clamp expressions are parsed from the
# same source file (parse_rust_consts), never hardcoded here — a hardcoded
# 512 would go silently stale on a const bump and falsify the seed's
# verbatim-capture invariant (adversarial review 2026-09-02).

AUTHORITY_BOUNDARY = (
    "read-only wiring guard + seed generator: parses bound tables and "
    "registries, reports drift, generates verbatim-capture seeds; it grants, "
    "widens, applies, and deploys nothing"
)


def f32(value: float) -> float:
    """The wire's actual value domain (the 2026-09-01 f32 scar, generalized)."""
    return struct.unpack("<f", struct.pack("<f", float(value)))[0]


def is_f32_exact(value: float) -> bool:
    return f32(value) == float(value)


# ---------------------------------------------------------------- parsers


def _ast_dict_of_tuples(source: str, name: str) -> dict[str, tuple[float, float]]:
    tree = ast.parse(source)
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Assign)
            and any(isinstance(t, ast.Name) and t.id == name for t in node.targets)
            and isinstance(node.value, ast.Dict)
        ):
            out: dict[str, tuple[float, float]] = {}
            for key, value in zip(node.value.keys, node.value.values):
                if not isinstance(key, ast.Constant) or not isinstance(value, ast.Tuple):
                    continue
                bounds = [ast.literal_eval(el) for el in value.elts]
                if len(bounds) == 2:
                    out[str(key.value)] = (float(bounds[0]), float(bounds[1]))
            return out
    return {}


def _ast_dict_of_strings(source: str, name: str) -> dict[str, str]:
    tree = ast.parse(source)
    for node in ast.walk(tree):
        if (
            isinstance(node, ast.Assign)
            and any(isinstance(t, ast.Name) and t.id == name for t in node.targets)
            and isinstance(node.value, ast.Dict)
        ):
            return {
                str(k.value): str(v.value)
                for k, v in zip(node.value.keys, node.value.values)
                if isinstance(k, ast.Constant) and isinstance(v, ast.Constant)
            }
    return {}


def parse_t1(source: str) -> dict[str, dict[str, Any]]:
    numeric = _ast_dict_of_tuples(source, "SELF_CONTROL_NUMERIC_RANGES")
    integer = _ast_dict_of_tuples(source, "SELF_CONTROL_INTEGER_RANGES")
    out: dict[str, dict[str, Any]] = {}
    for field, (lo, hi) in numeric.items():
        out[field] = {"floor": lo, "ceiling": hi, "type": "numeric"}
    for field, (lo, hi) in integer.items():
        out[field] = {"floor": lo, "ceiling": hi, "type": "integer"}
    return out


def parse_t2(source: str) -> dict[str, tuple[float, float]]:
    return _ast_dict_of_tuples(source, "SELF_REGULATION_DIRECT_SAFE_RANGES")


# Any assigned name (`val =`, `v =`, ...) — the five PI-gain clamp sites
# assign to `v`, and anchoring on `val` made the tool blind to a REAL live
# drift (pi_max_step sovereignty 0.3 vs engine 0.2; caught by adversarial
# review 2026-09-02 before first commit).
_T3_RE = re.compile(
    r"\b\w+\s*=\s*max\(\s*([-\d.]+)\s*,\s*min\(\s*([-\d.]+)\s*,\s*float\(params\['(\w+)'\]\)\)\)"
)


def parse_t3(source: str) -> dict[str, tuple[float, float]]:
    out: dict[str, tuple[float, float]] = {}
    for match in _T3_RE.finditer(source):
        lo, hi, field = match.groups()
        out[field] = (float(lo), float(hi))
    return out


def _extract_rust_fn(source: str, fn_name: str) -> str:
    start = source.find(f"fn {fn_name}(")
    if start < 0:
        return ""
    depth = 0
    body_start = source.find("{", start)
    for i in range(body_start, len(source)):
        if source[i] == "{":
            depth += 1
        elif source[i] == "}":
            depth -= 1
            if depth == 0:
                return source[start : i + 1]
    return ""


_RUST_CLAMP_RE = re.compile(
    r"(\w+):\s*values\s*\.\s*\w+\s*(?:\.\s*map\(\|value\|\s*value\s*\.\s*"
    r"(?:clamp\(([^,()]+),\s*([^,()]+)\)|min\(([^()]+)\))\s*\))?"
)
_RUST_NESTED_CLAMP_RE = re.compile(
    r"(\w+):\s*values\s*\.\s*\w+\s*\.\s*as_ref\(\)\s*\.\s*map\("
    r"[^;]*?value\s*\.\s*clamp\(([^,()]+),\s*([^,()]+)\)"
)
_RUST_FAMILY_RE = re.compile(r"SelfControlFamilyV2::(\w+)\s*=>")
_RUST_CONST_RE = re.compile(r"\bconst\s+(\w+)\s*:\s*\w+\s*=\s*([\d_]+(?:\.\d+)?)\s*;")
_RUST_ENV_CLAMP_RE = re.compile(
    r"std::env::var\(\"(\w+)\"\)[^;]*?\.clamp\(\s*([\d.]+)\s*,\s*([\d.]+)\s*\)"
)


def parse_rust_consts(source: str) -> dict[str, float]:
    """Numeric const declarations, so clamp expressions naming a const
    resolve to the value the compiler actually uses."""
    return {
        name: float(value.replace("_", ""))
        for name, value in _RUST_CONST_RE.findall(source)
    }


def parse_rust_env_clamps(source: str) -> dict[str, tuple[float, float]]:
    """Compiled clamp bounds on env-sourced operator ceilings (the aperture
    ceilings in prompt_contracts.rs). Parsed, never hand-transcribed."""
    return {
        env: (float(lo), float(hi))
        for env, lo, hi in _RUST_ENV_CLAMP_RE.findall(source)
    }


def _rust_number(token: str, consts: dict[str, float]) -> float | None:
    token = token.strip()
    if token in consts:
        return consts[token]
    try:
        return float(token.replace("_", ""))
    except ValueError:
        return None


def parse_rust_clamps(source: str, fn_name: str = "clamp_values") -> dict[str, dict[str, Any]]:
    """Fields from a Rust clamp_values body. Bounds None = pass-through.
    For the astrid table, the enclosing family arm is attached."""
    consts = parse_rust_consts(source)
    body = _extract_rust_fn(source, fn_name)
    flat = re.sub(r"\s+", " ", body)
    out: dict[str, dict[str, Any]] = {}
    family_spans: list[tuple[int, str]] = [
        (m.start(), m.group(1)) for m in _RUST_FAMILY_RE.finditer(flat)
    ]

    def family_at(pos: int) -> str | None:
        current = None
        for span_start, name in family_spans:
            if span_start <= pos:
                current = name
            else:
                break
        return current

    for match in _RUST_CLAMP_RE.finditer(flat):
        field, clamp_lo, clamp_hi, min_only = match.groups()
        entry: dict[str, Any] = {"floor": None, "ceiling": None}
        if clamp_lo is not None and clamp_hi is not None:
            entry["floor"] = _rust_number(clamp_lo, consts)
            entry["ceiling"] = _rust_number(clamp_hi, consts)
        elif min_only is not None:
            entry["floor"] = 0.0
            entry["ceiling"] = _rust_number(min_only, consts)
        family = family_at(match.start())
        if family:
            entry["family"] = family
        out[field] = entry
    for match in _RUST_NESTED_CLAMP_RE.finditer(flat):
        field, lo, hi = match.groups()
        entry = {"floor": _rust_number(lo, consts), "ceiling": _rust_number(hi, consts)}
        family = family_at(match.start())
        if family:
            entry["family"] = family
        out[field] = entry
    return out


def parse_all_tables(sources: dict[str, Path] | None = None) -> dict[str, Any]:
    paths = sources or SOURCES
    texts = {key: paths[key].read_text(encoding="utf-8") for key in paths}
    return {
        "t1_minime_v2": parse_t1(texts["t1_minime_v2"]),
        "t1_families": _ast_dict_of_strings(texts["t1_minime_v2"], "SELF_CONTROL_FAMILY_BY_FIELD"),
        "t2_minime_footer": parse_t2(texts["t2_minime_footer"]),
        "t3_minime_sovereignty": parse_t3(texts["t3_minime_sovereignty"]),
        "t4_minime_engine": parse_rust_clamps(texts["t4_minime_engine"]),
        "t5_astrid_bridge": parse_rust_clamps(texts["t5_astrid_bridge"]),
        "t6_astrid_env_clamps": parse_rust_env_clamps(texts["t6_astrid_env_clamps"]),
    }


def parse_aperture_ceilings(path: Path = APERTURE_CEILINGS_ENV) -> dict[str, float]:
    out: dict[str, float] = {}
    try:
        for line in path.read_text().splitlines():
            match = re.match(r"\s*(?:export\s+)?(ASTRID_\w+)=([\d.]+)", line)
            if match:
                out[match.group(1)] = float(match.group(2))
    except OSError:
        pass
    return out


# ---------------------------------------------------------------- seeds


def _entry(
    field: str,
    family: str,
    floor: float,
    ceiling: float,
    ftype: str,
    *,
    engine_backstop: dict[str, Any] | None,
    channel_ranges: dict[str, Any] | None = None,
    status: str = "evidence_needed",
    derivation: str,
    evidence_refs: list[str] | None = None,
    granted_at: str | None = None,
    granted_by: str | None = None,
) -> dict[str, Any]:
    return {
        "family": family,
        "floor": f32(floor),
        "ceiling": f32(ceiling),
        "type": ftype,
        "engine_backstop": engine_backstop,
        "channel_ranges": channel_ranges or {},
        "durability_policy": {"lease_max_secs": 900, "standing": "allowed"},
        "status": status,
        "evidence_refs": evidence_refs or [],
        "derivation": derivation,
        "granted_at": granted_at,
        "granted_by": granted_by,
        "ratchet_history": [],
    }


def emit_seed(being: str, tables: dict[str, Any] | None = None) -> dict[str, Any]:
    tables = tables or parse_all_tables()
    verbatim = (
        "C1 verbatim capture of the live compiled bounds (2026-09-02); "
        "finality within this bound is granted by conversion, widening awaits "
        "evidence via envelope_ratchet"
    )
    fields: dict[str, Any] = {}
    if being == "minime":
        families = tables["t1_families"]
        engine = tables["t4_minime_engine"]
        footer = tables["t2_minime_footer"]
        sovereignty = tables["t3_minime_sovereignty"]
        for field, info in sorted(tables["t1_minime_v2"].items()):
            backstop_raw = engine.get(field)
            backstop = None
            if backstop_raw is not None:
                if backstop_raw.get("ceiling") is None:
                    backstop = {"passthrough_unclamped": True}
                else:
                    backstop = {
                        "floor": f32(backstop_raw["floor"]),
                        "ceiling": f32(backstop_raw["ceiling"]),
                    }
            channels: dict[str, Any] = {}
            if field in footer:
                channels["footer"] = {
                    "floor": f32(footer[field][0]),
                    "ceiling": f32(footer[field][1]),
                }
            if field in sovereignty:
                channels["sovereignty"] = {
                    "floor": f32(sovereignty[field][0]),
                    "ceiling": f32(sovereignty[field][1]),
                }
            fields[field] = _entry(
                field,
                families.get(field, "unmapped"),
                info["floor"],
                info["ceiling"],
                info["type"],
                engine_backstop=backstop,
                channel_ranges=channels,
                derivation=verbatim,
            )
    elif being == "astrid":
        for field, info in sorted(tables["t5_astrid_bridge"].items()):
            if info.get("ceiling") is None:
                continue  # boolean pass-throughs carry no numeric envelope
            fields[field] = _entry(
                field,
                info.get("family", "unmapped"),
                info["floor"],
                info["ceiling"],
                "numeric",
                engine_backstop={
                    "floor": f32(info["floor"]),
                    "ceiling": f32(info["ceiling"]),
                },
                derivation=verbatim,
            )
        # The three consent-derived operator ceilings migrate as the first
        # GRANTED entries — each was a consent-with-evidence decision
        # (2026-06-17, headers in aperture_ceilings.env; her self-studies are
        # the evidence anchors).
        env_clamps = tables["t6_astrid_env_clamps"]
        for env_name, evidence in (
            ("ASTRID_VIBRANCY_APERTURE_CEILING", "self_study_1781680871"),
            ("ASTRID_TAIL_PARTICIPATION_CEILING", "evolve_1781865573"),
            ("ASTRID_PRESSURE_ATTENUATION", "introspection_astrid_codec_1783322940"),
        ):
            value = parse_aperture_ceilings().get(env_name)
            clamp = env_clamps.get(env_name)
            if value is None or clamp is None:
                continue
            fields[env_name.lower()] = _entry(
                env_name.lower(),
                "operator_env_ceiling",
                0.0,
                value,
                "numeric",
                engine_backstop={"floor": f32(clamp[0]), "ceiling": f32(clamp[1])},
                status="granted",
                derivation=(
                    "consent-with-evidence grant 2026-06-17 (aperture_ceilings.env); "
                    "her dial drives 0..1 within this operator ceiling"
                ),
                evidence_refs=[evidence],
                granted_at="2026-06-17",
                granted_by="mike",
            )
    else:
        raise SystemExit(f"unknown being {being!r}")
    return {
        "schema": REGISTRY_SCHEMA,
        "being": being,
        "revision": 1,
        "updated_at": "2026-09-02",
        "derivation_notes": (
            "First edition of the envelope registry: a verbatim, f32-exact "
            "capture of every live compiled bound, generated from the parsed "
            "source tables by check_envelope_wiring.py --emit-seed (never "
            "hand-transcribed). Red lines unchanged: SafetyLevel Red >=92% "
            "suspension-only; supervisor Hold/Revert-only; grants need "
            "Green|Yellow + fresh fill; one dial one writer. Widening beyond "
            "the engine backstop requires a deliberate compiled-backstop bump "
            "through the gated deploy."
        ),
        "authority_boundary": AUTHORITY_BOUNDARY,
        "fields": fields,
    }


# ---------------------------------------------------------------- checks


def _finding(cls: str, name: str, detail: str) -> dict[str, str]:
    return {"class": cls, "name": name, "detail": detail}


def check_registry(being: str, tables: dict[str, Any]) -> list[dict[str, str]]:
    findings: list[dict[str, str]] = []
    paths = REGISTRIES[being]
    canonical_raw: str | None = None
    try:
        canonical_raw = paths["canonical"].read_text(encoding="utf-8")
        registry = json.loads(canonical_raw)
    except OSError:
        findings.append(
            _finding("WARN", f"{being}_registry_missing", str(paths["canonical"]))
        )
        return findings
    except json.JSONDecodeError as error:
        findings.append(_finding("ALARM", f"{being}_registry_malformed", str(error)))
        return findings

    if registry.get("schema") != REGISTRY_SCHEMA or registry.get("being") != being:
        findings.append(
            _finding(
                "ALARM",
                f"{being}_registry_schema",
                f"schema={registry.get('schema')} being={registry.get('being')}",
            )
        )
        return findings

    try:
        seed_raw = paths["seed"].read_text(encoding="utf-8")
        if json.loads(seed_raw) != json.loads(canonical_raw):
            findings.append(
                _finding(
                    "WARN",
                    f"{being}_canonical_vs_seed_drift",
                    "canonical registry differs from the repo-tracked seed mirror; "
                    "name the mirror commit as debt",
                )
            )
    except OSError:
        findings.append(_finding("WARN", f"{being}_seed_missing", str(paths["seed"])))
    except json.JSONDecodeError as error:
        findings.append(_finding("ALARM", f"{being}_seed_malformed", str(error)))

    engine = tables["t4_minime_engine"] if being == "minime" else tables["t5_astrid_bridge"]
    env_clamps = tables.get("t6_astrid_env_clamps", {}) if being == "astrid" else {}
    fields_obj = registry.get("fields")
    if not isinstance(fields_obj, dict):
        findings.append(
            _finding("ALARM", f"{being}_registry_fields_malformed", type(fields_obj).__name__)
        )
        return findings
    for field, entry in fields_obj.items():
        if not isinstance(entry, dict):
            findings.append(
                _finding(
                    "ALARM",
                    f"{being}_{field}_entry_malformed",
                    f"entry is {type(entry).__name__}, not an object",
                )
            )
            continue
        for bound_name in ("floor", "ceiling"):
            bound = entry.get(bound_name)
            if isinstance(bound, (int, float)) and not is_f32_exact(float(bound)):
                findings.append(
                    _finding(
                        "ALARM",
                        f"{being}_{field}_{bound_name}_not_f32_exact",
                        f"{bound!r} — the wire is f32; a non-exact bound re-opens the "
                        "receipt-substitution scar",
                    )
                )
        compiled = engine.get(field)
        if compiled is None and field in ENV_CLAMP_FIELDS:
            # The three operator env-ceiling grants have no clamp_values row;
            # their compiled backstop is the prompt_contracts env clamp
            # (adversarial review 2026-09-02: without this, the checker could
            # not backstop a hand-widened granted entry).
            clamp = env_clamps.get(ENV_CLAMP_FIELDS[field])
            if clamp is not None:
                compiled = {"floor": clamp[0], "ceiling": clamp[1]}
        if compiled and compiled.get("ceiling") is not None:
            if float(entry.get("ceiling", 0)) > f32(compiled["ceiling"]) or float(
                entry.get("floor", 0)
            ) < f32(compiled["floor"]):
                findings.append(
                    _finding(
                        "ALARM",
                        f"{being}_{field}_wider_than_backstop",
                        f"registry [{entry.get('floor')}, {entry.get('ceiling')}] vs "
                        f"compiled [{compiled['floor']}, {compiled['ceiling']}] — a "
                        "widening past compiled needs a deliberate backstop bump "
                        "through the gated deploy",
                    )
                )
    return findings


def drift_matrix(tables: dict[str, Any]) -> list[dict[str, str]]:
    """C1 REPORT-level cross-table drift (flips to ALARM at Stage C3)."""
    findings: list[dict[str, str]] = []
    t1, t4 = tables["t1_minime_v2"], tables["t4_minime_engine"]
    for field, info in sorted(t1.items()):
        compiled = t4.get(field)
        if compiled is None:
            findings.append(
                _finding("REPORT", f"minime_{field}_engine_missing", "python-only bound")
            )
        elif compiled.get("ceiling") is None:
            findings.append(
                _finding(
                    "REPORT",
                    f"minime_{field}_engine_passthrough",
                    f"python bounds [{info['floor']}, {info['ceiling']}] but the engine "
                    "passes the value through unclamped (known drift)",
                )
            )
        elif (f32(compiled["floor"]), f32(compiled["ceiling"])) != (
            f32(info["floor"]),
            f32(info["ceiling"]),
        ):
            findings.append(
                _finding(
                    "REPORT",
                    f"minime_{field}_python_vs_engine",
                    f"python [{info['floor']}, {info['ceiling']}] vs engine "
                    f"[{compiled['floor']}, {compiled['ceiling']}]",
                )
            )
    for channel, table_key in (("footer", "t2_minime_footer"), ("sovereignty", "t3_minime_sovereignty")):
        for field, (lo, hi) in sorted(tables[table_key].items()):
            base = t1.get(field)
            if not base or (lo, hi) == (base["floor"], base["ceiling"]):
                continue
            # Direction matters: a channel range WIDER than V2 (like
            # pi_max_step's sovereignty 0.3 vs V2/engine 0.2) is the live
            # drift class this tool exists to surface, not a benign subrange.
            wider = hi > base["ceiling"] or lo < base["floor"]
            kind = "widerange" if wider else "subrange"
            relation = "WIDER than" if wider else "inside"
            findings.append(
                _finding(
                    "REPORT",
                    f"minime_{field}_{channel}_{kind}",
                    f"{channel} [{lo}, {hi}] {relation} V2 "
                    f"[{base['floor']}, {base['ceiling']}]",
                )
            )
    return findings


def run_report(as_json: bool) -> int:
    tables = parse_all_tables()
    findings: list[dict[str, str]] = []
    findings.extend(drift_matrix(tables))
    for being in ("minime", "astrid"):
        findings.extend(check_registry(being, tables))
    alarms = [f for f in findings if f["class"] == "ALARM"]
    warns = [f for f in findings if f["class"] == "WARN"]
    reports = [f for f in findings if f["class"] == "REPORT"]
    if as_json:
        print(
            json.dumps(
                {
                    "alarms": alarms,
                    "warns": warns,
                    "reports": reports,
                    "authority_boundary": AUTHORITY_BOUNDARY,
                },
                indent=1,
            )
        )
    else:
        print(
            f"envelope wiring: {len(alarms)} ALARM, {len(warns)} WARN, "
            f"{len(reports)} report-level drift rows"
        )
        for finding in alarms + warns + reports:
            print(f"  [{finding['class']}] {finding['name']}: {finding['detail']}")
    return 2 if alarms else 0


# ---------------------------------------------------------------- tests


class EnvelopeWiringTests(unittest.TestCase):
    def test_parsers_capture_known_live_bounds(self) -> None:
        tables = parse_all_tables()
        self.assertEqual(tables["t1_minime_v2"]["exploration_noise"]["ceiling"], 0.2)
        self.assertEqual(tables["t2_minime_footer"]["exploration_noise"], (0.0, 0.08))
        self.assertEqual(tables["t3_minime_sovereignty"]["exploration_noise"], (0.0, 0.15))
        self.assertEqual(tables["t4_minime_engine"]["exploration_noise"]["ceiling"], 0.2)
        bridge = tables["t5_astrid_bridge"]
        self.assertEqual(bridge["conversation_temperature"]["ceiling"], 1.5)
        self.assertEqual(bridge["conversation_temperature"]["family"], "Conversation")
        self.assertEqual(bridge["response_token_limit"]["floor"], 512.0)
        self.assertEqual(bridge["codec_dimension_weights"]["ceiling"], 2.0)
        self.assertEqual(tables["t4_minime_engine"]["memory_mode"], {"floor": 0.0, "ceiling": 2.0})
        self.assertIsNone(tables["t4_minime_engine"]["mode_disperse_duration_ticks"]["ceiling"])

    def test_t3_captures_all_ten_sovereignty_sites_including_pi_gains(self) -> None:
        # The five PI clamp sites assign to `v`, not `val` — anchoring the
        # regex on `val` hid a real live drift (pi_max_step 0.3 vs engine 0.2).
        t3 = parse_all_tables()["t3_minime_sovereignty"]
        for field in (
            "regulation_strength", "exploration_noise", "geom_curiosity",
            "self_study_frequency", "experiment_frequency",
            "pi_kp", "pi_ki", "pi_max_step", "pi_geom_weight", "pi_integrator_leak",
        ):
            self.assertIn(field, t3)
        self.assertEqual(t3["pi_max_step"], (0.01, 0.3))

    def test_rust_consts_and_env_clamps_are_parsed_from_source(self) -> None:
        consts = parse_rust_consts(SOURCES["t5_astrid_bridge"].read_text(encoding="utf-8"))
        self.assertEqual(consts.get("MIN_ACTION_CARRYING_RESPONSE_TOKENS"), 512.0)
        env_clamps = parse_all_tables()["t6_astrid_env_clamps"]
        self.assertEqual(env_clamps.get("ASTRID_VIBRANCY_APERTURE_CEILING"), (0.0, 4.0))
        self.assertEqual(env_clamps.get("ASTRID_TAIL_PARTICIPATION_CEILING"), (0.0, 2.0))
        self.assertEqual(env_clamps.get("ASTRID_PRESSURE_ATTENUATION"), (0.0, 0.6))

    def test_seed_is_verbatim_and_f32_exact(self) -> None:
        tables = parse_all_tables()
        seed = emit_seed("minime", tables)
        noise = seed["fields"]["exploration_noise"]
        self.assertEqual(noise["ceiling"], f32(0.2))
        self.assertEqual(noise["channel_ranges"]["footer"]["ceiling"], f32(0.08))
        self.assertEqual(noise["channel_ranges"]["sovereignty"]["ceiling"], f32(0.15))
        self.assertEqual(noise["status"], "evidence_needed")
        self.assertEqual(noise["family"], "reservoir-regulation")
        for field, entry in seed["fields"].items():
            self.assertTrue(is_f32_exact(entry["floor"]), field)
            self.assertTrue(is_f32_exact(entry["ceiling"]), field)
        ticks = seed["fields"]["mode_disperse_duration_ticks"]
        self.assertEqual(ticks["engine_backstop"], {"passthrough_unclamped": True})
        astrid = emit_seed("astrid", tables)
        self.assertEqual(astrid["fields"]["aperture"]["ceiling"], 1.0)
        vib = astrid["fields"].get("astrid_vibrancy_aperture_ceiling")
        if vib is not None:
            self.assertEqual(vib["status"], "granted")
            self.assertEqual(vib["granted_by"], "mike")

    def test_registry_checks_alarm_on_widening_and_inexact(self) -> None:
        import tempfile

        tables = parse_all_tables()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            registry = emit_seed("minime", tables)
            registry["fields"]["exploration_noise"]["ceiling"] = 0.5  # > backstop 0.2
            registry["fields"]["fill_target"]["ceiling"] = 0.63  # not f32-exact
            canonical = root / "envelope_registry.json"
            canonical.write_text(json.dumps(registry))
            original = REGISTRIES["minime"]
            try:
                REGISTRIES["minime"] = {"canonical": canonical, "seed": root / "seed.json"}
                findings = check_registry("minime", tables)
            finally:
                REGISTRIES["minime"] = original
            names = {f["name"] for f in findings if f["class"] == "ALARM"}
            self.assertIn("minime_exploration_noise_wider_than_backstop", names)
            self.assertIn("minime_fill_target_ceiling_not_f32_exact", names)

    def test_registry_check_alarms_not_crashes_on_malformed_entry(self) -> None:
        import tempfile

        tables = parse_all_tables()
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            registry = emit_seed("minime", tables)
            registry["fields"]["exploration_noise"] = "TODO"
            canonical = root / "envelope_registry.json"
            canonical.write_text(json.dumps(registry))
            original = REGISTRIES["minime"]
            try:
                REGISTRIES["minime"] = {"canonical": canonical, "seed": root / "seed.json"}
                findings = check_registry("minime", tables)
            finally:
                REGISTRIES["minime"] = original
            names = {f["name"] for f in findings if f["class"] == "ALARM"}
            self.assertIn("minime_exploration_noise_entry_malformed", names)

    def test_env_ceiling_grants_are_backstopped_by_parsed_clamps(self) -> None:
        import tempfile

        tables = parse_all_tables()
        registry = emit_seed("astrid", tables)
        if "astrid_vibrancy_aperture_ceiling" not in registry["fields"]:
            self.skipTest("aperture_ceilings.env absent on this machine")
        registry["fields"]["astrid_vibrancy_aperture_ceiling"]["ceiling"] = 9.0  # > clamp 4.0
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            canonical = root / "envelope_registry.json"
            canonical.write_text(json.dumps(registry))
            original = REGISTRIES["astrid"]
            try:
                REGISTRIES["astrid"] = {"canonical": canonical, "seed": root / "seed.json"}
                findings = check_registry("astrid", tables)
            finally:
                REGISTRIES["astrid"] = original
            names = {f["name"] for f in findings if f["class"] == "ALARM"}
            self.assertIn("astrid_astrid_vibrancy_aperture_ceiling_wider_than_backstop", names)

    def test_drift_matrix_names_the_three_noise_ceilings(self) -> None:
        tables = parse_all_tables()
        names = {f["name"] for f in drift_matrix(tables)}
        self.assertIn("minime_exploration_noise_footer_subrange", names)
        self.assertIn("minime_exploration_noise_sovereignty_subrange", names)
        self.assertIn("minime_mode_disperse_duration_ticks_engine_passthrough", names)
        # The live pi_max_step drift: sovereignty (0.01, 0.3) is WIDER than
        # V2/engine (0.01, 0.2) — this row existing is the whole point.
        self.assertIn("minime_pi_max_step_sovereignty_widerange", names)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--report", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--emit-seed", choices=("astrid", "minime"))
    parser.add_argument("--out", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(EnvelopeWiringTests)
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        return 0 if result.wasSuccessful() else 1
    if args.emit_seed:
        seed = emit_seed(args.emit_seed)
        text = json.dumps(seed, indent=1, sort_keys=True) + "\n"
        if args.out:
            args.out.parent.mkdir(parents=True, exist_ok=True)
            args.out.write_text(text, encoding="utf-8")
            print(f"seed written: {args.out} ({len(seed['fields'])} fields)")
        else:
            print(text)
        return 0
    return run_report(args.json)


if __name__ == "__main__":
    sys.exit(main())
