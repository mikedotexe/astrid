# Perception memory investigation and staged containment

Status (October 22): **short-lived camera worker deployed to the live perception
LaunchAgent after actual-camera acceptance; the two-hour footprint observation
is complete**. The original `perception.py` is retained at
`.runtime/perception-memory-investigation/deployment-01-original-perception.py`
with its original SHA-256 and a bounded rollback procedure. The live change is
only the reviewed `capture_frame` delegation and new `camera_capture.py` helper;
the existing LaunchAgent, camera index, microphone, cadence, workspace and
authored history were retained. The actual-camera fixture produced decodable
1280×720 JPEGs from both implementations and reaped the candidate child. The
deployment then produced fresh visual, ASCII and audio records under the new
process, with the MLX provider ready and reservoir-connected. See the private
`actual-camera-window-01.json`, `deployment-01.json`, and
`deployment-operator-review-01.json` receipts. This establishes initial camera
compatibility, **not** a long-term memory or leak-cause claim. The two-hour
read-only `post-containment-rusage-01.json` observation completed with 121
samples under one process identity. Physical footprint ranged from
41,452,120 to 51,806,808 bytes; first/last ten-sample medians were 46,121,560
and 49,005,144 bytes. New records within that window included 113 visual,
136 ASCII and 20 audio artifacts (metadata counts, not content assessment).
This supports bounded two-hour containment, not lifetime stability or proof of
the allocation cause. Exact final interpretation and source hashes are in
`.runtime/mlx-upgrade/stage2-0323-01/observer-closeout-01.json`; no repeat timer
or automatic restart is requested.

Earlier investigation status: the candidate was isolated and not deployed when
the following evidence was written. The MLX passive baseline completed
with all 119 bound files unchanged, but failed its reservoir health gate.
No camera, microphone, model or live
profiler was invoked for this investigation. The earlier one-time perception
recovery preceded this baseline; it must not be repeated automatically.

## What the evidence establishes

The previous perception process, PID 3895, ran from September 30 until October
15. Its retained memory report showed a **10.7 GiB physical footprint**, including
substantial swapped memory. This is not 10.7 GiB of resident RAM. DefaultMalloc
held **39,699,536 allocations**, 4.3 GiB allocated and 1.6 GiB fragmentation.
Replacing that process with the same code reduced its footprint to approximately
102 MiB five minutes later, while saved history remained intact. The allocation
callers were not captured, so the exact leak is **not established**.

A static audit of the complete perception loop found bounded Python history
(`deque(maxlen=25)` holding scalar features), no growing retained frame list, and
no persistent local MLX model. Vision HTTP responses and image objects are local;
audio transcription and rendering already use subprocesses. The repeatedly
opened native camera path is the leading investigation target, not a proven
cause. The old process's exact log prefix contains 21,293 successful vision
calls, 66 vision-provider failures, and no logged camera-access or visual-path
exceptions. Those counts do not cover every silent frame-read failure.

