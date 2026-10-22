# MLX 0.32.3 controlled performance review — October 22, 2026

Historical decision record: the following describes the pre-activation review.
The later full 0.32.3 activation and prospective policy reset are recorded in
[the integration closeout](2026-10-22-mlx-perception-integration.md). Earlier
outcomes below remain unchanged; their then-current deployment statements are
not current status or a new request to repeat qualification.

This is a diagnostic decision record, not a package qualification proof or an
activation release. Astrid still runs MLX/Metal 0.32.0. The private 0.32.3
candidate was tested with Minime autonomous chat and Astrid perception paused
under a bounded, user-authorized hold; both were restored afterward. The provider,
reservoir, bridge, engine, Ollama server, history and existing holds were preserved.

The first matched comparison used a baseline batch in one controlled window and a
core batch in another. All 15 outputs and both in-workload memory gates passed,
but the official performance comparison found seven regressions, including
exact-output completion at 2.87 times baseline and `NEXT:` completion at 2.49
times baseline. Preserve that failed comparison at
`.runtime/mlx-upgrade/alternative-0323-01/source-rebind-01/qualification-02/performance-comparison-01.json`
(SHA-256 `7d61dde5f08ecf41b8c2a17f796438da9eca49f532e5a6b9599a7bb8cdd1911b`).

The one allowed reversed-order pair ran core then baseline back-to-back in a
single bounded window. Both batches passed all 15 output checks and their
in-workload memory gates; the official comparison passed every published
performance threshold. All 15 paired response texts and stop reasons were
identical. The exact two-attempt review is
`.runtime/mlx-upgrade/alternative-0323-01/source-rebind-01/qualification-02/performance-comparison-reversed-01.json`
(SHA-256 `e33a9bd7e97de2f6abc7a3e25f6a41d95c90e283e29ba4d46cd8bf8dabf48424`).
The final controlled-window closeout is
`.runtime/mlx-upgrade/alternative-0323-01/source-rebind-01/qualification-02/controlled-window-04/closeout-01.json`
(SHA-256 `d239b51505bd83336ebfeba89a641af623b474c992a4e560020ee7e35ddf81b0`).

The unchanged 0.32.0 baseline itself varied substantially between windows:
exact-output p95 completion was 1.31 seconds in the first and 3.59 seconds in
the reverse window; `NEXT:` was 4.32 and 14.61 seconds. Both batches recorded
pressure level 1 and no added swap. This establishes timing variability, but
does not identify the competing process, scheduler or thermal cause. The
reverse pass must not be presented as a measured explanation for the initial
failure. Controlled-load evidence also cannot establish ordinary-production
capacity while both paused requesters run.

The 20% p95 latency, 10% decode-throughput and 10% peak-allocation limits are
local acceptance choices, not MLX compatibility requirements. With three
repetitions, p95 is the slowest sample. Short exact outputs additionally make
tokens-per-second sensitive to small timing changes. These limits should be
evaluated against user-visible absolute latency and repeated same-window
variance before they are changed; do not choose a new threshold after seeing a
candidate result and call old evidence passing.

For future package intake, perform each baseline/candidate pair back-to-back
within one explicitly bounded host and requester state, record process/GPU
contention where possible, and limit the campaign to one initial pair plus one
reversed pair. A cross-window comparison is diagnostic, not a passing or failing
promotion attempt. Apply this prospective method to new evidence only; retain
the two historical 0.32.3 outcomes as recorded. If parallel model loading
cannot represent normal production because it duplicates model memory, prepare
a separate supervised single-provider canary with a reviewed rollback plan and
normal-load service-level checks. No such canary is authorized or deployed by
this note. Do not promote 0.32.3, 0.32.4 or Stage 2 from this record.
