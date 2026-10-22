# Search Delivery: Known Challenge, Not Queued Results

> Status chronology: the sections below retain the earlier diagnostic tranche's
> evidence and hashes. The final Brave implementation section supersedes its
> candidate-source descriptions, not its historical test results. Nothing here
> establishes a live Brave activation.

## Scope and Status

Mike requested follow-through on the earlier finding that page reads passed but
searches returned HTTP 202. This is operator-reported engineering follow-through,
not a new being-authored complaint or evidence about either being's interests.

Candidate worktrees, both `codex/search-delivery-20261022`:

- Astrid: `/Users/v/other/worktrees/search-delivery-20261022/astrid`, based on
  `054f4ff335f197c3f2921b995f82121a0eb4c00a`.
- Minime: `/Users/v/other/worktrees/search-delivery-20261022/minime`, based on
  `73f1e390565c52f83c77fd4ffd31d38103c3bb33`.

No activation, grant change, letter, model call, restart, Git integration or
automation resume is included. Search availability is **not restored** by this
diagnostic repair. Canonical main trees were clean at the start.

## Direct Evidence

The prior rollout's `public-web-canaries-01.json` has SHA-256
`3e9c143c0865398a89d2033f8c654ab847cb6f01582d525b6cc8fbe3ce8e062f`;
its page requests passed and searches failed with generic `http_status`.
See [the rollout record](2026-10-22-freer-exploration-rollout.md).

One bounded operator diagnostic on 2026-10-22 used the public query
`echo state network`, the transport's `Astrid-Public-Web/2` user agent, a
25-second deadline and a 1 MiB body cap. It requested only
`https://html.duckduckgo.com/html/?q=echo+state+network`. The response reported
HTTP 202, `text/html; charset=UTF-8`, and server date
`Thu, 22 Oct 2026 21:31:14 GMT`. It contained the service's `challenge-form`,
`anomaly-modal__description` and a request for human verification. This was a
challenge page, not search hits or evidence of an asynchronous search queue.

This curl diagnostic is not a production-helper attestation. No challenge form,
image, script, cookie retry, alternate user agent, proxy or fallback service was
used. The response body was not retained in the repository or supplied to a
being/model. Tests use a short synthetic template, not copied challenge tokens.

## Repair

- Shared Rust failures retain optional `http_status` for the failed hop.
  Earlier redirects do not become the status of a later DNS/transport failure.
  Old receipts without this field remain readable and explicitly unknown.
- A narrow fingerprint identifies this known template only for search,
  `html.duckduckgo.com`, `/html` or `/html/`, and HTML. It yields `search_challenge`, including when
  the template arrives with HTTP 200. This is not a general CAPTCHA classifier.
  Unknown markup is not claimed to be a diagnosed challenge.
- Generic HTTP 202 remains incomplete, without a claim that work is queued or
  that a later result will arrive. No polling or automatic retry is introduced.
- Stream completeness, authority, address, byte, deadline and headroom checks
  retain precedence. Failure receipts include no target, upstream prose or
  partial body. A challenge does not become source evidence.
- Bridge feedback and its explicit operator search probe preserve the safe
  explanation instead of erasing it. The optional legacy enrichment path still
  omits unavailable enrichment; its shared transport logs the content-free
  failure. No unrelated generation or activity is substituted.
- Minime's thin adapter validates the new code and optional status and preserves
  the delivery reason in action feedback. It adds no network or HTML grammar.

Receipt schema remains v2 with an additive optional status. Older Rust readers
can read missing status; old clients that do not recognize the new error code
fail closed. The helper and clients must be qualified together before activation.
No authored-state migration or rewriting of historical receipts is needed.

## Qualification

Before builds, the explicit target audit passed: approximately 2.82 GiB debug
cache for `target-web`, 5.57 GiB for `target-bridge`, and 397.09 GiB free.
Reused owned caches under
`/Users/v/other/worktrees/freer-exploration-20261021/`, with
`CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0`.
No release profile or immutable release bytes were changed.

