#!/usr/bin/env python3
"""Qualify installed generate_step timing with tiny CPU arrays and no weights."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import importlib
from importlib import metadata
import inspect
import json
import math
import os
from pathlib import Path
import sys


SCHEMA = "astrid_mlx_generator_contract_v1"
HISTORY_MODES = {"0.31.3": "final_prompt_token_only", "0.32.0": "full_prefilled_prompt"}
PROMPT = [1, 2, 3, 4, 5]
PREDICTIONS = [6, 6, 7, 0, 1]
MAX_TOKENS = 4
VOCAB_SIZE = 8
CHECK_KEYS = ("package_history", "processor_order", "feedback_lookahead",
              "bounded_yields", "cache_progression", "close_restores_stream")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def expected_events(history_mode: str, *, early_close: bool) -> list[dict]:
    """Explicit expected protocol, not an alternate token generator."""
    require(history_mode in HISTORY_MODES.values(), "unsupported processor history policy")
    events = [
        {"kind": "model", "tokens": [1, 2], "cache_before": 0, "cache_after": 2},
        {"kind": "model", "tokens": [3, 4], "cache_before": 2, "cache_after": 4},
    ]
    history = [5] if history_mode == "final_prompt_token_only" else list(PROMPT)
    count = 2 if early_close else MAX_TOKENS + 1
    for prediction in range(count):
        incoming = [5] if prediction == 0 else [PREDICTIONS[prediction - 1]]
        if prediction:
            history += incoming
        events.append({"kind": "model", "tokens": incoming,
                       "cache_before": 4 + prediction, "cache_after": 5 + prediction})
        feedback = max(0, prediction - 1)
        events.append({"kind": "processor", "name": "feedback", "prediction": prediction,
                       "tokens": list(history), "feedback": feedback,
                       "selected": PREDICTIONS[prediction]})
        events.append({"kind": "processor", "name": "additional", "prediction": prediction,
                       "tokens": list(history), "feedback": feedback,
                       "selected": PREDICTIONS[prediction]})
        if prediction:
            emitted = prediction - 1
            events.append({"kind": "yield", "index": emitted, "token": PREDICTIONS[emitted],
                           "logprobs_argmax": PREDICTIONS[emitted]})
            events.append({"kind": "feedback", "after_yield": emitted, "value": prediction})
    events.append({"kind": "closed" if early_close else "exhausted"})
    return events


def validate_trace(trace, history_mode: str) -> dict[str, bool]:
    """Pure evidence verifier; imports no MLX and performs no runtime operation."""
    require(isinstance(trace, list) and len(trace) == 2, "two generator cases required")
    for case, name in zip(trace, ("complete", "early_close")):
        require(isinstance(case, dict) and case.get("case") == name, "generator case order changed")
        expected = expected_events(history_mode, early_close=name == "early_close")
        # Canonical JSON comparison distinguishes bools from integer evidence.
        require(json.dumps(case.get("events"), sort_keys=True, allow_nan=False)
                == json.dumps(expected, sort_keys=True), f"{name} generator trace differs from contract")
        before, after = case.get("default_stream_before"), case.get("default_stream_after")
        require(isinstance(before, str) and before and after == before,
                f"{name} did not restore the caller CPU stream")
        require(case.get("generation_stream_unchanged") is True,
                f"{name} replaced the installed generation stream")
    return {key: True for key in CHECK_KEYS}


def run_case(mx, module, *, early_close: bool) -> dict:
    events = []
    case = {"case": "early_close" if early_close else "complete", "events": events,
            "default_stream_before": repr(mx.default_stream(mx.cpu))}
    original_stream = module.generation_stream
    feedback = 0
    predictions = 0

    def cpu_boundary():
        require(mx.default_device().type == mx.cpu, "callback left CPU device")
        require(module.generation_stream is original_stream
                and original_stream.device.type == mx.cpu, "generation stream changed or left CPU")

    class TinyCache:
        def __init__(self):
            self.tokens = []
            self.state = [mx.array([], dtype=mx.int32)]

    owned_cache = TinyCache()

    class TinyModel:
        def __call__(self, tokens, cache=None):
            cpu_boundary()
            require(isinstance(cache, list) and len(cache) == 1 and cache[0] is owned_cache,
                    "installed generator replaced the provided cache")
            require(tokens.ndim == 2 and tokens.shape[0] == 1, "unexpected toy-model input shape")
            incoming = tokens.tolist()[0]
            before = len(owned_cache.tokens)
            owned_cache.tokens += incoming
            owned_cache.state = [mx.array(owned_cache.tokens, dtype=mx.int32)]
            events.append({"kind": "model", "tokens": incoming,
                           "cache_before": before, "cache_after": len(owned_cache.tokens)})
            return mx.zeros((*tokens.shape, VOCAB_SIZE), dtype=mx.float32)

    def feedback_processor(tokens, logits):
        nonlocal predictions
        cpu_boundary()
        selected = (6 + feedback) % VOCAB_SIZE
        events.append({"kind": "processor", "name": "feedback", "prediction": predictions,
                       "tokens": tokens.tolist(), "feedback": feedback, "selected": selected})
        predictions += 1
        return mx.broadcast_to(mx.where(mx.arange(VOCAB_SIZE) == selected, 10.0, -10.0), logits.shape)

    def additional_processor(tokens, logits):
        cpu_boundary()
        events.append({"kind": "processor", "name": "additional", "prediction": predictions - 1,
                       "tokens": tokens.tolist(), "feedback": feedback,
                       "selected": mx.argmax(logits, axis=-1).item()})
        return logits + 0.25

    generator = module.generate_step(mx.array(PROMPT, dtype=mx.int32), TinyModel(),
        max_tokens=MAX_TOKENS, prefill_step_size=2, prompt_cache=[owned_cache],
        logits_processors=[feedback_processor, additional_processor])
    try:
        for index, (token, logprobs) in enumerate(generator):
            cpu_boundary()
            require(index < MAX_TOKENS, "installed generator yielded extra tokens")
            values = logprobs.tolist()
            require(len(values) == VOCAB_SIZE and all(math.isfinite(value) for value in values),
                    "invalid installed generator log probabilities")
            events.append({"kind": "yield", "index": index, "token": token,
                           "logprobs_argmax": mx.argmax(logprobs).item()})
            feedback = index + 1
            events.append({"kind": "feedback", "after_yield": index, "value": feedback})
            if early_close:
                break
        else:
            events.append({"kind": "exhausted"})
    finally:
        generator.close()
    if early_close:
        events.append({"kind": "closed"})
    cpu_boundary()
    case["default_stream_after"] = repr(mx.default_stream(mx.cpu))
    case["generation_stream_unchanged"] = module.generation_stream is original_stream
    return case


def run(output: Path) -> int:
    # An interrupted/failed attempt is evidence, never overwritten by a retry.
    with output.open("x") as stream:
        record = {"schema": SCHEMA, "passed": False, "pid": os.getpid(), "python": sys.executable,
                  "started_at": datetime.now(timezone.utc).isoformat(),
                  "wrapper_sha256": digest(Path(__file__)), "trace": [], "checks": {},
                  "source_hashes": {}, "device": {}, "versions": {}}
        status = 1
        try:
            require(not any(name == "mlx_lm" or name.startswith("mlx_lm.") for name in sys.modules),
                    "mlx_lm was imported before CPU selection; use a fresh process")
            record["versions"] = {name: metadata.version(name) for name in ("mlx", "mlx-metal", "mlx-lm")}
            version = record["versions"]["mlx-lm"]
            require(version in HISTORY_MODES, "unqualified mlx-lm version")
            record["expected_history_mode"] = HISTORY_MODES[version]
            mx = importlib.import_module("mlx.core")
            mx.set_default_device(mx.cpu)
            require(mx.default_device().type == mx.cpu, "could not select CPU before generator import")
            record["cpu_selected_before_generator_import"] = True
            module = importlib.import_module("mlx_lm.generate")
            require(module.generation_stream.device.type == mx.cpu, "installed generation_stream is not CPU")
            require(mx.default_device().type == mx.cpu, "generator import changed default device")
            record["device"] = {"default": "cpu", "generation_stream": "cpu"}
            generate_path = Path(module.__file__).resolve(strict=True)
            require(Path(inspect.getsourcefile(module.generate_step)).resolve() == generate_path,
                    "generate_step is not the installed generator function")
            package = sys.modules["mlx_lm"]
            paths = (generate_path, Path(package.__file__).resolve(strict=True),
                     Path(mx.__file__).resolve(strict=True))
            record["source_hashes"] = {str(path): digest(path) for path in paths}
            for early in (False, True):
                record["trace"].append(run_case(mx, module, early_close=early))
            record["checks"] = validate_trace(record["trace"], record["expected_history_mode"])
            require(all(digest(Path(path)) == sha for path, sha in record["source_hashes"].items()),
                    "installed source changed during qualification")
            record["passed"], status = True, 0
        except BaseException as error:
            record["error"] = f"{type(error).__name__}: {error}"
            status = 130 if isinstance(error, KeyboardInterrupt) else 1
        finally:
            record["completed_at"] = datetime.now(timezone.utc).isoformat()
            json.dump(record, stream, indent=2, sort_keys=True, allow_nan=False)
            stream.write("\n")
            stream.flush()
            os.fsync(stream.fileno())
    return status


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    return run(parser.parse_args().output)


if __name__ == "__main__":
    raise SystemExit(main())
