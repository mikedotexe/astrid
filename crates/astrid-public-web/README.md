# Public Web Foundation V2

This crate is a policy/transport foundation, not a live rollout. It changes no
authority by itself and creates no grant. It includes approved operator
publication, never key generation, trust bootstrap, or services. The parent
owns effect classification, volition, adapter wiring, and deployment.

## Adapter API

Native callers use:

```rust,ignore
let config = astrid_public_web::PublicWebConfig::load(operator_config_path)?;
let result = astrid_public_web::fetch(&config, &current_runtime_owner, &request).await;
```

Python invokes the same Rust implementation, without a second HTTP client:

```text
astrid-public-web fetch --config /absolute/operator/config.json --runtime-owner '{"being":"...","principal":"...","public_key_hex":"...","deployment_identity":"..."}'
```

One bounded JSON request is read from stdin:

```json
{"schema":"astrid.public_web.request.v2","operation":"page","target":"https://example.com/"}
```

`operation` is `page` or `search`; search takes a literal query, not a URL or
command. The current search endpoint is DuckDuckGo HTML. The adapter resolves
SEARCH/RESEARCH and BROWSE/EXAMINE-URL aliases to these effects before admission.
It must never fall back to the previous HTTP path on any failure.

The runtime owner is trusted adapter input, not model-authored JSON. Astrid
should obtain deployment identity from the same
`signal_spine::signal_deployment_identity_v1()` used by existing volition, and
use its current signer/public identity. Minime must provide its own equivalent
loaded-runtime identity, not a canonical checkout hash or the policy's claim.
The explicit runtime owner must equal the operator configuration, which must
equal the signed policy. Loading a stale matching config/policy is insufficient.

Success returns `final_url`, `content_type`, `body` (JSON byte array for the
helper), and a content-free `receipt`. Only the intended reading consumer may
receive the payload; logs and general receipts use `receipt` alone. Failures
return `complete:false`, a fixed `code`, request hash, and observed byte/redirect
counts, with no partial body, raw URL, query, path, or underlying exception.
The helper exits nonzero on failure. Its stdout payload is bounded but larger
than 4 MiB because JSON represents bytes as numbers; bound subprocess capture
accordingly (20 MiB is sufficient). Incomplete helper output/process death is
not success or permission to retry outside this transport.

`astrid-public-web preview --config /absolute/operator/config.json` performs
read-only signature/trust/state validation and prints policy/owner hashes,
revision, expiry, operations and limits. Revoked/unavailable authority returns a
fixed error. Preview neither installs authority nor claims adapter integration.

### Minime Thin Adapter

The sibling `minime_autonomy/public_web.py` exposes `fetch(agent, operation,
target) -> dict` and `preview(agent, operation) -> dict`, raising only
`PublicWebError` with a content-free `.code`. Operations are `search` and `page`.
Preview invokes the shared CLI without a fetch request: it is an availability
observation, not a queue reservation. Fetch independently rechecks everything.

It requires `MINIME_PUBLIC_WEB_HELPER`, `MINIME_PUBLIC_WEB_HELPER_SHA256`, and
`MINIME_PUBLIC_WEB_CONFIG`; there is no discovery or HTTP fallback. Pipe capture
is bounded to 20 MiB and helper execution to 125 seconds (plus bounded child
reaping on failure). Stderr is discarded and subprocesses use no shell or
ambient loader environment. Helpers are hash-checked before and after use.

Actual owner is `being=principal=minime`, public key from the existing
`minime.self_control.trust_store.v1` `pinned_public_keys.minime`. Root resolution
matches Rust: `MINIME_SELF_CONTROL_ROOT`, else `$HOME/.minime/self-control-v2`,
else `workspace/self-control-v2` relative to the current directory. Only public
`trust.json` is read, never private identity or seed files. Deployment identity
is lowercase SHA-256 of the sorted compact UTF-8 JSON startup source inventory
(`ensure_ascii=False`). Current `source_inputs(BASE_DIR)` must equal the
nonempty startup inventory before and after each call. Config identity must
equal this derived owner, never supply it. This is a trusted local source
assertion, not cryptographic attestation of Python's loaded machine code.

Success validates exact receipt fields, request/policy/owner/URL/body hashes,
completion, status and byte/redirect ceilings before returning payload. Unknown
receipt fields are refused, so raw helper errors cannot enter receipt logs.
Both languages carry a fixed Unicode request/owner hash fixture. Python tests
use temporary source/trust/config files and mocked processes with real anonymous
pipes; they never execute the helper or perform a network request.

## Authority And Installation Contract

`astrid_minime_protocol::public_web` exports `PublicWebPolicyV2`,
`PublicWebPolicyStateV2`, `PublicWebOwnerV2`, `PublicWebOperationV2`, and limits.
The signed JSON is inspectable. Its canonical signing bytes clear only
`signature_hex`; use the provided `signing_bytes()` and `sha256()` methods.

Requirements before any fetch:

- Existing `astrid.volition.delegated_trust.v1` bootstrap trust, with the exact
  owner being, trusted issuer and approval references. No implicit bootstrap.
- A valid existing `CapabilityToken`, `Persistent`, not single-use, exactly
  `Permission::Read` on `network://public-web/v2`. Its canonical displayed ID is
  `token:<uuid>`, and its exact file bytes are SHA-256-bound by the policy.
- A new issuer-signed `volition.public_web_policy.v2`, binding the owner,
  public key, loaded deployment, token identity/hash, allowed operations,
  expiry, approval references, and per-fetch safety ceilings.
