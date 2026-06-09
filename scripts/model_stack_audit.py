#!/usr/bin/env python3
"""Audit the local Astrid/Minime model stack.

The local stack has enough moving pieces that prose gets stale quickly. This
script favors runtime/config truth over docs: source defaults, LaunchAgent
arguments, listening ports, Ollama's loaded/installed model lists, and a small
stale-reference scan for common old claims.
"""

from __future__ import annotations

import argparse
import json
import os
import plistlib
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Any


ASTRID_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_MINIME_ROOT = ASTRID_ROOT.parent / "minime"
DEFAULT_RESERVOIR_ROOT = ASTRID_ROOT.parent / "neural-triple-reservoir"
LAUNCH_AGENT = Path.home() / "Library/LaunchAgents/com.reservoir.coupled-astrid.plist"


@dataclass
class Fact:
    role: str
    value: str
    source: str


def run(cmd: list[str], timeout: float = 5.0) -> tuple[int, str]:
    try:
        proc = subprocess.run(
            cmd,
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            timeout=timeout,
        )
        return proc.returncode, proc.stdout.strip()
    except (FileNotFoundError, subprocess.TimeoutExpired) as exc:
        return 127, str(exc)


def read_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except OSError:
        return ""


def first_match(path: Path, pattern: str, flags: int = 0) -> str | None:
    match = re.search(pattern, read_text(path), flags)
    return match.group(1) if match else None


def http_json(url: str, timeout: float = 3.0) -> Any | None:
    try:
        with urllib.request.urlopen(url, timeout=timeout) as response:
            return json.loads(response.read().decode("utf-8"))
    except (OSError, urllib.error.URLError, json.JSONDecodeError):
        return None


def models_url_for_chat_endpoint(url: str) -> str:
    if url.endswith("/v1/chat/completions"):
        return f"{url.removesuffix('/chat/completions')}/models"
    if url.endswith("/v1"):
        return f"{url}/models"
    return url


def endpoint_port(url: str) -> int | None:
    parsed = urllib.parse.urlparse(url)
    if parsed.port is not None:
        return parsed.port
    if parsed.scheme == "http":
        return 80
    if parsed.scheme == "https":
        return 443
    return None


def listener_state(port: int) -> dict[str, Any]:
    state: dict[str, Any] = {}
    code, pids = run(["lsof", f"-tiTCP:{port}", "-sTCP:LISTEN"])
    state["pids"] = pids.splitlines() if code == 0 and pids else []
    if state["pids"]:
        _code, ps = run(
            [
                "ps",
                "-ww",
                "-p",
                ",".join(state["pids"]),
                "-o",
                "pid,ppid,etime,command",
            ]
        )
        state["process"] = ps
    return state


def plist_model(path: Path) -> str | None:
    try:
        with path.open("rb") as fh:
            data = plistlib.load(fh)
    except OSError:
        return None
    args = list(data.get("ProgramArguments") or [])
    for idx, arg in enumerate(args):
        if arg == "--model" and idx + 1 < len(args):
            return str(args[idx + 1])
    return None


