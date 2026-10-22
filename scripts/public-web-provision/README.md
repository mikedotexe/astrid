# Public-Web Operator Preparation

Standalone, offline Rust workspace. It prepares authority, but has no install,
fetch, service, model, or automatic owner-discovery command. Use only reviewed
operator JSON. Runtime identity must come from the operator's independently
verified owner/startup inputs, never from generated text.

The issuer is dedicated to this purpose and reused for both owners. Policy and
capability lifetime is 365 days from preparation, not from installation. Both
search and page are granted, with no action-count quota. Per-fetch limits are
exactly `PublicWebLimitsV2::default()`: 4 MiB decoded and wire bodies, 120 seconds
total, 5 seconds connect, 10 seconds search hop, 15 seconds page hop, five
redirects, and 64 MiB storage headroom. Existing transport restrictions and the
owner-wide single-flight lock remain the runtime's responsibility.

## Build and Test

From `/Users/v/other/worktrees/freer-exploration-20261021/astrid`:

```sh
python3 -B scripts/build_cache_audit.py --check --target-dir /Users/v/other/worktrees/freer-exploration-20261021/target-web
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0 cargo test --offline --locked --manifest-path scripts/public-web-provision/Cargo.toml --target-dir /Users/v/other/worktrees/freer-exploration-20261021/target-web
CARGO_BUILD_JOBS=2 CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo build --offline --locked --manifest-path scripts/public-web-provision/Cargo.toml --target-dir /Users/v/other/worktrees/freer-exploration-20261021/target-web
```

The executable is
`/Users/v/other/worktrees/freer-exploration-20261021/target-web/debug/astrid-public-web-provision`.
The lockfile is seeded from the existing root lockfile, then pruned offline;
there is no release-profile change. This workspace is not a runtime dependency
and is outside the stage's named helper and package input inventory.

## Prepare

Use absolute, non-symlink paths. The operator must first create a private 0700
parent directory; this tool never changes permissions of an existing object.
Inputs are regular, single-link, current-user-owned 0600 files in 0700 parent
directories. Issuer and output directories must not already exist when created.
The planned installation root must be absent, with an existing non-symlink parent;
it must not overlap the issuer or output. No installation directory is created.
`owner_lock_dir` is required and must already be an absolute, non-symlink,
current-user-owned 0700 directory. It must be disjoint from the issuer, output,
and policy installation root. The tool only validates it with `Directory::open`;
it never creates or changes it. Select the same stable directory for both owners
and all future policy/deployment rotations, for example
`/Users/v/other/astrid/.runtime/public-web/locks`. Owner-specific lock filenames
remain determined by the existing runtime identity API.

```sh
TOOL=/Users/v/other/worktrees/freer-exploration-20261021/target-web/debug/astrid-public-web-provision
"$TOOL" init-issuer --issuer-dir /absolute/private/operator/issuer
"$TOOL" prepare --issuer-dir /absolute/private/operator/issuer --request /absolute/private/operator/astrid-reviewed.json --out /absolute/private/operator/astrid-packet
"$TOOL" prepare --issuer-dir /absolute/private/operator/issuer --request /absolute/private/operator/minime-reviewed.json --out /absolute/private/operator/minime-packet
```

Prepare request (placeholders are deliberately not valid owner identities):

```json
{
  "schema": "astrid.public_web.provision.prepare.v1",
  "owner": {
    "being": "astrid",
    "principal": "REVIEWED_PRINCIPAL",
    "public_key_hex": "REVIEWED_64_HEX_OWNER_PUBLIC_KEY",
    "deployment_identity": "REVIEWED_STARTUP_INPUT_IDENTITY"
  },
  "policy_id": "REVIEWED_UNIQUE_POLICY_ID",
  "install_root": "/absolute/private/selected-owner-policy",
  "owner_lock_dir": "/Users/v/other/astrid/.runtime/public-web/locks",
  "approval_audit_ref": "REVIEWED_USER_APPROVAL_REFERENCE"
}
```

Each packet contains `bundle.json`, `config.json`, `trust.json`,
`runtime-owner.json`, `approval.json`, and a content-free `receipt.json` written
last. The bundle includes the exact signed capability JSON. Config selects
`policy.json`, `state.json`, `state-floor.json`, `delegation/`, and `storage/`
beneath the reviewed install root. The supplied stable `owner_lock_dir` is
preserved separately and bound by the config/approval hashes. `trust.json` is an unsigned trust
bootstrap proposal for that owner's new dedicated delegation directory, **not**
an instruction to replace existing runtime trust. The operator owns all later
directory/trust provisioning, existing-helper dry-run/install, and launch binding.

