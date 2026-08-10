# Immutable edge web broker contract

`astrid-edge-web-broker` is a small root-installed trust-boundary program. Two independent
instances perform bounded read-only web search and source retrieval for exactly one local client
each. The program has no filesystem write, shell, process, package, source, deployment, model,
capsule, IPC, or arbitrary network authority.

## Listener and client separation

Local traffic uses two systemd-owned `AF_UNIX` stream sockets:

| client | socket unit | service unit | exact path |
|---|---|---|---|
| mutable edge runtime | `astrid-edge-web-broker-runtime.socket` | `astrid-edge-web-broker-runtime.service` | `/run/astrid-edge-self-change/web-runtime.sock` |
| immutable steward | `astrid-edge-web-broker-steward.socket` | `astrid-edge-web-broker-steward.service` | `/run/astrid-edge-self-change/web-steward.sock` |

Each socket has `Accept=no`, `SocketUser=root`, mode `0660`, and a client-specific group. The
installer resolves the expected client UID and socket GID, writes those numbers into that
listener's immutable configuration, and installs the socket with the same GID. The parent
directory is root-owned and not group/world writable. Each service receives exactly one
listening socket on standard input and refuses every other path, owner, group, mode, link count,
or file type.

On Linux, the broker reads `SO_PEERCRED` before allocating an authentication worker and requires
the exact configured client UID plus a positive peer PID. Header identity and the request HMAC
must then agree with the listener's fixed client identity. Runtime and steward have distinct
processes, listener queues, pre-authentication pools, authenticated concurrency pools, replay
partitions, and request keys. A valid or unauthenticated runtime client therefore cannot consume
the steward's broker capacity, and vice versa. Restarting either service does not remove its
systemd-owned socket or interrupt the other broker.

## Exact local protocol

Both sockets carry the same deliberately strict HTTP/1.1 framing:

```text
POST /v1/search HTTP/1.1
Host: astrid-edge-web-broker
Content-Type: application/json
Accept: application/json
Connection: close
Content-Length: <canonical decimal, at most 4096>
X-Astrid-Web-Client: <edge-runtime|edge-steward>
X-Astrid-Web-Nonce: <64 lowercase hex; first 16 encode Unix milliseconds>
X-Astrid-Web-Auth: <64 lowercase hex HMAC-SHA256>
```

No other method, path, version, header, transfer encoding, duplicate header, folded header,
pipelined bytes, or ambiguous framing is accepted. A search body is:

```json
{"schema":"astrid.edge.web_search.request.v2","trace_id":"11111111-1111-4111-8111-111111111111","query":"reservoir entropy","limit":5}
```

The HMAC binds authentication protocol v2, fixed client identity, path, exact logical Host value,
nonce, and SHA-256 of the exact body. A nonce is accepted only inside the compiled freshness
window and once for that listener. Replay state is bounded to 2,048 nonces and fails closed.
Possession of one request key cannot authenticate on the other socket.

`trace_id` is an exact lowercase canonical UUID. `query` is trimmed ASCII research text of at
least two and at most 24 bounded tokens. The broker rejects URLs, paths, assignments,
credentials, hashes, UUIDs, long digit runs, mixed-class encoded tokens, controls, and unknown
punctuation. The runtime listener admits at most 8 searches in a rolling hour and 24 in one UTC
day; the steward listener admits at most 2 per rolling hour and 12 per UTC day. No trace may
authorize more than two searches. `limit` is from 1
through the configured maximum and never more than 5. Unknown JSON fields are rejected.
Useful research must remain free-form, so ordinary-looking topic words still constitute a
low-bandwidth residual covert channel. Syntax filtering cannot eliminate that semantic risk;
host isolation, the absence of secret roots from the client namespace, exact trace-bound fetch
grants, and the persisted hourly/UTC-day ceilings bound it without claiming perfect
noninterference. Query text and web results remain untrusted and cannot authorize an Action,
candidate, build, activation, or continuity admission.

A successful search response is:

```json
{
  "schema":"astrid.edge.web_search.response.v1",
  "results":[{"title":"...","url":"https://...","snippet":"..."}]
}
```

Every authenticated response contains the exact client identity and nonce, canonical request
hash, and a 128-lowercase-hex Ed25519 signature. Its signature binds protocol v2, client identity,
nonce, status, request hash, and response-body hash. Only either broker instance receives the
shared response-signing seed. Clients receive only the corresponding public verify key. An
unsigned error is never trusted as a broker result.

Title, URL, and snippet bounds are 200, 2,048, and 500 characters. They are untrusted public-web
metadata; instruction-like text remains inert data.

`POST /v1/fetch` accepts:

```json
{"schema":"astrid.edge.web_fetch.request.v2","trace_id":"11111111-1111-4111-8111-111111111111","url":"https://example.org/article","max_chars":8000}
```

Fetch accepts only an exact canonical HTTPS URL returned by the same authenticated client and
trace's search metadata during the preceding 30 minutes. The ephemeral grant also retains the
exact query hash and result index, is single-use, is capped at 128 grants, and disappears on a
service restart. A different client, trace, URL, replay, or expired grant fails closed. Fetch
rejects credentials, fragments,
private/reserved IP literals, local DNS suffixes, redirects, encoded bodies, ambiguous framing,
PDFs, binary content, and more than one MiB of upstream bytes. The response contains only the
canonical URL, status 200, original byte count, truncation flag, and at most 65,536 characters of
readable text. Executable and page-chrome regions are removed. Returned text remains explicitly
untrusted evidence. Headers, cookies, credentials, and raw upstream bytes are never returned or
written.