def collect_config_facts(minime_root: Path, reservoir_root: Path) -> list[Fact]:
    facts: list[Fact] = []

    minime_agent = minime_root / "autonomous_agent.py"
    minime_config = minime_root / "mikemind/config.py"
    minime_start = minime_root / "scripts/start.sh"
    bridge_llm = ASTRID_ROOT / "capsules/spectral-bridge/src/llm.rs"
    reflective = ASTRID_ROOT / "capsules/spectral-bridge/src/reflective.rs"
    reservoir_server = reservoir_root / "coupled_astrid_server.py"

    add = facts.append
    if value := plist_model(LAUNCH_AGENT):
        add(Fact("Astrid live coupled launchd model", value, str(LAUNCH_AGENT)))
    if value := first_match(reservoir_server, r'--model", default="([^"]+)"'):
        add(Fact("Astrid coupled server code default", value, str(reservoir_server)))
    if value := first_match(bridge_llm, r'const DEFAULT_MLX_URL: &str = "([^"]+)"'):
        add(Fact("Astrid bridge MLX endpoint default", value, str(bridge_llm)))
        add(Fact("Astrid bridge MLX endpoint env override", "ASTRID_BRIDGE_MLX_URL", str(bridge_llm)))
    if value := first_match(bridge_llm, r'const DEFAULT_MLX_PROFILE: &str = "([^"]+)"'):
        add(Fact("Astrid bridge MLX profile default", value, str(bridge_llm)))
        add(
            Fact(
                "Astrid bridge MLX profile env override",
                "ASTRID_BRIDGE_MLX_PROFILE",
                str(bridge_llm),
            )
        )
    if value := first_match(bridge_llm, r'const DEFAULT_OLLAMA_URL: &str = "([^"]+)"'):
        add(Fact("Astrid bridge Ollama endpoint default", value, str(bridge_llm)))
        add(Fact("Astrid bridge Ollama endpoint env override", "ASTRID_BRIDGE_OLLAMA_URL", str(bridge_llm)))
    if value := first_match(bridge_llm, r'const DEFAULT_OLLAMA_FALLBACK_MODEL: &str = "([^"]+)"'):
        add(Fact("Astrid Ollama fallback default", value, str(bridge_llm)))
        add(Fact("Astrid Ollama fallback env override", "ASTRID_OLLAMA_FALLBACK_MODEL", str(bridge_llm)))
    if value := first_match(bridge_llm, r'const EMBED_MODEL: &str = "([^"]+)"'):
        add(Fact("Astrid embedding model", value, str(bridge_llm)))
    if value := first_match(reflective, r'\.arg\("--model-label"\)\s*\.arg\("([^"]+)"\)', re.S):
        add(Fact("Astrid reflective sidecar label", value, str(reflective)))

    if value := first_match(minime_agent, r'MODEL = os\.environ\.get\("MINIME_MODEL", "([^"]+)"\)'):
        add(Fact("Minime autonomous primary", value, str(minime_agent)))
    if value := first_match(minime_agent, r'FALLBACK_MODEL = os\.environ\.get\("MINIME_FALLBACK_MODEL", "([^"]+)"\)'):
        add(Fact("Minime autonomous fast fallback", value, str(minime_agent)))
    if value := first_match(minime_agent, r'LLM_BACKEND = os\.environ\.get\("MINIME_LLM_BACKEND", "([^"]+)"\)'):
        add(Fact("Minime backend preference default", value, str(minime_agent)))
    if value := first_match(minime_config, r'CONVERSATION = "([^"]+)"'):
        add(Fact("Minime interactive conversation", value, str(minime_config)))
    if value := first_match(minime_config, r'LLAVA_VISION = "([^"]+)"'):
        add(Fact("Minime vision model", value, str(minime_config)))
    if value := first_match(minime_config, r'MOONDREAM_VISION = "([^"]+)"'):
        add(Fact("Minime lightweight vision alternative", value, str(minime_config)))
    if value := first_match(minime_config, r'model: str = "([^"]+)"'):
        add(Fact("Minime embedding default", value, str(minime_config)))
    if value := first_match(minime_start, r'MLX_MODEL="\$\{MLX_MODEL:-([^}]+)\}"'):
        add(Fact("Minime optional MLX model", value, str(minime_start)))
    if value := first_match(minime_start, r'ENABLE_MLX_VISION="\$\{ENABLE_MLX_VISION:-([^}]+)\}"'):
        add(Fact("Minime MLX vision enabled by default", value, str(minime_start)))

    return facts