Synthetic tests
exercise success, known challenges at 200/202/403, generic 202, no retry,
query/body non-disclosure, scope exclusions, body-limit/revocation precedence,
failed redirect-hop status and legacy receipts.

Completed checks:

- `cargo test --locked -p astrid-public-web`: 48 library tests and one CLI test
  pass; one process fixture is intentionally ignored as a standalone test and
  invoked by the cross-process tests.
- Shared transport strict Clippy and formatting pass. One intermediate strict
  run found the fetch function at 101 lines; common media-type normalization is
  now shared between the two consumers rather than suppressing the lint.
- Bridge library suite: final post-review and normalization run **2,544 pass,
  three existing ignored**, 42.84 seconds. Final domain-boundary verification:
  zero violations. Workspace and bridge formatting checks pass. Final bridge
  `cargo clippy --locked --all-targets -- -D warnings` passes.
- Minime focused checks: 39 tests and 75 subtests pass. Complete configured run:
  **2,580 pass, one existing skip, 341 subtests pass**, 127.58 seconds. Log at
  `/Users/v/other/worktrees/search-delivery-20261022/minime-tests-configured.log`,
  SHA-256 `a52b17d9ebd971e37a64ce29f97e8a346a92427e00c5bcca505710bf6a8e78c1`.
  It used the unchanged source-reader helper
  `target-web/debug/astrid-source-study`, SHA-256
  `9b147546976f660c7f845cb1d73af5ed80edc13ebfe7586767a009432dac8258`.
- Independent read-only review identified status loss on invalid headers and an
  overbroad same-host path match. Both were repaired; the reviewer confirmed the
  fixes. Production header parsing now retains status before refusal without
  reading a rejected body. Added negative same-host/different-path coverage.

The initial full Python invocation omitted `ASTRID_SOURCE_STUDY_BIN`: 46 failures,
156 setup errors, 2,378 passes and one skip. Most failures were missing-helper
setup; one same-millisecond synthetic session-draft ID collision also occurred.
The correctly configured full rerun passed without test/source changes. That
timing-sensitive fixture remains a known residual; it is not a search regression
or proof of live data loss. The failed diagnostic log remains outside Git with
inherited environment renderings redacted. Pytest exposed part of an inherited
API credential while rendering the missing-variable exception. No credential
was copied to source or these notes; the rerun used `env -i`, an explicit helper,
and short tracebacks. Rotation of the affected credential is prudent.

Candidate Minime source hashes:

- `minime_autonomy/public_web.py`:
  `e23c9e69397fae72189cb292e15764d6bc4f72eb94378dd80339c91156ac2606`
- `minime_autonomy/runtime.py`:
  `015d5b66ea07e95f1ba7081cb56470d8976f2d738d86fb7df31a448d61240bc9`

Final transport and bridge test logs remain outside Git under
`/Users/v/other/worktrees/search-delivery-20261022/`:

- `transport-tests-final.log`:
  `8f4ac274e47ad848b58641b715384e5080d22ac004f44d46f0c1229f2d8d260e`
- `bridge-tests-post-normalization.log`:
  `d1b133aa615cfb8ac9e3a0ac6def0f95c9accd4bb6a68e4269ea1abbcc9495b9`

Final candidate Rust source hashes (SHA-256):

- `crates/astrid-public-web/src/lib.rs`:
  `baa576a9672278f34e57f42792d756e9069fb201b9f7ddb03d2881c29ef65968`
- `crates/astrid-public-web/src/network.rs`:
  `396da6cec3348bb314ef82a0e3fc55e66e53a2f60ca306f1fbbe7725b45b20e3`
- `crates/astrid-public-web/src/transport.rs`:
  `faff5ff55db0220163dc5a3619fcfe8455a069c6104ecfbc52c05457616296a8`
- `crates/astrid-public-web/src/tests/search_delivery.rs`:
  `13242a09382ec353306e25a2f74bb343ab23649d9e8d899eb1edf406d9b3852a`
- `capsules/spectral-bridge/src/public_web.rs`:
  `91cb66fad52defd0388addde2ca884b8b5c3140d2364ec82da2269ffd731eb87`
