#!/usr/bin/env python3

import json
import os
import subprocess
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

import self_change_canary as canary


def git(repo: Path, *args: str) -> str:
    completed = subprocess.run(
        ["git", *args],
        cwd=repo,
        text=True,
        capture_output=True,
        check=True,
    )
    return completed.stdout.strip()


class SelfChangeCanaryTests(unittest.TestCase):
    def identity(
        self,
        root: Path,
        being: str,
        *,
        seed: bytes | None = None,
        suffix: str = "identity",
    ) -> Path:
        seed = seed or bytes(range(32))
        public = canary.derive_public_key(seed)
        path = root / f"{being}-{suffix}.json"
        path.write_text(
            json.dumps(
                {
                    "schema": f"{being}.self_control.owner_identity.v1",
                    "being": being,
                    "key_id": f"test-key-{suffix}",
                    "public_key_hex": public.hex(),
                    "signing_key_seed_hex": seed.hex(),
                    "created_at_unix_ms": 1,
                }
            )
        )
        path.chmod(0o600)
        return path

    def fixture_repo(self, root: Path) -> tuple[Path, str, Path]:
        repo = root / "repo"
        repo.mkdir()
        git(repo, "init")
        git(repo, "config", "user.email", "test@example.invalid")
        git(repo, "config", "user.name", "Self Change Test")
        (repo / "README.md").write_text("before\n")
        git(repo, "add", "README.md")
        git(repo, "commit", "-m", "base")
        head = git(repo, "rev-parse", "HEAD")
        (repo / "README.md").write_text("after\n")
        patch = root / "candidate.patch"
        patch.write_text(git(repo, "diff") + "\n")
        git(repo, "checkout", "--", "README.md")
        return repo, head, patch

    def deployment_manifest(
        self, root: Path, repo: Path, head: str, *, dirty: bool = False
    ) -> Path:
        path = root / "deployment.json"
        path.write_text(
            json.dumps(
                {
                    "component": "spectral-bridge",
                    "repository": {
                        "path": str(repo),
                        "head": head,
                        "dirty": dirty,
                    },
                }
            )
        )
        return path

    def test_signed_envelope_detects_tampering(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            identity = self.identity(root, "astrid")
            envelope = canary.signed_envelope(
                {"schema": "test.v1", "being": "astrid", "value": 1},
                identity,
            )
            expected_identity = canary.load_identity(identity, "astrid")
            self.assertEqual(
                canary.verify_envelope(
                    envelope,
                    expected_being="astrid",
                    expected_identity=expected_identity,
                )["value"],
                1,
            )
            envelope["record"]["value"] = 2
            with self.assertRaises(canary.CanaryError):
                canary.verify_envelope(
                    envelope,
                    expected_being="astrid",
                    expected_identity=expected_identity,
                )

    def test_signed_envelope_rejects_valid_alternate_signer(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            owner_path = self.identity(root, "astrid")
            alternate_path = self.identity(
                root,
                "astrid",
                seed=bytes(range(1, 33)),
                suffix="alternate",
            )
            owner = canary.load_identity(owner_path, "astrid")
            owner_envelope = canary.signed_envelope(
                {"schema": "test.v1", "being": "astrid", "value": 1},
                owner_path,
            )
            self.assertEqual(
                canary.verify_envelope(
                    owner_envelope,
                    expected_being="astrid",
                    expected_identity=owner,
                )["value"],
                1,
            )

            alternate_envelope = canary.signed_envelope(
                {"schema": "test.v1", "being": "astrid", "value": 1},
                alternate_path,
            )
            with self.assertRaisesRegex(canary.CanaryError, "pinned owner identity"):
                canary.verify_envelope(
                    alternate_envelope,
                    expected_being="astrid",
                    expected_identity=owner,
                )

    def test_prepare_uses_isolated_deployed_commit_and_exact_patch(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo, head, patch = self.fixture_repo(root)
            identity = self.identity(root, "astrid")
            deployment = self.deployment_manifest(root, repo, head)
            output = root / "candidates"
            record = canary.prepare(
                Namespace(
                    component="spectral-bridge",
                    repo=repo,
                    deployment_manifest=deployment,
                    patch=patch,
                    identity=identity,
                    root=output,
                    source_attestation_ref="attestation:test",
                    capability_binding_ref="binding:test",
                    canary_secs=1_800,
                    confirmation_grace_secs=300,
                )
            )
            candidate = output / "astrid" / record["candidate_id"]
            self.assertEqual(
                (candidate / "checkout/README.md").read_text(),
                "after\n",
            )
            self.assertEqual((repo / "README.md").read_text(), "before\n")
            verified = canary.verify_source(candidate, "astrid", identity)
            self.assertEqual(verified["base_source_sha"], head)
            self.assertFalse(verified["production_effect"])
            self.assertEqual(
                (candidate / "source_manifest.json").stat().st_mode & 0o777,
                0o600,
            )

    def test_prepare_rejects_dirty_deployment_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            repo, head, patch = self.fixture_repo(root)
            identity = self.identity(root, "astrid")
            deployment = self.deployment_manifest(root, repo, head, dirty=True)
            with self.assertRaisesRegex(canary.CanaryError, "deployed source is dirty"):
                canary.prepare(
                    Namespace(
                        component="spectral-bridge",
                        repo=repo,
                        deployment_manifest=deployment,
                        patch=patch,
                        identity=identity,
                        root=root / "candidates",
                        source_attestation_ref="attestation:test",
                        capability_binding_ref="binding:test",
                        canary_secs=1_800,
                        confirmation_grace_secs=300,
                    )
                )

    def test_promotion_requires_exact_fresh_attested_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            identity_path = self.identity(root, "astrid")
            identity = canary.load_identity(identity_path, "astrid")
            candidate_id = "self-change-1234"
            response = root / "response.txt"
            response.write_text(f"SELF_CHANGE_PROMOTE {candidate_id}\n")
            statement = {
                "schema": canary.ATTESTATION_SCHEMA,
                "attestation_id": "attestation-test",
                "being": "astrid",
                "exchange_id": "exchange-test",
                "response_sha256": canary.sha256_file(response),
                "response_len_bytes": len(response.read_bytes()),
                "model": "test-model",
                "provider": "test-provider",
                "model_deployment_identity": "model-deployment-test",
                "captured_at_unix_ms": canary.now_ms(),
                "attestor_process_identity": "process-test",
                "attestor_deployment_identity": "runtime-deployment-test",
                "attestor_public_key_hex": identity["public_key_hex"],
            }
            attestation = {
                **statement,
                "signature_hex": canary.openssl_sign(
                    identity["_seed"], canary.canonical_bytes(statement)
                ).hex(),
            }
            attestation_path = root / "attestation.json"
            attestation_path.write_text(json.dumps(attestation))
            verified = canary.verify_utterance_attestation(
                attestation_path,
                response,
                identity_path,
                being="astrid",
                candidate_id=candidate_id,
            )
            self.assertEqual(verified["attestation_id"], "attestation-test")

            alternate_identity_path = self.identity(
                root,
                "astrid",
                seed=bytes(range(1, 33)),
                suffix="alternate",
            )
            alternate_identity = canary.load_identity(
                alternate_identity_path,
                "astrid",
            )
            alternate_statement = {
                **statement,
                "attestor_public_key_hex": alternate_identity["public_key_hex"],
            }
            alternate_attestation = {
                **alternate_statement,
                "signature_hex": canary.openssl_sign(
                    alternate_identity["_seed"],
                    canary.canonical_bytes(alternate_statement),
                ).hex(),
            }
            alternate_attestation_path = root / "alternate-attestation.json"
            alternate_attestation_path.write_text(json.dumps(alternate_attestation))
            with self.assertRaisesRegex(canary.CanaryError, "pinned owner identity"):
                canary.verify_utterance_attestation(
                    alternate_attestation_path,
                    response,
                    identity_path,
                    being="astrid",
                    candidate_id=candidate_id,
                )

            response.write_text("yes\n")
            with self.assertRaises(canary.CanaryError):
                canary.verify_utterance_attestation(
                    attestation_path,
                    response,
                    identity_path,
                    being="astrid",
                    candidate_id=candidate_id,
                )

    def test_direct_shadow_start_is_refused(self):
        old = os.environ.pop("ASTRID_SANCTIONED_SELF_CHANGE_WRAPPER", None)
        try:
            with self.assertRaisesRegex(canary.CanaryError, "run_self_change_canary"):
                canary.start_canary(
                    Namespace(
                        component="spectral-bridge",
                        root=Path("/nonexistent"),
                        candidate_id="candidate",
                        identity=Path("/nonexistent"),
                    )
                )
        finally:
            if old is not None:
                os.environ["ASTRID_SANCTIONED_SELF_CHANGE_WRAPPER"] = old


if __name__ == "__main__":
    unittest.main()