The existing camera function skips explicit `release()` on some failure paths.
Eight isolated characterization tests confirmed that cleanup gap. However,
normal successful captures already release the camera, and the installed
OpenCV 4.13.0.92 implementation also releases captures on destruction. Its macOS
backend already drains local autorelease pools during camera open, grab and stop.
A missing Python `finally` or an extra caller-thread autorelease pool therefore
does not explain the observed growth on its own. See the exact upstream
[camera implementation](https://github.com/opencv/opencv/blob/b4c5ec4042f097e2a5b386b9d413ec7333d0a184/modules/videoio/src/cap_avfoundation_mac.mm)
and [capture destructor](https://github.com/opencv/opencv/blob/b4c5ec4042f097e2a5b386b9d413ec7333d0a184/modules/videoio/src/cap.cpp).

Comparing the relevant OpenCV 4.13 and 4.14 source did not identify a fix to this
camera lifecycle. Upgrading that dependency is not presently supported as the
repair. Complete source, package and evidence identities are in
[audit.json](audit.json) and [upstream-audit.json](upstream-audit.json).

## Prepared repair

The staged candidate delegates each existing synchronous capture to one
short-lived worker, executed with the exact running Python interpreter. Only
that worker imports OpenCV. Native allocations owned by a normally exiting
worker cannot accumulate in the long-lived perception process across captures.
This is containment of a suspected allocation path, not proof that all future
perception memory growth is prevented.

- Preserve camera index, default backend, resolution, JPEG settings, cadence,
  audio behavior, model settings and history handling.
- Release the camera in `finally`, check capture and JPEG-encoding success, and
  require the exact child to exit successfully and be reaped before accepting
  its output. The caller starts one worker at a time.
- Reserve a non-inheritable binary descriptor for the JPEG and redirect native
  stdout diagnostics to inherited stderr before importing OpenCV.
- Limit retained JPEG payload to 16 MiB, drain and discard overflow, and reject
  missing JPEG start/end markers. This is envelope checking, not full decoding.
  The largest of 199 recent saved frames was 196,068 bytes; only file metadata
  was inspected, not image contents.
- Add no timeout, forced termination or retry. A wedged camera can still block
  as before. An immediate or repeated interrupt during `wait()` can interrupt
  reaping; this is not an all-signal orphan-prevention mechanism.

The isolated candidate is under
`/Users/v/other/astrid/.runtime/perception-memory-investigation/candidate/`.
The durable [implementation patch](camera-process-containment.patch),
[staged manifest](staged-containment-manifest.json), and
[archived fixture source and reconstruction instructions](fixture-source/README.md)
preserve the reviewed change outside ignored runtime storage.
Its `baseline/` directory preserves the original copied source and earlier
characterization inputs. The staged manifest maps old receipt paths and hashes
to those preserved bytes. Only the `capture_frame` function changes in
`perception.py`; the camera helper is new. No production file was patched at
candidate staging. The reviewed containment was subsequently deployed as
described in the current status above.

## Qualification completed and deferred

Twenty focused tests passed using fake camera modules and 18 small owned
subprocesses; all children were reaped. They cover cleanup, encoding failure,
bounded output, drainage beyond pipe capacity, native diagnostic separation,
descriptor ownership, exact interpreter selection and source isolation.
Independent reviews found no blocker to this isolated stage. See
[test evidence](containment-tests-02.json),
[candidate review](containment-review.json), and
[baseline isolation verification](containment-isolation-verification.json).

A separate owned Python 3.14 asyncio fixture passed all eight checks for the
first SIGINT: synchronous capture completed, its finite fake worker exited and
was reaped, then cancellation surfaced at the next await. It sent no signal to
production. See [graceful-cancellation evidence](first-sigint-fixture-01.json).

Passive OS accounting uses `proc_pid_rusage` with exact process identity guards;
it does not attach to or suspend perception. Short-term footprint fluctuations
must not be interpreted as a long-term leak rate. The initial ten-minute run
completed 21 samples without API or identity errors: footprint began at
133.17 MiB, peaked at 414.58 MiB, and ended at its 108.39 MiB minimum. Peaks
recovered within that window. This neither proves nor rules out slower growth.
See [passive memory evidence](passive-memory-observation-01.json). A separately
reviewed continuation records one sample per minute through the current MLX
baseline's completion. Its [launch handoff](passive-memory-continuation-02.json)
binds observer PID 37412 and a finite 5,520-second run ending near 03:21:59 UTC
October 16 (20:21:59 Pacific October 15). It stops on identity/API failure and
has no automatic repeat. Review its completed result after 03:22 UTC; do not
mistake the launch receipt for completed observation.

That continuation **completed at 03:21:59 UTC October 16** with 93 samples
covering 92 minutes and no accounting or process-identity errors. The initial
417.06 MiB footprint peak subsided to 110.00 MiB. Later readings showed upward
drift: ten-sample medians increased from 129.46 MiB in the first block to
180.05 MiB over minutes 80–89, and the last reading was 204.14 MiB. The initial
transient makes the whole-run endpoint decrease an incomplete description.
There was no 10.7 GiB recurrence during this window; this does not establish
long-term stability or identify the allocation source. The observer exited and
will not be repeated automatically. See the
[completed observation review](passive-memory-observation-02.json), including
raw-result hashes and the full sequence of block summaries.

The following was the original deferred actual-camera acceptance plan. The
October 22 deployment completed actual-camera acceptance, fresh live output
checks and the bounded footprint observation above. These historical planned
steps are retained for provenance, not a request to replay the deployment:

1. Revalidate stewardship, source hashes, process identity and all retained
   candidate/review artifacts. Keep the preceding source as rollback material.
2. Use an owned fixture in the actual launchd/user/interpreter context to verify
   camera authorization, camera selection, fresh fully decodable frames,
   geometry, ordinary latency, failure/reconnect behavior and complete child
   reaping. Avoid simultaneous competing camera capture. Matching interpreter
   identity alone does not establish macOS camera-permission equivalence.
3. Compare sequential normal-cadence captures with the original and candidate,
   preserving outputs privately. Record parent and child memory, capture count,
   elapsed time and host pressure after warmup. No forced termination or retries
   that hide failed captures. A short fixture pass does not establish a stable
   long-term footprint.
4. Only after hardware acceptance, deploy through a newly reviewed graceful
   perception transition. Preserve authored history and current journals;
   verify continuing vision, ASCII and audio output and protected service
   identities. Observe the parent over a sustained representative period and
   retain passive footprint samples. If growth continues outside the camera
   worker, investigate that path instead of adding automatic whole-process
   restarts.

MLX package promotion and this now-deployed containment remain separate claims;
neither the earlier recovery nor this containment establishes MLX attention-memory
savings. The associated periodic follow-up is paused.
