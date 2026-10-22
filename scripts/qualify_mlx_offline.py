#!/usr/bin/env python3
"""Run and bind the existing offline reservoir gates to one prepared MLX stage."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import subprocess

from qualify_mlx_packages import (validate_baseline, validate_predecessor_for_work,
                                  validate_stage, validate_release, harness_hashes,
                                  validate_memory_policy)
from prepare_mlx_environments import digest
from astrid_mlx_memory import (Monitor, STRICT_POLICY, STAGE2_DIAGNOSTIC_POLICY,
                              POLICIES)
from stage2_wide_exception import evaluate as evaluate_wide_exception


TEST_MODULES = ["test_coupled_astrid_token_policy", "test_generation_controls",
                "test_generation_completion", "test_coupled_http_gateway", "test_slow_coupling",
                "test_offline_coupling_replay", "test_wide_processor"]


def qualify(args):
    memory_policy = getattr(args, "memory_policy", STRICT_POLICY)
    baseline_hash = validate_baseline(args.baseline_soak, stage=args.stage,
                                      qualification_target=args.qualification_target,
                                      memory_policy=memory_policy)
    preparation = json.loads(args.preparation.read_text())
    predecessor = validate_predecessor_for_work(args, baseline_hash, preparation)
    stage = validate_stage(preparation, args.stage)
    manifest_hash = validate_release(args.manifest, preparation, args.stage)
    selected_argv = None
    if memory_policy == STAGE2_DIAGNOSTIC_POLICY:
        from model_environment import validate_manifest
        selected_argv = validate_manifest(args.manifest)["manifest"]["argv"]
    validate_memory_policy(memory_policy, preparation_hash=digest(args.preparation),
                           baseline_hash=baseline_hash, manifest_hash=manifest_hash,
                           stage=args.stage, qualification_target=args.qualification_target)
    root = Path(stage["source_root"])
    if args.output_dir.exists():
        raise ValueError("preserve prior results; choose a new output directory")
    args.output_dir.mkdir(parents=True, mode=0o700)
    record = {"schema": "mlx_offline_qualification_v1", "stage": args.stage,
              "qualification_target": args.qualification_target,
              "predecessor_acceptance": predecessor,
              "packages": stage["packages"], "python": stage["python"],
              "source_hashes": preparation["source_hashes"], "source_root": str(root),
              "model_snapshot": preparation["model_snapshot"], "baseline_soak_sha256": baseline_hash,
              "manifest_sha256": manifest_hash, "preparation_sha256": digest(args.preparation),
              "memory_policy": memory_policy,
              "selected_argv": selected_argv if memory_policy == STAGE2_DIAGNOSTIC_POLICY else None,
              "harness_hashes": harness_hashes(),
              "started_at": datetime.now(timezone.utc).isoformat(), "passed": False, "tests": []}
    receipt = args.output_dir / "offline.json"
    def save():
        receipt.write_text(json.dumps(record, indent=2, sort_keys=True) + "\n")
    commands = [
        ("contracts", ["-m", "unittest", *TEST_MODULES]),
        ("generator-contract", [str(Path(__file__).with_name("astrid_mlx_generator_contract.py")),
                                "--output", str(args.output_dir / "generator-contract.json")]),
        ("reservoir-equivalence", ["mlx_reservoir.py", "--verify", "--bench", "--n-nodes", "192"]),
        ("multi-headed", ["test_multi_headed.py"]),
        ("wide-coupling", [str(Path(__file__).with_name("astrid_mlx_embedding_gate.py")),
                           "--source-root", str(root), "--model", preparation["model_snapshot"],
                           "--audit-file", str(args.output_dir / "embedding-gate.json")]),
    ]
    save()
    env = dict(os.environ, HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", PYTHONDONTWRITEBYTECODE="1")
    memory = Monitor(policy=memory_policy)
    try:
        memory.start()
        for name, arguments in commands:
            command = [stage["python"], "-B", *arguments]
            log = args.output_dir / f"{name}.log"
            item = {"name": name, "command": command, "log": str(log), "returncode": None}
            record["tests"].append(item)
            save()
            with log.open("x") as output:
                result = subprocess.run(command, cwd=root, env=env, stdout=output,
                                        stderr=subprocess.STDOUT, timeout=1800, check=False)
            item["returncode"] = result.returncode
            item["log_sha256"] = digest(log)
            save()
            print(f"{args.stage}: {name} exit={result.returncode}", flush=True)
            if result.returncode and name == "wide-coupling" and memory_policy == STAGE2_DIAGNOSTIC_POLICY:
                audit_path = args.output_dir / "embedding-gate.json"
                if audit_path.is_file():
                    audit = json.loads(audit_path.read_text())
                    index = (selected_argv.index("--wide-coupling-strength")
                             if selected_argv.count("--wide-coupling-strength") == 1 else -1)
                    disabled = index >= 0 and index + 1 < len(selected_argv) and selected_argv[index + 1] == "0.0"
                    exception = evaluate_wide_exception(
                        log.read_text(), audit, stage=args.stage,
                        qualification_target=args.qualification_target,
                        actual_returncode=result.returncode,
                        wide_coupling_strength=0.0 if disabled else float("nan"),
                        expected_source_root=str(root),
                        expected_source_hashes={key: record["source_hashes"][key]
                                                for key in ("test_wide_coupling_offline.py",
                                                            "coupled_astrid_server.py", "wide_coupling.py")},
                        expected_wrapper_sha256=record["harness_hashes"]["astrid_mlx_embedding_gate.py"],
                        expected_model=record["model_snapshot"])
                    item["wide_timing_exception"] = exception
                    item["embedding_audit"] = {"path": str(audit_path.resolve()),
                                               "sha256": digest(audit_path)}
                    save()
                    if exception["accepted_exception"]:
                        continue
            if result.returncode:
                raise RuntimeError(f"offline gate failed: {name}; inspect {log}")
            if name == "generator-contract":
                from model_qualification import verify_generator_audit
                audit_path = args.output_dir / "generator-contract.json"
                verify_generator_audit(json.loads(audit_path.read_text()), record)
                item["generator_audit"] = {"path": str(audit_path.resolve()), "sha256": digest(audit_path)}
                save()
            if name == "wide-coupling":
                from model_qualification import verify_embedding_audit
                audit_path = args.output_dir / "embedding-gate.json"
                audit = json.loads(audit_path.read_text())
                verify_embedding_audit(audit, record)
                item["embedding_audit"] = {"path": str(audit_path.resolve()), "sha256": digest(audit_path)}
                save()
        validate_release(args.manifest, preparation, args.stage)
        if record["harness_hashes"] != harness_hashes():
            raise ValueError("qualification harness changed during offline tests")
        record["passed"] = True
    except BaseException as error:
        record["error"] = str(error)
        raise
    finally:
        for item in record["tests"]:
            if Path(item["log"]).is_file():
                item["log_sha256"] = digest(Path(item["log"]))
        record["host_memory"] = memory.stop()
        if not record["host_memory"]["automated_ok"]:
            record["passed"] = False
            record["memory_failure_reasons"] = record["host_memory"]["failure_reasons"]
        record["completed_at"] = datetime.now(timezone.utc).isoformat()
        save()
    return 0 if record["passed"] else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--preparation", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--baseline-soak", type=Path, required=True)
    parser.add_argument("--stage", choices=("baseline", "core", "full"), required=True)
    parser.add_argument("--qualification-target", choices=("core", "full"), required=True)
    parser.add_argument("--memory-policy", choices=POLICIES, default=STRICT_POLICY)
    parser.add_argument("--core-manifest", type=Path)
    parser.add_argument("--core-proof", type=Path)
    parser.add_argument("--activation-receipt", type=Path)
    parser.add_argument("--supervised-watch", type=Path)
    parser.add_argument("--supervised-review", type=Path)
    parser.add_argument("--passive-review", type=Path)
    parser.add_argument("--output-dir", type=Path, required=True)
    return qualify(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
