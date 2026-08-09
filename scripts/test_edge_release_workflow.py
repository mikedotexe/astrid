#!/usr/bin/env python3
"""Regression tests for the trusted CPU-edge release workflow boundary."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


WORKFLOW = Path(__file__).parents[1] / ".github/workflows/release.yml"
CPU_EDGE_WORKFLOWS = (
    Path(__file__).parents[1] / ".github/workflows/cpu-edge.yml",
    Path(__file__).parents[1] / ".github/workflows/cpu-edge-astralis-capsules.yml",
)
HEADLESS_GUIDE = Path(__file__).parents[1] / "docs/headless-linux.md"


class ReleaseWorkflowPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.text = WORKFLOW.read_text(encoding="utf-8")

    def test_every_release_action_is_pinned_to_a_full_commit(self) -> None:
        references = re.findall(r"^\s*-?\s*uses:\s*([^\s#]+)", self.text, flags=re.MULTILINE)
        self.assertGreaterEqual(len(references), 10)
        for reference in references:
            with self.subTest(reference=reference):
                self.assertRegex(reference, r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[0-9a-f]{40}$")

    def test_every_cpu_edge_ci_action_is_pinned_and_checkout_is_noncredentialed(self) -> None:
        for workflow in CPU_EDGE_WORKFLOWS:
            text = workflow.read_text(encoding="utf-8")
            references = re.findall(
                r"^\s*-?\s*uses:\s*([^\s#]+)", text, flags=re.MULTILINE
            )
            self.assertGreaterEqual(len(references), 4, workflow)
            for reference in references:
                with self.subTest(workflow=workflow.name, reference=reference):
                    self.assertRegex(
                        reference,
                        r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+@[0-9a-f]{40}$",
                    )
            checkout_count = sum(
                reference.startswith("actions/checkout@") for reference in references
            )
            self.assertEqual(
                text.count("persist-credentials: false"), checkout_count, workflow
            )

    def test_write_and_oidc_permissions_are_job_scoped(self) -> None:
        self.assertRegex(self.text, r"(?m)^permissions:\n  contents: read$")
        edge = self.text.split("\n  edge-appliance:\n", 1)[1].split(
            "\n  github-release:\n", 1
        )[0]
        self.assertIn(
            "    permissions:\n"
            "      contents: read\n"
            "      id-token: write\n"
            "      attestations: write\n"
            "      artifact-metadata: write\n",
            edge,
        )
        release = self.text.split("\n  github-release:\n", 1)[1]
        self.assertIn("    permissions:\n      contents: write\n", release)
        self.assertNotIn("id-token: write", release)
        self.assertEqual(self.text.count("persist-credentials: false"), 3)

    def test_only_x86_profile_builds_and_attests_self_evolution_bootstrap(self) -> None:
        self.assertGreaterEqual(
            self.text.count("if: matrix.target == 'x86_64-unknown-linux-gnu'"), 3
        )
        attest = self.text.split(
            "      - name: Attest complete self-evolution bootstrap provenance\n", 1
        )[1].split("\n      - name:", 1)[0]
        self.assertIn("if: matrix.target == 'x86_64-unknown-linux-gnu'", attest)
        self.assertRegex(attest, r"uses: actions/attest@[0-9a-f]{40}")
        self.assertIn("subject-path: ${{ env.BOOTSTRAP_ASSET }}", attest)
        self.assertIn("subject-name: ${{ env.BOOTSTRAP_NAME }}", attest)

    def test_source_bundle_vendors_the_clean_checkout_with_every_lock(self) -> None:
        build = self.text.split(
            "      - name: Build integrity-bound offline source and native toolchain inputs\n",
            1,
        )[1].split("\n      - name:", 1)[0]
        self.assertIn('git worktree add --detach "$clean_source" "$GITHUB_SHA"', build)
        self.assertIn('--manifest-path "$clean_source/Cargo.toml"', build)
        self.assertRegex(build, r"(?m)^\s+--locked \\$")
        self.assertIn('--cargo-lock "$clean_source/Cargo.lock"', build)
        for service in (
            "astrid-edge-runtime",
            "astrid-edge-checkpoint",
            "astrid-edge-presentation-broker",
            "astrid-edge-provider-broker",
            "astrid-edge-rescue-helper",
            "astrid-edge-steward-helper",
            "astrid-edge-web-broker",
        ):
            self.assertIn(f"services/{service}/Cargo.toml", build)
        for capsule in (
            "astrid-capsule-agents",
            "astrid-capsule-cli",
            "astrid-capsule-edge-context",
            "astrid-capsule-edge-introspector",
            "astrid-capsule-edge-spectral",
            "astrid-capsule-fs",
            "astrid-capsule-http",
            "astrid-capsule-memory",
            "astrid-capsule-shell",
            "astrid-capsule-skills",
        ):
            self.assertIn(capsule, build)

    def test_cpu_edge_ci_covers_private_inquiry_and_retirement_surfaces(self) -> None:
        workflow = CPU_EDGE_WORKFLOWS[0].read_text(encoding="utf-8")
        for test in (
            "scripts/test_astrid_train.py",
            "scripts/test_retire_edge_origin_mac_affordance.py",
        ):
            with self.subTest(test=test):
                self.assertIn(test, workflow)
        for script in (
            "scripts/astrid_train.py",
            "scripts/retire_edge_origin_mac_affordance.py",
        ):
            with self.subTest(script=script):
                self.assertIn(script, workflow)

    def test_headless_guide_does_not_advertise_inherited_corpus_access(self) -> None:
        guide = HEADLESS_GUIDE.read_text(encoding="utf-8")
        self.assertNotIn("inherited corpus for introspection requests", guide)
        self.assertIn("No inherited corpus is available to either appliance.", guide)


if __name__ == "__main__":
    unittest.main()