- `capsules/spectral-bridge/src/mcp.rs`:
  `7679357bd01ac1b689732c11b1f8587b7eaf937a5e4fd0572e70e44f30e3b018`

Canonical trees were rechecked clean on `main`, aligned with their local
`origin/main` references. No fetch or remote-write verification is claimed by
that status check. Candidate changes remain unstaged and uncommitted.

At the post-build audit, the same caches remained within budget (approximately
2.82 and 5.77 GiB); free space was 395.87 GiB, down about 1.22 GiB from the initial
snapshot. These are non-atomic whole-volume measurements, not an attribution of
all growth to this work. No cache was deleted: the helper is used by these tests,
the bridge cache remains in qualification use, and earlier qualification work
also references the reused cache roots. Immutable releases remain untouched.

Controller status remained paused at generation 534 with no lease or active
projection. Indexed-tail evidence stayed valid at sequence 1123188, head
`6064ec94a2dd37895edd952bc0ffcd48322215249aa265724cc0594b879b0cdd`;
all four V1 streams were immutable. This was interactive candidate work, not a
productive automation round. No source-first run or projection was claimed.

## Restoring Dependable Search

Recommended next candidate: a supported search API with a structured response,
not changes to DuckDuckGo's challenge handling. Brave documents
[API-key authentication](https://api-dashboard.search.brave.com/documentation/guides/authentication)
and a [web search endpoint](https://api-dashboard.search.brave.com/app/documentation/web-search).
An API account, credential placement and any spending must be approved explicitly.
The existing public-web grant excludes authenticated and paid services; it must
not be silently reinterpreted as that approval.

Keep page reads on the existing public transport. Any new API integration should
use an operator-selected credential restricted to the exact API origin, never
forward it to redirects or returned page links, parse bounded structured results
in shared Rust, and retain current network/resource and owner checks. Do not
introduce inherited experiment quotas or automatic query substitution. Qualify
synthetic failure cases and a bounded approved live search before promising it
to either being. No account or subscription was created in this tranche.

## Brave Evaluation Requested by Mike

**Recommendation: technically suitable, conditional on retention rights and an
approved account/configuration.** Use its structured web search, not generated
Answers, as the initial candidate. It fits our existing title/URL/snippet view
and leaves interpretation with Astrid and Minime. No live quality or latency
comparison has been run; an API's advertised coverage is not measured suitability
for their questions.

The [web API reference](https://api-dashboard.search.brave.com/api-reference/web/search/get)
documents JSON results, a 600-character/75-word query limit and optional query
rewriting via spellcheck. Proposed integration: `count=5`, `result_filter=web`,
`text_decorations=false`, `spellcheck=false`, no automatic pagination or extra
callbacks. Keep the exact authored query; reject an oversized request explicitly,
never shorten it. Explain any region/language/filter defaults in the operator
preview. Results are suggestions for further reading, not full-page evidence.

The [published Search pricing](https://api-dashboard.search.brave.com/app/plans)
checked on October 22 lists $5 per 1,000 requests and $5 monthly credits on its
prepaid Search product. Illustrative combined usage across both beings:

| Searches/day | Searches/30 days | Gross cost | After $5 credit, if applicable |
| --- | --- | --- | --- |
| 100 | 3,000 | $15 | $10 |
| 500 | 15,000 | $75 | $70 |
| 1,000 | 30,000 | $150 | $145 |

These are arithmetic examples, not observed demand, a quote for storage rights,
or spending approval. Confirm actual checkout terms. Any operator-approved money
limit is an account resource boundary, not a renewed per-inquiry research quota.

**Retention is the material unresolved fit question.** Brave's
[September 1 terms](https://api-dashboard.search.brave.com/documentation/resources/terms-of-service)
restrict non-transient storage of search results and include restrictions on
AI-model evaluation/training uses. Its
[official API information](https://brave.com/search/api/)
states that storage requires a plan explicitly granting those rights. Our code
persists result snippets in research history and may later revisit them. Do not
silently delete that history or assume standard API access covers retention.
Obtain explicit terms covering retained results, inference, ordinary research
inspection and any planned evaluation use before activation. This is a concrete
contract question for the provider, not a legal conclusion about every use.

Credential design: an operator-owned secret outside Git and authored workspaces,
attached only to the exact `api.search.brave.com` HTTPS endpoint. Reject redirects
on credentialed calls rather than forwarding the credential. Public returned
links continue through the existing credential-free page reader. Preserve the
one-operation-per-owner lock, 4 MiB cumulative body bound, 120-second logical
deadline and tighter hop timeouts. Do not claim zero data retention: only selected
public search queries should leave the machine; private prose must not become a
query implicitly. Secrets and raw service errors never belong in routine logs.

Qualification should cover typed empty results versus malformed responses,
401/403, 429, timeouts, result identity, oversized/chunked/compressed data,
credential redaction and destination changes. A bounded approved public-query
pilot should check useful results, cost and latency, followed by immutable paired
release review. Current grants explicitly exclude authenticated/paid services;
evaluation approval alone does not broaden them. No Brave API call, key lookup,
account creation, subscription or new grant occurred here.

## Follow-through: Dependable Machine Search

Mike emphasized that reliable searching is the opportunity, not merely better
challenge diagnostics. The recommended route remains one explicitly configured
machine-search API, with Brave the first candidate. Do not replace the challenge
with browser automation, proxy rotation or an unreviewed public metasearch proxy.
An API avoids dependence on this human-facing HTML form; it does not guarantee
availability, result quality or unrestricted use.

The integration is bounded by existing code, not a new research framework:

1. `crates/astrid-public-web/src/transport.rs::request_url` currently fixes every
   search to DuckDuckGo. Introduce operator-selected API search here or in a
   closely scoped sibling transport, retaining the existing bounded fetch
   machinery and separate credential-free page reading.
2. `capsules/spectral-bridge/src/llm/provider/research.rs::web_search_checked` and
   Minime's `runtime.py::_web_search` currently parse that HTML independently.
   Return a shared typed result envelope with provider identity, exact query,
   ordered title/URL/snippet results and delivery provenance. Python should not
   grow a second provider-response parser. Keep provider snippets distinct from
   subsequently fetched source pages and from optional model summaries.
3. Both callers currently treat an empty parsed hit list as failure. The new
   contract must distinguish a verified zero-result response from malformed
   data, challenge, authentication failure, rate limit and service outage.
   Preserve the chosen action and do not substitute a reflection or local study.
4. Repeated ordinary searches need no experiment binding, inquiry quota or
   per-query human approval. The operator configures account, allowed endpoint,
   credential and spending once. Existing network/resource checks remain;
   provider throttling is factual service state, not a veto on a research topic.
5. Authenticated search is not covered by the current credential-free v2 grant
   (`crates/astrid-minime-protocol/src/volition/public_web.rs`). Add an explicitly
   approved narrow API capability, without changing page-read authority or
   manufacturing approval from old grants. Keep credentials out of authored
   commands, result bodies, redirects and routine logs.

Brave's [official FAQ](https://brave.com/search/api/) explicitly requires a plan
granting storage rights for retained results. Prepared operator question, **not
sent**:

> Which Search API plan or addendum permits two locally operated AI agents to
> retain returned titles, URLs and snippets in private research notebooks,
> revisit them during inference, and let their operator inspect those records?
> We are not reselling a search index or training a competing search model.
> Please distinguish ordinary application testing from any restricted model
> evaluation, and specify retention rights after subscription termination.

A bounded alternatives check did not establish a clearly preferable substitute.
[Exa's terms](https://exa.ai/terms) also contain copying restrictions requiring
careful reconciliation with API use. [Tavily's terms](https://www.tavily.com/terms)
distinguish service outputs but include provider input/output reuse provisions
and a restriction on disclosing service performance analysis. Neither was
declared retention-compatible or approved from marketing alone. Tavily's
[pricing](https://www.tavily.com/pricing) offers a no-card 1,000-credit monthly
trial, which makes it a possible alternative pilot, not automatic authorization.

Implementation can be qualified with synthetic API responses before an account
is installed. Live acceptance then requires a small approved public-query pilot
through both adapters: exact query delivery, useful structured results, truthful
zero-result/failure handling, source reading and retained evidence. No API
account, live search request, secret inspection, alternate-provider selection or
runtime change occurred during this follow-through. Existing diagnostic patches
remain candidates. This investigation adds no build or test claim.

## Remaining Boundaries

Immutable paired qualification and fresh rollout approval remain separate.
Larger writing capacity, MLX, providers, reservoir controls, sensory services and
paused automations are unchanged. No favorable report or increased research
activity is an acceptance condition. No letters were resent and no uptake
review was accelerated.

## Brave Implementation and Operator Configuration

Mike supplied a locally stored `BRAVE_SEARCH_API_KEY`, identified the account as
the **Search plan**, and approved **$25/month initially**, shared by both beings.
The credential value was never printed, sourced, passed as a command argument,
or copied to Git. A content-free scan located one literal assignment in
`~/.zshrc`. The first extraction attempt refused unrelated multiline shell
syntax; it did restrict the verified profile inode from 0644 to 0600 without
changing its bytes. Subsequent installation/check outcomes are recorded below.

The official [Search pricing](https://api-dashboard.search.brave.com/app/plans)
was checked again: $5 per 1,000 requests, prepaid, with a $5 monthly credit.
Accounting uses the gross $0.005/request rate and does not assume the credit is
unused. The chosen $25 ceiling therefore admits at most 5,000 locally reserved
requests per UTC calendar month, shared across Astrid, Minime and the operator
check. This is a local integration ceiling, not a verified Brave-dashboard cap;
other applications using the account are outside its accounting. Taxes or
changed pricing are not represented as known billing amounts.

### Contract and Boundaries

- V2 grants/configuration serialize unchanged when the new optional fields are
  absent. V3 explicitly selects a separately signed Brave policy and capability
  resource, account identity, reviewed price, spending ceiling and retention-rights
  audit reference. Possession of a key does not manufacture storage permission.
  The existing exact operator approval binds the configuration paths.
- Queries remain literal data. The fixed HTTPS API origin receives the sensitive
  credential header; page reads and search-result links do not. API redirects
  are refused, without forwarding credentials or attempting an alternate host.
- The existing public-address pinning, stream/decompression ceilings, owner
  flight lock, deadlines, revocation and storage checks remain. No additional
  inquiry quota or inherited experiment allowance is introduced.
- API options are explicit: five web candidates, US/en/en-US, moderate Safe
  Search, spellcheck and text decorations disabled. No automatic pagination.
  The documented 600-character/75-word query limits fail explicitly.
- Shared Rust parses provider JSON into `astrid.search.results.v1`; Python
  validates only that normalized envelope. Both adapters distinguish empty
  success from incomplete delivery. Links/snippets are navigation candidates,
  not verified page reads or causal evidence.
- Receipt hashes bind the normalized body. Wire/decoded counters still measure
  the bounded raw provider response; they are not fabricated normalized sizes.
- A cross-process account lock protects atomic, fsynced gross-cost reservations
  before each outbound paid request. Failed or uncertain attempts remain counted;
  missing/corrupt state, mismatched identities or backward calendar movement
  cannot silently reset an allowance. Initialization is an explicit operator
  operation with a durable marker, never an effect of fetching.
- `brave-check` uses one fixed public query, the same bounded network/parser,
  and the same cost account. It returns only status and result count, retains no
  response content, installs no grant and makes no model call.

The [official FAQ](https://brave.com/search/api/) and
[terms](https://api-dashboard.search.brave.com/documentation/resources/terms-of-service)
still make non-transient result storage conditional. The Search plan name alone
does not establish that right. No production Brave policy has been signed or
installed, and no provider result has been placed in a being's notebook. This is
an external account question, not another experimental or cognitive gate.

### Qualification Progress

The build-cache audit passed for the two explicitly reused cache roots before
compilation (about 2.82 GiB and 5.77 GiB; 391 GiB free). Debug/test symbols and
incremental compilation remain disabled for this qualification; immutable
release profiles and binaries are untouched.

Initial new test compilation found missing test-only imports and a test use of
`unwrap_err` requiring a payload-bearing Debug implementation. Tests now inspect
the error without adding that Debug surface. Initial strict Clippy identified
small style issues and an overlong fetch function; normalization/redirect
validation was factored into cohesive helpers. Those attempts are not counted
as passing qualification. Final counts and remaining release work follow below.

### Actual Operator Check

The explicit literal-selection import succeeded from line 631. This is extraction
of the operator-designated assignment, not a claim about shell execution order.
The credential is now at
`/Users/v/.config/astrid/credentials/brave-search-api-key`, mode 0600 in a private
directory. No value or credential digest is included here. The separate shared
account is `/Users/v/.config/astrid/brave-search-account`, initialized for
`mike-brave-search-20261022` with `monthly_limit_microusd=25000000`.

Three fixed-query transient checks were made:

1. An incomplete HTTP response; the first operator-check implementation omitted
   failed-hop status. That diagnostic defect was repaired with a regression test.
2. HTTP 422, 416 received bytes, no redirects. No upstream prose was retained.
   The official API reference exposed a mismatch: its `Accept` header supports
   `application/json` or `*/*`, unlike the generic page transport's multi-format
   header. Authenticated API requests now request JSON explicitly.
3. HTTP 200, `authenticated=true`, `result_count=5`, `results_retained=false`.
   No titles, URLs, snippets or raw response were written, logged or supplied to
   a model. The source-check binary SHA-256 was
   `4c866241a33a1d228883fa7986480f3ede9f77ffed8f73ab508a37a1be00d626`.

The account status recorded three reservations, gross $0.015, against $25 for
UTC October 2026. This is not a claim that Brave billed all three calls.
The subsequent testability-only extraction of request construction is covered
by synthetic header/credential tests; that later source identity was not
represented as the binary used for the successful network check.

The complete Minime candidate suite passed **2,622 tests, 341 subtests, one
existing skip**, using an isolated environment and the explicitly selected
source-reader helper. No API key entered that environment. Its 123.83-second
run is a source-candidate result, not immutable release qualification.

Canonical trees remain clean, with no changes staged, committed or merged by
this tranche. The controller remains paused at generation 535 with no lease or
active projection. No service was restarted, no paid-search grant installed,
and no letter or automatic uptake review was sent. Existing releases, provider
processes, reservoir controls and larger-capacity work remain untouched.

### Final Source Qualification

- Shared public-web crate: **59 library tests plus one CLI test pass**; one
  standalone process fixture remains ignored and is exercised by parent tests.
  Includes literal/Unicode queries, absent/null/empty result lists, authentication
  and rate refusals, header negotiation, forbidden credential destinations,
  non-forwarded redirects, concurrent cost reservations, corrupt/missing cost
  history, calendar rollback and content-free transient checks.
- Protocol: **61 tests pass** (31 unit, five substrate, 25 wire), including old
  wire shape and signed v3 separation.
- Provisioning CLI: **17 tests pass**, including both owners, old v2 exact
  serialized bytes/signatures, v3 fields and exact renewal/revocation validation.
- Credential installer: **23 synthetic tests pass**. Explicit line selection
  handles unrelated multiline profiles without evaluating them. Duplicate or
  ambiguous references, expansion, heredoc markers, unsafe paths/permissions,
  conflicting installed values and publication races remain tested.
- Complete bridge library: **2,554 pass, zero failures, three existing ignores**.
  Ten focused search-delivery tests cover structured results and the dispatch
  follow-through. Exact literal queries now survive SEARCH/RESEARCH aliases and
  explicit experiment binding. Empty search success releases the accepted search
  correctly, leaves the existing reading position intact and creates no failure
  reflection. Routine response-body preview logging is removed from enrichment.
- Complete Minime suite: **2,622 pass, 341 subtests pass, one existing skip**, as
  recorded above. Storage, source selection and authored-writing behavior are
  not claimed to improve because these tests passed.
- Strict Clippy passes for shared transport/protocol, provisioning and all
  bridge targets. Parent's final formatting checks pass for the same scopes;
  an earlier worker formatting concern did not reproduce in the final check.
  Domain-boundary audit has zero violations without snapshot changes. Both
  candidate trees pass `git diff --check`.

Initial standalone provision/bridge `--locked` attempts refused lockfiles that
lacked the two new dependency edges. Refresh added only `chrono` and `zeroize`
to `astrid-public-web` in each of the three lockfiles. An offline bridge refresh
also proposed an unrelated `cpufeatures` downgrade; that proposal was removed,
and final locked qualification preserves 0.3.1. These were build-preparation
attempts, not changes to live dependency identities.

Final shared-source SHA-256 witnesses:

| Path | SHA-256 |
| --- | --- |
| `crates/astrid-public-web/src/brave.rs` | `d526ac2ca5f2bbce35b25aaa2de83998c75658e76d9fbaf3951999ac0bc8c936` |
| `crates/astrid-public-web/src/spending.rs` | `690cb76d4b4a414dbcdd5bcc4019e5b35afb440dabd2f71e6e74a6e7e1ece494` |
| `crates/astrid-public-web/src/network.rs` | `b37b00e78ec385fae3fd62e42fdb7beb6f73c47576ba064fc2018f04da7079f9` |
| `crates/astrid-public-web/src/transport.rs` | `ea079b5b47e0094e7ced22cd7bf1f34a2e2fd51053cc86641371aa708554c44f` |
| `scripts/brave_search_credentials.py` | `d941cb4ccf23b671b74bd9eba1d639f3bd68f18d191bd87bd3da50a7b5d62b73` |
| `crates/astrid-minime-protocol/src/volition/public_web.rs` | `aa11fd2f3257d843171c6408f611a9d012316f2712c4fe5111600d514648693c` |

The closing cache audit remains within budget: target-web 3,145,891,840 bytes,
target-bridge 6,335,565,824 bytes; volume free space 416,677,744,640 bytes.
Free space fell 3,510,505,472 bytes from the initial snapshot, a whole-volume,
non-atomic observation rather than attribution to this build. No cache was
deleted: these exact debug helpers/caches remain referenced by ongoing candidate
qualification. No new target root or release profile was created.

PID/start-time inspection confirmed the same bridge 59199, agent 56428, engine
35303, provider 77735, camera 3914, microphone 3879, visual service 95786,
host-sensory 94952, feeder 3878, gateway 86873 and supervisor 86833. No live
configuration points at this candidate's paid-search policy because none exists.

Remaining: confirmation of account storage rights; reconciliation with the two
new main tooling commits; immutable paired packaging/preservation qualification;
reviewed v3 policy installation and graceful adapter activation; coordinated
Git integration. Do not describe the successful operator check as either being
having used Brave, or promise search adoption. No automatic letters or retries
are scheduled.

## Informed Activation Decision

On October 22, Mike requested deployment and merge, then explicitly selected:
"Enable; record storage rights as unverified" after being told that existing
notebooks retain search titles, URLs and snippets. The relevant approval is
`call_3e1119ee6b8b4d4f8560b20850d1da3c`. The shared $25/month gross local cap
remains unchanged. This is operational authorization, not evidence that Brave
has granted persistent-storage rights. The FAQ and terms remain referenced
above; account entitlement has not been independently established.

The as-yet-unissued v3 policy now records typed `storage_rights_status` and
`retention_review_audit_ref`, rather than requiring a misleading confirmed-rights
attestation. Production preparation must use `unverified`. Tests bind the status
to the signature and reject missing, unknown or implicitly inferred status.
No previous grant or retained authored state is rewritten by this adjustment.

The October 22 source tests above remain historical qualification evidence.
The renewed shared protocol/transport tests pass after this adjustment (61
protocol tests, 59 transport tests plus one CLI test). Final immutable release
and actual activation evidence will be recorded separately below.
