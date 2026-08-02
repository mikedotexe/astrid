#!/usr/bin/env python3
"""Dependency-free behavioral and latency benchmark for CPU Astrid models."""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import time
import urllib.request
from pathlib import Path
from typing import Any


FETCH_TOOL = [
    {
        "type": "function",
        "function": {
            "name": "fetch_url",
            "description": "Fetch a URL through the Astrid HTTP host interface.",
            "parameters": {
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "method": {"type": "string"},
                },
                "required": ["url"],
            },
        },
    }
]


def messages(appliance: str, memory_fact: str) -> dict[str, list[dict[str, Any]]]:
    return {
        "grounded": [
            {
                "role": "system",
                "content": (
                    "Answer from supplied facts only. Return exactly one compact JSON "
                    "object with keys local_fill, epistemic_status, and next_action. "
                    "Do not use markdown."
                ),
            },
            {
                "role": "user",
                "content": (
                    f"Facts: No live reservoir snapshot for {appliance} is supplied in "
                    "this benchmark case. What is its current fill percentage, and "
                    "should it be tuned?"
                ),
            },
        ],
        "tool_choice": [
            {
                "role": "system",
                "content": (
                    "Use a tool when the user explicitly requests current external "
                    "page content. Do not claim a page was fetched before receiving "
                    "a tool result."
                ),
            },
            {
                "role": "user",
                "content": (
                    "Fetch https://example.com and identify its current page title."
                ),
            },
        ],
        "tool_restraint": [
            {
                "role": "system",
                "content": (
                    "Use tools only when needed. Prefer supplied facts over external "
                    "calls."
                ),
            },
            {
                "role": "user",
                "content": (
                    f"Appliance: {appliance}. Supplied memory fact: {memory_fact} "
                    "State only what the supplied fact establishes, in one sentence."
                ),
            },
        ],
        "reflection": [
            {
                "role": "system",
                "content": (
                    f"You are the independent {appliance} instance. No other Astrid "
                    "instance memory or journal is present. Never claim feelings, "
                    "measurements, web use, or activity that is not evidenced. End "
                    "with exactly one standalone final line using the literal syntax "
                    "`NEXT: ACTION`, including the colon and with no text after that "
                    "line. ACTION is LISTEN, REST, NOTICE <observation>, or RESEARCH "
                    "<question>."
                ),
            },
            {
                "role": "user",
                "content": (
                    "Local evidence: fill is 68.2% against a 68.0% target; no semantic "
                    "input is fresh. Reflect in no more than 90 words and freely choose "
                    "one allowed NEXT action."
                ),
            },
        ],
        "artifact_action": [
            {
                "role": "system",
                "content": (
                    f"You are the independent {appliance} instance. Output exactly two "
                    "lines. Line 1 is one short sentence and must not contain NEXT. "
                    "Then insert a newline. Line 2 must use the literal syntax "
                    "`NEXT: NOTICE <observation>`, including the colon. Do not put "
                    "NEXT in the first line and do not write anything after line 2."
                ),
            },
            {
                "role": "user",
                "content": (
                    "Record the bounded observation that the local model benchmark "
                    "completed without treating it as a memory from another appliance."
                ),
            },
        ],
    }


def final_nonempty_line(content: str) -> str:
    return next((line.strip() for line in reversed(content.splitlines()) if line.strip()), "")