Policy/state/approval signatures use their exported `signing_bytes()` methods
and `astrid_crypto::KeyPair::sign`; capability creation uses
`CapabilityToken::create`. No alternative canonicalization or signature format
is implemented. Hashes of typed statements use their existing `sha256()` APIs;
`file_sha256` is explicitly the hash of emitted file bytes. Trust has no public
constructor in the runtime crate, so only its existing unsigned serde wire
document is represented locally and exercised through the actual validator.

## Renew Only Approval

Install/revoke approval expires in five minutes. To renew, supply the exact
original documents and their reviewed hashes in a new private JSON file:

```json
{
  "schema": "astrid.public_web.provision.renew.v1",
  "exact": {
    "owner": { "being": "astrid", "principal": "REVIEWED_PRINCIPAL", "public_key_hex": "REVIEWED_64_HEX_OWNER_PUBLIC_KEY", "deployment_identity": "REVIEWED_STARTUP_INPUT_IDENTITY" },
    "owner_lock_dir": "/Users/v/other/astrid/.runtime/public-web/locks",
    "config_path": "/absolute/private/operator/astrid-packet/config.json",
    "bundle_path": "/absolute/private/operator/astrid-packet/bundle.json",
    "config_sha256": "RECEIPT_CONFIG_HASH",
    "policy_sha256": "RECEIPT_POLICY_HASH",
    "state_sha256": "RECEIPT_STATE_HASH"
  },
  "approval_path": "/absolute/private/operator/astrid-packet/approval.json",
  "approval_file_sha256": "RECEIPT_ARTIFACT_APPROVAL_FILE_HASH",
  "approval_audit_ref": "REVIEWED_RENEWAL_REFERENCE"
}
```

```sh
"$TOOL" renew-approval --issuer-dir /absolute/private/operator/issuer --request /absolute/private/operator/renew-reviewed.json --out /absolute/private/operator/renewed-packet
```

Renewal preserves policy, state, config, operation and previous-floor binding.
The required `exact.owner_lock_dir` must match the pinned config, and its existing
private directory and disjointness are revalidated on renewal and revocation.
It neither extends the one-year grant nor rebases approval onto a different
installed/pending head. The old signature is checked at its issue time; current
policy/token validity is still required. The new packet has no new trust proposal.
Use the existing helper's explicit dry-run to detect any intervening head change.

## Prepare Revocation

Use schema `astrid.public_web.provision.revoke.v1`, the same `exact` object, and
these additional fields from an independently reviewed existing-helper status:

```json
{
  "expected_previous_floor_sha256": "REVIEWED_CURRENT_FLOOR_HASH",
  "expected_state_revision": 1,
  "approval_audit_ref": "REVIEWED_REVOCATION_REFERENCE"
}
```

```sh
"$TOOL" prepare-revoke --issuer-dir /absolute/private/operator/issuer --request /absolute/private/operator/revoke-reviewed.json --out /absolute/private/operator/revocation-packet
```

This command reads the selected installed authority through the library's
`status`, compares every reviewed precondition, signs the next revoked state,
and calls read-only `preview_mutation`. It does not publish it. A changed head,
pending transition, already revoked or expired policy refuses preparation.
Expired authority already denies fetch; this tool does not revive it to revoke.

## Failure and Key Handling

The 0600 `issuer.key` has a version header, 32-byte seed and matching public key.
Only the public key and key path are reported. The seed and read buffers are
zeroized in memory. A missing, malformed, mismatched, public-readable,
hardlinked or symlinked key is refused, never regenerated. Keep the issuer out
of Git, ordinary artifacts, stdout, and logs.

Descriptor-relative no-follow access is used for every file component; exclusive
creation prevents replacement. All created files are 0600 and directories 0700.
A partial failure leaves its new directory for operator inspection, with no
successful completion receipt. No cleanup, retry-overwrite or force mode exists.
Tests use only temporary synthetic identities/state and the existing library's
preview/mutate/status validators. They never fetch, use live keys, or deploy.