def collect_live_state(candidate: str | None, candidate_mlx_url: str | None) -> dict[str, Any]:
    live: dict[str, Any] = {}
    port_8090 = listener_state(8090)
    live["port_8090_pids"] = port_8090.get("pids", [])
    if port_8090.get("process"):
        live["port_8090_process"] = port_8090["process"]
    live["mlx_models_8090"] = http_json("http://127.0.0.1:8090/v1/models")
    if candidate_mlx_url:
        live["candidate_mlx_url"] = candidate_mlx_url
        live["candidate_mlx_models"] = http_json(models_url_for_chat_endpoint(candidate_mlx_url))
        if port := endpoint_port(candidate_mlx_url):
            candidate_port = listener_state(port)
            live["candidate_mlx_port"] = port
            live["candidate_mlx_port_pids"] = candidate_port.get("pids", [])
            if candidate_port.get("process"):
                live["candidate_mlx_port_process"] = candidate_port["process"]
    live["ollama_loaded"] = http_json("http://127.0.0.1:11434/api/ps")
    live["ollama_tags"] = http_json("http://127.0.0.1:11434/api/tags")
    if candidate:
        code, show = run(["ollama", "show", candidate], timeout=8)
        live["candidate"] = {
            "name": candidate,
            "installed": code == 0,
            "show": show,
        }
    return live


STALE_PATTERNS: dict[str, str] = {
    "legacy Mixtral/Dolphin labels": r"mixtral:8x7b|Dolphin-Mixtral",
    "old Astrid 27B dialogue claims": r"gemma3:27b",
    "old Astrid plain MLX 12B lane": r"mlx_lm\.server.*gemma-3-12b|gemma3:12b on port 8090",
    "old Qwen-current assertions": r"Qwen3-8B is current|currently active.*Qwen|Qwen3\.5-27B.*currently active",
    "old 32D semantic bridge dimensions": r"32D semantic|32-dimensional semantic",
}


def scan_stale(minime_root: Path, include_historical: bool = False) -> dict[str, list[str]]:
    roots = [ASTRID_ROOT, minime_root]
    skip_dirs = {
        ".git",
        ".mypy_cache",
        ".pytest_cache",
        ".venv",
        "logs",
        "target",
        "workspace",
        "__pycache__",
        "node_modules",
        "review-bundles",
        "venv",
    }
    results: dict[str, list[str]] = {label: [] for label in STALE_PATTERNS}
    self_path = Path(__file__).resolve()
    historical_roots = {
        ASTRID_ROOT / "docs/steward-notes",
        minime_root / "docs/steward-notes",
    }
    for root in roots:
        if not root.exists():
            continue
        for dirpath, dirnames, filenames in os.walk(root):
            dirnames[:] = [name for name in dirnames if name not in skip_dirs]
            for filename in filenames:
                path = Path(dirpath) / filename
                if not include_historical and (
                    filename == "CHANGELOG.md" or any(root in path.parents for root in historical_roots)
                ):
                    continue
                if not path.is_file() or path.resolve() == self_path:
                    continue
                if path.suffix.lower() not in {".md", ".py", ".rs", ".sh", ".toml", ".example", ".plist"}:
                    continue
                text = read_text(path)
                if not text:
                    continue
                for label, pattern in STALE_PATTERNS.items():
                    if re.search(pattern, text, re.I):
                        rel = path if path.is_absolute() else path.resolve()
                        results[label].append(str(rel))
    return {key: sorted(set(value)) for key, value in results.items() if value}


def model_names(payload: Any) -> list[str]:
    if not isinstance(payload, dict):
        return []
    return [
        str(item.get("name") or item.get("model") or item.get("id"))
        for item in payload.get("models") or payload.get("data") or []
        if isinstance(item, dict)
    ]