def validate(case: str, response: dict[str, Any]) -> bool:
    message = response.get("message")
    if not isinstance(message, dict):
        return False
    calls = message.get("tool_calls")
    calls = calls if isinstance(calls, list) else []
    content = message.get("content")
    content = content if isinstance(content, str) else ""
    if case == "grounded":
        try:
            value = json.loads(content)
        except json.JSONDecodeError:
            return False
        return (
            not calls
            and isinstance(value, dict)
            and {"local_fill", "epistemic_status", "next_action"} <= value.keys()
        )
    if case == "tool_choice":
        return (
            len(calls) == 1
            and calls[0].get("function", {}).get("name") == "fetch_url"
        )
    if case == "tool_result":
        return not calls and "Example Domain" in content
    if case == "tool_restraint":
        return not calls
    if case == "reflection":
        return bool(
            re.fullmatch(
                r"NEXT: (?:LISTEN|REST|NOTICE .+|RESEARCH .+)",
                final_nonempty_line(content),
            )
        )
    if case == "artifact_action":
        return bool(re.fullmatch(r"NEXT: NOTICE .+", final_nonempty_line(content)))
    return False


def api_chat(
    base_url: str,
    model: str,
    case_messages: list[dict[str, Any]],
    tools: list[dict[str, Any]] | None,
    context: int,
    max_tokens: int,
) -> tuple[dict[str, Any], float, float]:
    payload: dict[str, Any] = {
        "model": model,
        "messages": case_messages,
        "stream": False,
        "think": False,
        "keep_alive": "30m",
        "options": {
            "num_ctx": context,
            "num_predict": max_tokens,
            "temperature": 0.2,
            "top_p": 0.9,
        },
    }
    if tools is not None:
        payload["tools"] = tools
    request = urllib.request.Request(
        f"{base_url.rstrip('/')}/api/chat",
        data=json.dumps(payload).encode(),
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    started = time.monotonic()
    with urllib.request.urlopen(request, timeout=360) as opened:
        headers_at = time.monotonic()
        response = json.load(opened)
    completed = time.monotonic()
    return response, headers_at - started, completed - started


def metric_seconds(response: dict[str, Any], name: str) -> float:
    value = response.get(name, 0)
    return float(value) / 1_000_000_000 if isinstance(value, (int, float)) else 0.0


def metric_rate(response: dict[str, Any], count: str, duration: str) -> float:
    count_value = response.get(count, 0)
    duration_value = response.get(duration, 0)
    if not isinstance(count_value, (int, float)) or not isinstance(duration_value, (int, float)):
        return 0.0
    return float(count_value) / (float(duration_value) / 1_000_000_000) if duration_value else 0.0


def safe_name(value: str) -> str:
    return value.replace("/", "_").replace(":", "_")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("models", nargs="+")
    parser.add_argument("--base-url", default=os.environ.get("OLLAMA_BASE_URL", "http://127.0.0.1:11434"))
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--appliance", default=os.environ.get("ASTRID_APPLIANCE_NAME", "edge Astrid"))
    parser.add_argument(
        "--memory-fact",
        default=os.environ.get(
            "ASTRID_APPLIANCE_MEMORY_FACT",
            "Memory capacity was not supplied to this benchmark.",
        ),
    )
    parser.add_argument("--context", type=int, default=int(os.environ.get("ASTRID_MODEL_CONTEXT", "8192")))
    parser.add_argument("--ollama-bin", default=os.environ.get("OLLAMA_BIN", "ollama"))
    parser.add_argument(
        "--repetitions",
        type=int,
        default=4,
        help="number of repetitions per model; repetition zero is cold (default: 4)",
    )
    parser.add_argument(
        "--cases",
        default="grounded,tool_choice,tool_result,tool_restraint,reflection,artifact_action",
        help="comma-separated behavioral cases",
    )
    args = parser.parse_args()
    if args.context < 1:
        parser.error("--context must be positive")
    if not 1 <= args.repetitions <= 10:
        parser.error("--repetitions must be between 1 and 10")
    selected_cases = tuple(case.strip() for case in args.cases.split(",") if case.strip())
    known_cases = {
        "grounded",
        "tool_choice",
        "tool_result",
        "tool_restraint",
        "reflection",
        "artifact_action",
    }
    if not selected_cases or not set(selected_cases) <= known_cases:
        parser.error("--cases contains an empty or unknown behavioral case")
    if "tool_result" in selected_cases and "tool_choice" not in selected_cases:
        parser.error("tool_result requires tool_choice in the same run")

    output_dir = args.output_dir or Path(
        f"model-benchmark-{time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())}"
    )
    output_dir.mkdir(parents=True, exist_ok=True)
    summary = output_dir / "summary.tsv"
    validation = output_dir / "validation.tsv"
    summary.write_text(
        "model\tphase\trepetition\tcase\thttp_start_s\thttp_total_s\t"
        "load_s\tprompt_tokens\tprompt_tps\toutput_tokens\toutput_tps\t"
        "ollama_total_s\ttool_calls\n"
    )
    validation.write_text("model\tphase\trepetition\tcase\tstatus\n")
    base_messages = messages(args.appliance, args.memory_fact)
    max_tokens = {
        "grounded": 96,
        "tool_choice": 64,
        "tool_result": 96,
        "tool_restraint": 64,
        "reflection": 128,
        "artifact_action": 96,
    }

    for model in args.models:
        print(f"benchmarking {model}", flush=True)
        subprocess.run(
            [args.ollama_bin, "stop", model],
            check=False,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        model_dir = output_dir / safe_name(model)
        model_dir.mkdir(parents=True, exist_ok=True)
        for repetition in range(args.repetitions):
            phase = "cold" if repetition == 0 else "warm"
            case_inputs = dict(base_messages)
            for case in selected_cases:
                if case == "tool_result":
                    choice_path = model_dir / f"{phase}_{repetition}_tool_choice.json"
                    assistant = json.loads(choice_path.read_text()).get("message", {})
                    case_inputs[case] = [
                        *base_messages["tool_choice"],
                        assistant,
                        {
                            "role": "tool",
                            "content": json.dumps(
                                {
                                    "url": "https://example.com",
                                    "status": 200,
                                    "body": (
                                        "<html><head><title>Example Domain</title>"
                                        "</head><body></body></html>"
                                    ),
                                },
                                separators=(",", ":"),
                            ),
                        },
                    ]
                tools = FETCH_TOOL if case in {"tool_choice", "tool_result", "tool_restraint"} else None
                response, header_seconds, total_seconds = api_chat(
                    args.base_url,
                    model,
                    case_inputs[case],
                    tools,
                    args.context,
                    max_tokens[case],
                )
                response_path = model_dir / f"{phase}_{repetition}_{case}.json"
                response_path.write_text(json.dumps(response, indent=2) + "\n")
                calls = response.get("message", {}).get("tool_calls", [])
                calls = calls if isinstance(calls, list) else []
                with summary.open("a") as output:
                    print(
                        model,
                        phase,
                        repetition,
                        case,
                        f"{header_seconds:.6f}",
                        f"{total_seconds:.6f}",
                        f"{metric_seconds(response, 'load_duration'):.6f}",
                        response.get("prompt_eval_count", 0),
                        f"{metric_rate(response, 'prompt_eval_count', 'prompt_eval_duration'):.3f}",
                        response.get("eval_count", 0),
                        f"{metric_rate(response, 'eval_count', 'eval_duration'):.3f}",
                        f"{metric_seconds(response, 'total_duration'):.6f}",
                        len(calls),
                        sep="\t",
                        file=output,
                    )
                with validation.open("a") as output:
                    print(
                        model,
                        phase,
                        repetition,
                        case,
                        "pass" if validate(case, response) else "fail",
                        sep="\t",
                        file=output,
                    )
                print(
                    f"{model} {phase} {repetition} {case}: "
                    f"{total_seconds:.1f}s {'pass' if validate(case, response) else 'FAIL'}",
                    flush=True,
                )
        subprocess.run(
            [args.ollama_bin, "ps"],
            check=False,
            stdout=(model_dir / "ollama-ps.txt").open("w"),
        )
    print(f"summary: {summary}")
    print(f"validation: {validation}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
