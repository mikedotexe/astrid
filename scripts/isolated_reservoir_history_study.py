#!/usr/bin/env python3
"""Offline synthetic-history controls using the sibling's actual NumPy ESN.

No service import, live snapshot, LLM, learned readout, network or control call.
The generated histories are fixtures, not Astrid/Minime histories. This tests
whether retained recurrent state affects future states under identical inputs.
"""
import argparse
from dataclasses import asdict
import hashlib
import importlib.util
import json
from pathlib import Path
import sys


def digest(value):
    return hashlib.sha256(json.dumps(value, sort_keys=True, allow_nan=False).encode()).hexdigest()


def deny_network(event, _args):
    if event in {"socket.connect", "socket.bind", "socket.sendto", "socket.getaddrinfo"}:
        raise RuntimeError("network forbidden in isolated history study")


def run_study(reservoir_source, seeds=(7, 17, 29)):
    import numpy as np
    if not 1 <= len(seeds) <= 8 or any(type(s) is not int or not 0 <= s < 2**32 for s in seeds):
        raise ValueError("1..8 uint32 fixture seeds required")
    source = Path(reservoir_source).resolve()
    source_sha256 = hashlib.sha256(source.read_bytes()).hexdigest()
    spec = importlib.util.spec_from_file_location("history_study_reservoir", source)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    if hashlib.sha256(source.read_bytes()).hexdigest() != source_sha256:
        raise ValueError("reservoir source changed during import")

    def array_hash(array):
        a = np.ascontiguousarray(array)
        return hashlib.sha256(str((a.dtype.str, a.shape)).encode() + a.tobytes()).hexdigest()

    def state_hash(state):
        return digest([array_hash(a) for a in state])

    plan = {"schema": "synthetic_history_plan_v1", "seeds": list(seeds), "n_nodes": 32,
            "history_ticks": 12, "future_ticks": 8, "input_dim": 3,
            "source_sha256": source_sha256,
            "runner_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "history_row_a": [.25, -.15, .1], "history_row_b": [-.25, .15, -.1],
            "future_row_a": [.05, .1, -.05], "future_row_b": [-.05, -.1, .05],
            "metric": "mean_future_concatenated_state_l2",
            "prediction": "different histories yield a gap above 1e-6 under a common future",
            "threshold": 1e-6, "identity_tolerance": 1e-7,
            "controls": ["identical_history", "copied_state_reset", "reversed_execution", "different_future_input"],
            "authorship": "developer_fixture_not_being_prediction",
            "comparison": "isolated_synthetic_counterfactual", "live_identity_established": False}
    plan_hash = digest(plan)
    trials = []
    for seed in seeds:
        config = module.ReservoirConfig(input_dim=3, n_nodes=32, seed=seed)
        model = module.TripleReservoir(config)
        weights_before = digest({k: array_hash(v) for k, v in vars(model).items() if isinstance(v, np.ndarray)})
        history_a = np.tile(np.array([plan["history_row_a"]], dtype=np.float32), (plan["history_ticks"], 1))
        history_b = np.tile(np.array([plan["history_row_b"]], dtype=np.float32), (plan["history_ticks"], 1))
        future = np.tile(np.array([plan["future_row_a"]], dtype=np.float32), (plan["future_ticks"], 1))
        other_future = np.tile(np.array([plan["future_row_b"]], dtype=np.float32), (plan["future_ticks"], 1))

        def evolve(initial, inputs):
            state = tuple(a.copy() for a in initial)
            trace = []
            for row in inputs:
                _, _, state = model.step_numpy(row[None], state)
                trace.append(np.concatenate(state, axis=1)[0].copy())
            return state, np.stack(trace)

        def gap(left, right):
            value = float(np.linalg.norm(left - right, axis=1).mean())
            if not np.isfinite(value):
                raise ValueError("nonfinite study metric")
            return value

        zero = model.zero_state()
        state_a, _ = evolve(zero, history_a)
        state_b, _ = evolve(zero, history_b)
        initial_hashes = [state_hash(state_a), state_hash(state_b)]
        _, trace_a = evolve(state_a, future)
        _, trace_b = evolve(state_b, future)
        _, identical = evolve(state_a, future)
        # Replacing the entire recurrent state removes the prior B history in this pure backend.
        reset_b = tuple(a.copy() for a in state_a)
        _, reset_trace = evolve(reset_b, future)
        _, reverse_b = evolve(state_b, future)
        _, reverse_a = evolve(state_a, future)
        _, other_input = evolve(state_a, other_future)
        measures = {"history_gap": gap(trace_a, trace_b), "identical_history_gap": gap(trace_a, identical),
                    "copied_state_reset_gap": gap(trace_a, reset_trace),
                    "execution_order_gap": max(gap(trace_a, reverse_a), gap(trace_b, reverse_b)),
                    "different_future_input_gap": gap(trace_a, other_input)}
        weights_after = digest({k: array_hash(v) for k, v in vars(model).items() if isinstance(v, np.ndarray)})
        originals_unchanged = initial_hashes == [state_hash(state_a), state_hash(state_b)]
        if weights_before != weights_after or not originals_unchanged:
            raise ValueError("runner mutated original state or weights")
        controls_valid = all(measures[k] <= plan["identity_tolerance"] for k in
                             ("identical_history_gap", "copied_state_reset_gap", "execution_order_gap"))
        trials.append({"seed": seed, "config": asdict(config), "weights_sha256": weights_before,
                       "history_sha256": [array_hash(history_a), array_hash(history_b)],
                       "future_sha256": array_hash(future), "initial_state_sha256": initial_hashes,
                       "trajectory_sha256": [array_hash(trace_a), array_hash(trace_b)],
                       "metric_values": measures, "controls_valid": controls_valid,
                       "prediction_outcome": ("insufficient" if not controls_valid else
                                              "matched" if measures["history_gap"] > plan["threshold"] else "not_matched"),
                       "original_states_unchanged": originals_unchanged, "weights_unchanged": True})
    return {"schema": "isolated_reservoir_history_study_v1", "plan": plan, "plan_sha256": plan_hash,
            "trials": trials, "numpy_version": np.__version__, "python_version": sys.version.split()[0],
            "backend": "TripleReservoir.step_numpy", "readout_used": False,
            "source_path": str(source), "source_bytes_unchanged": hashlib.sha256(source.read_bytes()).hexdigest() == plan["source_sha256"],
            "live_state_changed": False, "live_model_parity_established": False,
            "full_service_state_modeled": False, "authority_effect": False,
            "felt_state_conclusion": None, "being_learning_demonstrated": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reservoir-source", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    sys.addaudithook(deny_network)
    result = run_study(args.reservoir_source)
    encoded = json.dumps(result, indent=2, sort_keys=True, allow_nan=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        with args.output.open("x") as out:
            out.write(encoded)
    print(encoded)
    return 0 if result["source_bytes_unchanged"] and all(t["controls_valid"] for t in result["trials"]) else 1


if __name__ == "__main__":
    sys.exit(main())
