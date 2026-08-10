#!/usr/bin/env python3
"""Regression tests for the trusted CPU-edge release workflow boundary."""

from __future__ import annotations

import re
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).parents[1]
WORKFLOW = REPO_ROOT / ".github/workflows/release.yml"
CPU_EDGE_WORKFLOWS = (
    REPO_ROOT / ".github/workflows/cpu-edge.yml",
    REPO_ROOT / ".github/workflows/cpu-edge-astralis-capsules.yml",
)
HEADLESS_GUIDE = REPO_ROOT / "docs/headless-linux.md"
ESSENTIAL_CAPSULES = (
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
)


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

    def test_bootstrap_sidecar_is_verified_from_the_archive_directory(self) -> None:
        assemble = self.text.split(
            "      - name: Assemble and verify complete self-evolution bootstrap\n",
            1,
        )[1].split("\n      - name:", 1)[0]
        self.assertIn('cd "$(dirname "$bootstrap")"', assemble)
        self.assertIn(
            'sha256sum --check --strict '
            '"$(basename "${bootstrap}.sha256")"',
            assemble,
        )
        self.assertNotIn('sha256sum -c "${bootstrap}.sha256"', assemble)

    def test_source_bundle_vendors_the_clean_checkout_with_every_lock(self) -> None:
        build = self.text.split(
            "      - name: Build integrity-bound offline source and native toolchain inputs\n",
            1,
        )[1].split("\n      - name:", 1)[0]
        self.assertIn('git worktree add --detach "$clean_source" "$GITHUB_SHA"', build)
        self.assertIn('--manifest-path "$clean_source/Cargo.toml"', build)
        self.assertRegex(build, r"(?m)^\s+--locked \\$")
        self.assertIn('--cargo-lock "$clean_source/Cargo.lock"', build)
        self.assertIn(
            'git -C "$clean_source" ls-files --error-unmatch --', build
        )
        self.assertIn("crates/astrid-openclaw/kernel/engine.wasm", build)
        self.assertNotRegex(
            build,
            r"install -D -m 0644\s+\\?\s*crates/astrid-openclaw/kernel/engine\.wasm",
        )
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

    def test_external_source_snapshots_stay_outside_checkout_workspace(self) -> None:
        rebuild = self.text.split(
            "      - name: Rebuild pinned external cognition capsules and mutable source inputs\n",
            1,
        )[1].split("\n      - name:", 1)[0]
        bundle = self.text.split(
            "      - name: Build integrity-bound offline source and native toolchain inputs\n",
            1,
        )[1].split("\n      - name:", 1)[0]
        self.assertIn(
            'external_source_root="$RUNNER_TEMP/cpu-edge-external-source"', rebuild
        )
        self.assertIn('test ! -e "$external_source_root"', rebuild)
        self.assertIn(
            '--source-output-dir "$external_source_root/compat"', rebuild
        )
        self.assertIn(
            '--source-output-dir "$external_source_root/baseline"', rebuild
        )
        self.assertNotIn("--source-output-dir dist/external-", rebuild)
        self.assertIn(
            'external_source_root="$(readlink -f '
            '"$RUNNER_TEMP/cpu-edge-external-source")"',
            bundle,
        )
        self.assertIn('"$workspace_root"|"$workspace_root"/*)', bundle)
        self.assertIn('"$external_compat_source"', bundle)
        self.assertIn('"$external_baseline_source"', bundle)
        self.assertIn(
            '--external-capsule-source-dir "$external_compat_source"', bundle
        )
        self.assertIn(
            '--external-capsule-source-dir "$external_baseline_source"', bundle
        )
        self.assertNotIn("$GITHUB_WORKSPACE/dist/external-", bundle)
        self.assertNotIn("--external-capsule-source-dir dist/external-", bundle)

    def test_release_and_cpu_edge_require_the_tracked_quickjs_kernel(self) -> None:
        self.assertIn('ASTRID_REQUIRE_KERNEL_HASH: "1"', self.text)
        self.assertNotIn("ASTRID_AUTO_BUILD_KERNEL", self.text)
        cpu_edge = CPU_EDGE_WORKFLOWS[0].read_text(encoding="utf-8")
        self.assertIn('ASTRID_REQUIRE_KERNEL_HASH: "1"', cpu_edge)
        self.assertNotIn("ASTRID_AUTO_BUILD_KERNEL", cpu_edge)
        self.assertIn("'crates/astrid-openclaw/**'", cpu_edge)

    def test_cpu_edge_ci_covers_private_inquiry_and_retirement_surfaces(self) -> None:
        workflow = CPU_EDGE_WORKFLOWS[0].read_text(encoding="utf-8")
        for path in (
            "'crates/astrid-hooks/**'",
            "'scripts/*hardening*'",
            "'scripts/astrid_train.py'",
            "'scripts/test_astrid_train.py'",
        ):
            with self.subTest(path=path):
                self.assertEqual(workflow.count(path), 2)
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

    def test_optional_homebrew_notification_cannot_fail_the_fork_release(self) -> None:
        notification = self.text.split("      - name: Notify Homebrew tap\n", 1)[1]
        self.assertIn('if [ -z "${GH_TOKEN:-}" ]; then', notification)
        self.assertIn("skipping Homebrew notification", notification)
        self.assertIn("exit 0", notification)

    def test_release_tag_is_bound_to_workspace_version_before_both_builds(self) -> None:
        self.assertEqual(
            self.text.count("- name: Bind release tag to workspace version"), 2
        )
        self.assertEqual(self.text.count('os.environ["GITHUB_REF_NAME"]'), 2)
        self.assertEqual(self.text.count("does not match workspace"), 2)

    def test_capsule_builder_toolchain_alias_cannot_split_from_exact_pin(self) -> None:
        cpu_edge = CPU_EDGE_WORKFLOWS[0].read_text(encoding="utf-8")
        for workflow_name, workflow, expected_toolchain_count in (
            ("release", self.text, 2),
            ("cpu-edge", cpu_edge, 2),
        ):
            with self.subTest(workflow=workflow_name):
                self.assertEqual(
                    len(re.findall(r"(?m)^\s+toolchain: ['\"]1\.94\.1['\"]$", workflow)),
                    expected_toolchain_count,
                )
                self.assertNotRegex(
                    workflow,
                    r"(?m)^\s+toolchain: ['\"]1\.94['\"]$",
                )
        for command in (
            "rustc +1.94.1 --version --verbose",
            "cargo +1.94.1 fmt --version",
            "cargo +1.94.1 clippy --version",
        ):
            with self.subTest(command=command):
                self.assertEqual(self.text.count(command), 1)
                self.assertEqual(cpu_edge.count(command), 2)

    def test_standalone_essential_locks_follow_workspace_guest_version(self) -> None:
        workspace = (REPO_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        workspace_package = workspace.split("[workspace.package]", 1)[1].split(
            "\n[", 1
        )[0]
        version_match = re.search(
            r'(?m)^version = "([^"]+)"$', workspace_package
        )
        self.assertIsNotNone(version_match)
        workspace_version = version_match.group(1)
        for capsule in ESSENTIAL_CAPSULES:
            lock = REPO_ROOT / "capsules" / "astralis" / capsule / "Cargo.lock"
            package_blocks = re.split(
                r"(?m)^\[\[package\]\]\s*$", lock.read_text(encoding="utf-8")
            )[1:]
            guest_versions = []
            for block in package_blocks:
                name = re.search(r'(?m)^name = "([^"]+)"$', block)
                version = re.search(r'(?m)^version = "([^"]+)"$', block)
                source = re.search(r'(?m)^source = "', block)
                if name and name.group(1) == "astrid-guest" and source is None:
                    self.assertIsNotNone(version)
                    guest_versions.append(version.group(1))
            with self.subTest(capsule=capsule):
                self.assertEqual(guest_versions, [workspace_version])

    def test_source_install_is_bound_to_the_attested_fork_tag(self) -> None:
        self.assertIn(
            "cargo install --locked --git https://github.com/mikedotexe/astrid.git "
            "--tag ${{ github.ref_name }} astrid",
            self.text,
        )
        self.assertNotIn("cargo install astrid", self.text)


if __name__ == "__main__":
    unittest.main()