## Upstream network policy

The only configured search upstream is `https://search.brave.com/search` on port 443. Search
callers cannot select an upstream URL, host, path, method, header, proxy, environment value,
redirect policy, or command. Fetch callers provide only the bounded public HTTPS URL previously
granted by search. The client:

- uses WebPKI roots and HTTP/1.1;
- disables environment proxies, redirects, content decoding, and idle connection reuse;
- rejects the entire DNS answer set if empty or containing private, loopback, link-local,
  multicast, documentation, benchmark, mapped-private, or reserved addresses;
- accepts only exact HTTP 200 readable content with unambiguous bounded framing; and
- enforces connect, header, total, and one-megabyte body limits.

The service sandbox independently blocks known local and private destination ranges. Local client
access needs only `AF_UNIX`; public egress is confined to the immutable broker processes.

## Per-listener configuration

Each root-owned mode-`0440`, nlink-one configuration uses
`astrid.edge.web_broker.config.v3` and exactly these fields:

```text
schema
client_id
socket_path
expected_peer_uid
socket_gid
upstream_origin
connect_timeout_ms
header_timeout_ms
total_timeout_ms
client_read_timeout_ms
client_write_timeout_ms
maximum_request_body_bytes
maximum_upstream_body_bytes
maximum_results
maximum_concurrent_requests
maximum_searches_per_hour
maximum_searches_per_utc_day
quota_state_path
request_key_path
request_key_sha256
response_signing_key_path
response_signing_key_sha256
response_verify_key_sha256
```

The exact files are:

- `/etc/astrid-edge-self-change/web-broker-runtime.json`, binding `edge-runtime` to
  `/run/astrid-edge-self-change/web-runtime.sock` and the appliance runtime UID/GID;
- `/etc/astrid-edge-self-change/web-broker-steward.json`, binding `edge-steward` to
  `/run/astrid-edge-self-change/web-steward.sock` and `astrid-edge-steward` UID/GID.

Before sending a search upstream, each immutable broker durably appends a body-free hash-chain
record to its own mode-`0600` StateDirectory ledger. The record binds client, trace, request hash,
sequence, UTC day, and admission time, but never stores the query. Restart reloads the exact
ledger, so neither restart nor nonce-cache loss resets the budget. A partial final write is
removed; malformed complete records, hash-chain changes, client/path reuse, clock rollback, or
storage failure make search fail closed. Runtime and steward ledgers are in separate inaccessible
StateDirectories and cannot be substituted across listeners.

Each instance receives one distinct 32-byte request key and the broker-only response-signing seed
as systemd credentials. The runtime and steward each receive their own request key and the shared
raw public verify key; neither receives the signing seed or the other's request key. Credential
paths, exact 32-byte length, mode `0400`, and lowercase SHA-256 identities are pinned in the
configuration. Keys never appear in argv or environment variables.

The installer creates or verifies the response keypair with:

```text
/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker \
  --key-init \
  --signing-seed /etc/astrid-edge-self-change/web-response-signing.key \
  --verify-key /etc/astrid-edge-self-change/web-response.pub
```

The paths must be distinct children of one root-controlled non-writable, non-symlink directory.
Creation uses OS entropy, non-replacing creation, and directory fsync. Existing exact files are
verified idempotently; links, hardlinks, or identity/mode/length mismatches fail without
replacement. Output contains hashes and creation flags only.

## Required systemd integration

The two socket units each identify their one service explicitly:

```ini
[Socket]
ListenStream=/run/astrid-edge-self-change/web-runtime.sock # steward unit uses web-steward.sock
SocketUser=root
SocketGroup=<resolved exact client group>
SocketMode=0660
# Runtime and steward have unrelated groups. The shared parent contains no
# secret and is root-owned/non-writable; traversal is required for both.
DirectoryMode=0755
Accept=no
Service=astrid-edge-web-broker-runtime.service # or ...-steward.service
```

The matching service uses the immutable broker identity and the one activated socket:

```ini
[Service]
Type=simple
User=astrid-edge-web
Group=astrid-edge-web
StandardInput=socket
Sockets=astrid-edge-web-broker-runtime.socket # or ...-steward.socket
ExecStart=/usr/libexec/astrid-edge/immutable/astrid-edge-web-broker \
  --config /etc/astrid-edge-self-change/web-broker-runtime.json
LoadCredential=request.key:/etc/astrid-edge-self-change/runtime-web-request.key
LoadCredential=response-signing.key:/etc/astrid-edge-self-change/web-response-signing.key
UMask=0077
NoNewPrivileges=yes
CapabilityBoundingSet=
AmbientCapabilities=
PrivateTmp=yes
PrivateDevices=yes
ProtectSystem=strict
ProtectHome=yes
ProtectHostname=yes
ProtectClock=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes
RestrictNamespaces=yes
RestrictRealtime=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
SystemCallArchitectures=native
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
```

The steward instance substitutes its socket, service, config, and request-key source. Neither
service gets a writable directory, environment file, proxy variables, source/state/operator-home
access, SSH material, or deployment credentials. The runtime and steward unit dependencies name
their respective socket units, never a shared TCP broker service.