- Current signed `volition.public_web_policy_state.v2`, exact policy hash and
  revision at least the operator's durable floor. Missing, malformed, expired,
  mismatched or revoked state denies. Only the trusted issuer activates; either
  issuer or the policy owner may sign revocation.

There is no lifetime `action_count`, implicit V1 conversion, experiment quota,
or usage mutation. An old `network://search/**` capability is deliberately not
accepted as a standing public-web capability.

`PublicWebConfig` requires explicit absolute paths: `delegation_root` (existing
`delegated_capabilities`, containing `trust.json` and `tokens/<id>.json`),
`policy_path`, `state_path`, `state_floor_path`, `owner_lock_dir`, and `storage_path`; plus
`expected_owner`, `expected_policy_sha256`, `minimum_state_revision`, and schema
`astrid.public_web.config.v2`. Authority files must be private regular files,
owned by the effective OS user; containing directories must be private and
paths must not traverse symlinks. Temporary fixture paths are canonicalized.

Operator commands require pre-signed files, not a textual acknowledgement:

```text
astrid-public-web status --config /absolute/config.json
astrid-public-web install --config /absolute/config.json --runtime-owner OWNER_JSON --bundle /absolute/bundle.json --approval /absolute/approval.json --dry-run
astrid-public-web revoke --config /absolute/config.json --runtime-owner OWNER_JSON --bundle /absolute/bundle.json --approval /absolute/approval.json
```

`PublicWebBundle` contains `policy`, `state`, and `capability_token_json` (the
exact original token bytes as a string). `PublicWebApprovalV2` binds operation,
owner, policy/state/config hashes, previous floor hash, audit reference and a
maximum five-minute approval window. Install requires the trusted issuer;
revocation accepts the owner or issuer. Use its signing/hash methods rather
than reimplementing canonicalization. The tool never reads signing secrets.

Publication serializes operators, fsyncs a pending revision floor first, then
atomically publishes token (no-clobber), policy and state, revalidates approval,
and commits/fsyncs the floor. Pending always denies fetches. An exact retry, or
fresh approval of the exact pending candidate, can finish interrupted work;
another candidate cannot replace it. Existing orphaned state is not adopted.
Revoke advances revision, never deletes state. Status inspects active/revoked
authority. Mutation output contains only operation, hashes, revision, previous
floor hash, `already_published` and `implementation_scope:transport_foundation`.
It is not a claim that adapters or a live deployment are qualified.

Policy rotation requires a newly signed policy and explicit config/hash
selection. The private revision floor prevents restoring only an older signed
state; it cannot prevent an OS-authorized rollback of all authority files and
the floor together. Provisioning approvals and live rollout remain external.

## Physical Scope

- Only credential-free public HTTP port 80 / HTTPS port 443 GETs. No proxy,
  cookies, authorization headers, Referer, authenticated APIs, models, or
  substrate commands. TLS verification remains enabled.
- DNS answers are resolved per hop, all validated (mixed/private/special sets
  fail), then pinned into a fresh reqwest client. The connected peer must be in
  the pinned set. Literal addresses receive the same public-unicast checks.
- Redirects are manual, at most five, with URL and DNS revalidation on every
  hop, including same-host redirects. Redirect bodies count toward limits.
- Decoded bodies are cumulatively capped at 4 MiB per logical fetch, as are
  encoded body bytes. Identity and gzip are supported; other encodings fail
  explicitly. Gzip input is bounded before incremental bounded-output decoding;
  truncation/checksum errors return incomplete. Only 200 text/HTML/XHTML/JSON
  or PDF results are complete; 206 is not promoted to a complete page. PDF
  extraction is a separate caller operation, not Rust text decoding.
- One 120-second logical deadline includes DNS, headers, bodies, redirects and
  decoding. Per-hop search/page limits stay at most 10/15 seconds; connection
  timeout is at most 5 seconds. Signed policy can tighten these ceilings.
- One cross-process fs2 kernel lock per owner, across deployments/policies.
  Both adapters and all replacement/rollback processes MUST share the same
  operator-selected lock directory. Never unlink lock files. Process exit
  releases the lock; it does not prove an interrupted fetch succeeded.
- At least 64 MiB headroom plus the decoded-body allowance is checked on the
  configured storage volume before network work and at checkpoints. This is
  an admission check, not reserved disk space. Retention is a separate caller
  operation and must recheck headroom and preserve its own error/atomicity rules.

Authority/state and headroom are rechecked before/after hops and body chunks,
and before returning success. Revocation during a pending socket read is
noticed at the next checkpoint, bounded by the tighter hop deadline; it does
not retroactively undo bytes already sent/received. No hidden retries occur.
Receipts describe HTTP-body completion, not semantic usefulness, model output,
successful retention, or total TLS/header traffic. HTML script execution and
subresource fetching are not performed. Public content remains untrusted data.

## Qualification Boundary

Offline fixture tests use real signatures, the actual existing capability
format, temporary private authority files, fake DNS/body backends, and real
child-process lock exclusion/abrupt-exit release. Operator tests exercise real
signatures/publication, each interrupted publication stage, exact recovery,
stale approval rejection and owner revocation. They do not contact the
internet, start live services, load models, or install grants. Production TLS,
DNS infrastructure, and peer pinning against real sockets are statically wired
but are not end-to-end network-qualified by these fixtures.

First integration step: pin one reviewed helper/library build and the actual
loaded owner identities, route both adapters' parsed network effects through
this API, and qualify fail-closed/no-fallback behavior using synthetic helper
responses. Operator fault injection is not a physical power-loss test; qualify
deployment-specific paths, identity assertions and recovery before any approved
grant or network canary. No live installation is part of these tests.