def print_text(
    facts: list[Fact],
    live: dict[str, Any],
    stale: dict[str, list[str]],
    candidate: str | None,
    *,
    stale_skipped: bool = False,
) -> None:
    print("# Local Model Stack Audit")
    print()
    print("## Configured Roles")
    for fact in facts:
        print(f"- {fact.role}: `{fact.value}`")
        print(f"  source: {fact.source}")
    print()
    print("## Live State")
    if live.get("port_8090_process"):
        print("Port 8090 listener:")
        print(live["port_8090_process"])
    else:
        print("- Port 8090 listener: not detected")
    mlx_models = model_names(live.get("mlx_models_8090"))
    print(f"- 8090 /v1/models: {', '.join(mlx_models) if mlx_models else 'unavailable'}")
    if live.get("candidate_mlx_url"):
        candidate_url = live["candidate_mlx_url"]
        candidate_port = live.get("candidate_mlx_port")
        print(f"- Candidate MLX endpoint: {candidate_url}")
        if live.get("candidate_mlx_port_process"):
            print(f"  Port {candidate_port} listener:")
            print(live["candidate_mlx_port_process"])
        elif candidate_port:
            print(f"  Port {candidate_port} listener: not detected")
        candidate_models = model_names(live.get("candidate_mlx_models"))
        print(f"  /v1/models: {', '.join(candidate_models) if candidate_models else 'unavailable'}")
    loaded = model_names(live.get("ollama_loaded"))
    tags = model_names(live.get("ollama_tags"))
    print(f"- Ollama loaded: {', '.join(loaded) if loaded else 'unavailable or empty'}")
    print(f"- Ollama installed tags: {', '.join(tags[:24]) if tags else 'unavailable'}")
    if len(tags) > 24:
        print(f"  (+{len(tags) - 24} more)")
    if candidate:
        data = live.get("candidate", {})
        print()
        print(f"## Candidate: `{candidate}`")
        print(f"- Installed locally: {'yes' if data.get('installed') else 'no'}")
        show = str(data.get("show") or "").strip()
        if show:
            for line in show.splitlines()[:18]:
                print(f"  {line}")
    print()
    print("## Stale-Reference Suspects")
    if stale_skipped:
        print("- Skipped by --no-stale-scan.")
    elif not stale:
        print("- No configured stale patterns found in current source/docs scan.")
    else:
        for label, paths in stale.items():
            print(f"- {label}: {len(paths)} file(s)")
            for path in paths[:8]:
                print(f"  - {path}")
            if len(paths) > 8:
                print(f"  - ... +{len(paths) - 8} more")
    print()
    print("## Model Assessment Checklist")
    print("1. Run this audit before changing defaults.")
    print("2. Install/pull the candidate without changing launchd or repo defaults.")
    print("3. Canary the candidate in the narrowest role first: Minime primary/fallback, Astrid Ollama fallback, or Astrid coupled lane on an alternate port.")
    print("4. Capture latency, tokens/sec, memory/RSS, malformed `NEXT:` rate, leaked model artifacts, fallback count, and stable-core pressure before promoting.")
    print("5. Promote by config or LaunchAgent only after the rollback command/path is written down.")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--minime-root", type=Path, default=DEFAULT_MINIME_ROOT)
    parser.add_argument("--reservoir-root", type=Path, default=DEFAULT_RESERVOIR_ROOT)
    parser.add_argument("--candidate", help="Optional model name to inspect with `ollama show`.")
    parser.add_argument(
        "--candidate-mlx-url",
        help=(
            "Optional OpenAI-compatible Astrid canary endpoint to inspect, "
            "for example http://127.0.0.1:8092/v1/chat/completions."
        ),
    )
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON.")
    parser.add_argument("--no-stale-scan", action="store_true", help="Skip stale-reference pattern scan.")
    parser.add_argument("--include-historical", action="store_true", help="Include changelogs and historical steward notes in the stale scan.")
    args = parser.parse_args()

    facts = collect_config_facts(args.minime_root, args.reservoir_root)
    live = collect_live_state(args.candidate, args.candidate_mlx_url)
    stale = {} if args.no_stale_scan else scan_stale(args.minime_root, include_historical=args.include_historical)

    if args.json:
        print(json.dumps({
            "configured_roles": [asdict(fact) for fact in facts],
            "live_state": live,
            "stale_reference_suspects": stale,
        }, indent=2, sort_keys=True))
    else:
        print_text(facts, live, stale, args.candidate, stale_skipped=args.no_stale_scan)
    return 0


if __name__ == "__main__":
    sys.exit(main())
