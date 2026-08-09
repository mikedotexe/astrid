#!/usr/bin/env python3
"""Tests for trusted-operator CPU-edge release verification."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import os
import sys
import tarfile
import tempfile
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from unittest import mock


SCRIPT = Path(__file__).with_name("verify_edge_self_evolution_release.py")
SPEC = importlib.util.spec_from_file_location("edge_release_verifier", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
module = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = module
SPEC.loader.exec_module(module)


class ReleaseVerificationTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.artifact = (
            self.root
            / "astrid-edge-self-evolution-0.9.0-x86_64-unknown-linux-gnu.tar.gz"
        )
        self.artifact.write_bytes(b"verified bootstrap bytes")
        self.digest = hashlib.sha256(self.artifact.read_bytes()).hexdigest()

    def attestation(self, digest: str | None = None, name: str | None = None) -> bytes:
        document = [
            {
                "verificationResult": {
                    "statement": {
                        "predicateType": module.PREDICATE,
                        "subject": [
                            {
                                "name": name or self.artifact.name,
                                "digest": {"sha256": digest or self.digest},
                            }
                        ],
                    }
                }
            }
        ]
        return json.dumps(document).encode("utf-8")

    def test_attestation_pins_repository_workflow_ref_commit_and_runner(self) -> None:
        with mock.patch.object(module, "run_checked", return_value=self.attestation()) as run:
            identity, _, workflow = module.verify_attestation(
                self.artifact,
                source_ref="refs/tags/v0.9.0",
                source_digest="1" * 40,
            )
        self.assertEqual(identity.sha256, self.digest)
        self.assertEqual(workflow, "mikedotexe/astrid/.github/workflows/release.yml")
        command = run.call_args.args[0]
        self.assertIn("--signer-workflow", command)
        self.assertIn("--source-ref", command)
        self.assertIn("--source-digest", command)
        self.assertIn("--signer-digest", command)
        self.assertIn("--cert-identity", command)
        self.assertIn("--cert-oidc-issuer", command)
        self.assertIn("--hostname", command)
        self.assertIn("--deny-self-hosted-runners", command)
        self.assertIn(module.PREDICATE, command)

    def test_wrong_attested_subject_is_rejected(self) -> None:
        with mock.patch.object(module, "run_checked", return_value=self.attestation("0" * 64)):
            with self.assertRaisesRegex(module.VerificationError, "exact artifact and digest"):
                module.verify_attestation(
                    self.artifact,
                    source_ref="refs/tags/v0.9.0",
                    source_digest="1" * 40,
                )
        with mock.patch.object(
            module,
            "run_checked",
            return_value=self.attestation(name="renamed-identical-bytes.tar.gz"),
        ):
            with self.assertRaisesRegex(module.VerificationError, "exact artifact and digest"):
                module.verify_attestation(
                    self.artifact,
                    source_ref="refs/tags/v0.9.0",
                    source_digest="1" * 40,
                )

    def test_non_tag_and_unexpected_artifact_are_rejected_before_gh(self) -> None:
        with mock.patch.object(module, "run_checked") as run:
            with self.assertRaises(module.VerificationError):
                module.verify_attestation(
                    self.artifact,
                    source_ref="refs/heads/main",
                    source_digest="1" * 40,
                )
            unexpected = self.root / "payload.tar.gz"
            unexpected.write_bytes(b"x")
            with self.assertRaises(module.VerificationError):
                module.verify_attestation(
                    unexpected,
                    source_ref="refs/tags/v0.9.0",
                    source_digest="1" * 40,
                )
            with self.assertRaisesRegex(module.VerificationError, "version"):
                module.verify_attestation(
                    self.artifact,
                    source_ref="refs/tags/v0.9.1",
                    source_digest="1" * 40,
                )
        run.assert_not_called()

    def test_transfer_uses_fixed_host_path_and_checks_remote_digest(self) -> None:
        identity = module.stable_sha256(self.artifact)
        remote = f"/home/avado/astrid-bootstrap/{self.digest}/{self.artifact.name}"
        outputs = [b"", b"", b"", b"", f"{self.digest}  {remote}\n".encode("ascii")]
        with mock.patch.object(module, "run_checked", side_effect=outputs) as run:
            actual = module.transfer_verified(self.artifact, identity, appliance="avado")
        self.assertEqual(
            actual,
            f"/home/avado/astrid-bootstrap/{self.digest}/{self.artifact.name}",
        )
        self.assertEqual(run.call_count, 5)
        self.assertEqual(run.call_args_list[3].args[0][0], "scp")
        self.assertIn("avado:/home/avado/astrid-bootstrap/", run.call_args_list[3].args[0][-1])

    def test_remote_digest_with_wrong_path_is_rejected(self) -> None:
        identity = module.stable_sha256(self.artifact)
        outputs = [b"", b"", b"", b"", f"{self.digest}  remote\n".encode("ascii")]
        with mock.patch.object(module, "run_checked", side_effect=outputs):
            with self.assertRaisesRegex(module.VerificationError, "digest or path"):
                module.transfer_verified(self.artifact, identity, appliance="avado")

    def test_local_substitution_during_transfer_is_rejected(self) -> None:
        identity = module.stable_sha256(self.artifact)
        remote = f"/home/avado/astrid-bootstrap/{self.digest}/{self.artifact.name}"
        outputs = iter([b"", b"", b"", b""])

        def command(*_args, **_kwargs):
            try:
                return next(outputs)
            except StopIteration:
                self.artifact.write_bytes(b"substituted after point-in-time verification")
                return f"{self.digest}  {remote}\n".encode("ascii")

        with mock.patch.object(module, "run_checked", side_effect=command):
            with self.assertRaisesRegex(module.VerificationError, "changed during transfer"):
                module.transfer_verified(self.artifact, identity, appliance="avado")

    def test_root_install_uses_interactive_sudo_and_embedded_trusted_program(self) -> None:
        identity = module.stable_sha256(self.artifact)
        remote = f"/home/avado/astrid-bootstrap/{self.digest}/{self.artifact.name}"
        with mock.patch.object(module, "run_interactive") as interactive:
            module.install_verified_root_handoff(
                appliance="avado",
                remote_path=remote,
                identity=identity,
                source_ref="refs/tags/v0.9.0",
                source_digest="1" * 40,
            )
        command = interactive.call_args.args[0]
        self.assertEqual(command[:3], ("ssh", "-t", "avado"))
        self.assertIn("/usr/bin/sudo", command[3])
        self.assertIn("/usr/bin/python3", command[3])
        self.assertIn(identity.sha256, command[3])
        compile(module.ROOT_HANDOFF_PROGRAM, "<test-root-handoff>", "exec")

    def test_embedded_root_program_copies_hashes_extracts_and_receipts_before_exec(self) -> None:
        root_name = "astrid-edge-self-evolution-0.9.0-x86_64-unknown-linux-gnu"
        initial = self.root / "initial.tar.gz"
        with tarfile.open(initial, "w:gz") as archive:
            directory = tarfile.TarInfo(root_name)
            directory.type = tarfile.DIRTYPE
            directory.mode = 0o700
            archive.addfile(directory)
            installer = b"#!/usr/bin/env bash\nexit 0\n"
            member = tarfile.TarInfo(f"{root_name}/install")
            member.mode = 0o500
            member.size = len(installer)
            archive.addfile(member, io.BytesIO(installer))
        digest = hashlib.sha256(initial.read_bytes()).hexdigest()
        incoming = self.root / "incoming" / digest
        incoming.mkdir(parents=True)
        source = incoming / f"{root_name}.tar.gz"
        initial.rename(source)
        base = self.root / "root-handoff"
        program = module.ROOT_HANDOFF_PROGRAM.replace(
            'BASE = Path("/var/lib/astrid-edge-bootstrap")', f"BASE = Path({str(base)!r})"
        ).replace("REQUIRED_UID = 0", f"REQUIRED_UID = {os.getuid()}").replace(
            "REQUIRED_GID = 0", f"REQUIRED_GID = {os.getgid()}"
        )
        arguments = [
            "trusted-root-handoff",
            str(source),
            digest,
            str(source.stat().st_size),
            root_name,
            "avado",
            "v0.9.0",
            "1" * 40,
        ]

        class ExecCaptured(RuntimeError):
            pass

        captured = {}

        def execve(path, argv, environment):
            captured.update(path=path, argv=argv, environment=environment)
            raise ExecCaptured

        with mock.patch.object(sys, "argv", arguments), mock.patch("os.execve", side_effect=execve):
            with self.assertRaises(ExecCaptured):
                exec(compile(program, "<test-root-handoff>", "exec"), {})
        handoff = base / digest
        self.assertEqual(
            {path.name for path in handoff.iterdir()},
            {"release.tar.gz", "operator-handoff.json", root_name},
        )
        self.assertEqual(hashlib.sha256((handoff / "release.tar.gz").read_bytes()).hexdigest(), digest)
        receipt = json.loads((handoff / "operator-handoff.json").read_text(encoding="ascii"))
        self.assertEqual(receipt["schema"], module.ROOT_HANDOFF_SCHEMA)
        self.assertEqual(receipt["outer_archive_sha256"], digest)
        self.assertEqual(receipt["source_commit"], "1" * 40)
        self.assertEqual(captured["path"], "/usr/bin/bash")
        self.assertIn("--operator-handoff", captured["argv"])

    def test_receipt_is_owner_only_and_never_overwritten(self) -> None:
        receipt = self.root / "receipts" / "verified.json"
        receipt.parent.mkdir(mode=0o700)
        module.write_receipt(receipt, {"schema": module.SCHEMA})
        self.assertEqual(receipt.stat().st_mode & 0o777, 0o600)
        with self.assertRaisesRegex(module.VerificationError, "replace"):
            module.write_receipt(receipt, {"schema": "replacement"})

    def test_repository_cannot_be_overridden(self) -> None:
        with redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
            module.parser().parse_args(
                [
                    "--artifact", str(self.artifact),
                    "--source-ref", "refs/tags/v0.9.0",
                    "--source-digest", "1" * 40,
                    "--repository", "attacker/repository",
                    "--receipt", str(self.root / "receipt.json"),
                ]
            )


if __name__ == "__main__":
    unittest.main()
