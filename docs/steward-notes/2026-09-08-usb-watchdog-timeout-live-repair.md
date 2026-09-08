# USB inventory timeout repair is live - September 8, 2026

## Scope

This follow-through began as a read-only check that the newly pushed Astrid,
Minime, and reservoir histories still matched the running stack. It found a
separate live sensory-service defect and repaired that defect without changing
the reservoir, regulator, sensory cadence, fallback policy, language prompts,
or either language process.

## Finding

The Minime USB hot-plug watchdog treated every failed
`system_profiler SPCameraDataType -json` call as a successful observation of
zero cameras. A timeout therefore produced this false sequence:

1. the real LifeCam camera remained attached;
2. `system_profiler` timed out after eight seconds;
3. the watchdog converted the failed probe to an empty set;
4. it reported the known camera as removed and restarted both sensory clients;
5. the next successful probe reported the same camera as added and restarted
   both clients again.

The retained watchdog log shows repeated timeout, synthetic removal, synthetic
addition, and paired kickstart cycles through 09:23 PDT. Launchd had run each
client 364 times. A direct successful inventory still found both members of the
device:

- camera: `UVC Camera VendorID_1118 ProductID_1885`
- audio input: `Microsoft LifeCam Cinema(TM)`, USB, 48 kHz

This was a watcher observation bug, not evidence that the device was actually
removed.

## Repair

Minime commit `defd7c66e82d9d003d166a22f3bd8c72b3943ff5` changes
`scripts/usb_hotplug_watchdog.py` so command failures, timeouts, JSON failures,
wrong JSON roots, and missing expected category roots produce an unknown
observation. The watcher retains the last successful device set for that
category and emits a hot-plug transition only when two comparable successful
observations differ.

The repair preserves real unplug and replug handling. A successful observation
of an empty set still means removal; a later successful observation of the
device still means addition.

Verification:

- `python3 scripts/usb_hotplug_watchdog.py --self-test`: 9 passed
- `python3 -m py_compile scripts/usb_hotplug_watchdog.py`: passed
- `git diff --check`: passed
- direct `--once` inventory: one camera and one USB audio input

## Live activation

Only `com.minime.usb-hotplug-watchdog` was restarted. Its PID changed from
`41792` to `98932`. The then-current camera PID `98903` and microphone PID
`98910` were unchanged by activation. The bridge, Minime agent, Minime engine,
model, Division, host-sensory, and visual services were not signaled.

A three-minute canary sampled all three PIDs every fifteen seconds. They stayed
fixed. At 09:25:41 PDT, the new watcher encountered the same real eight-second
camera inventory timeout. It logged the failed probe but emitted no device
change and restarted neither client. This directly exercises the repaired live
boundary.

## Stack alignment

- Astrid bridge PID `90102` remains the signed SELF_STUDY parity release.
  Its binary SHA-256 is
  `7fef07299845be4d025ff8b4acf580c1b407816950b3df07ea63affebb97885e`.
- The deployed shared source reader SHA-256 is
  `6323a78ba5e32d590c4286c2d15b1e5c074efcbc64a31798c16371453c230cc9`.
- The live bridge manifest SHA-256 is
  `fa3f42e6d73319772205e1ec5dcc55f799f344201cf29feaf8ca63d11c720db0`,
  matching the activation receipt.
- Minime agent PID `91125` remains the verified rollout process. All 80 startup
  source hashes match disk and `reload_required` remains false.
- Coupled model PID `60333` reports both `/livez` and `/readyz` ready, with an
  empty queue and no last generation error.
- Division owns ports 7878-7880, the Minime engine owns 7900-7902, and the
  coupled model owns 8090.
- Fresh telemetry continued during the canary. Fill was approximately 73.0%
  against the unchanged 68% target.

The Astrid and Minime changes after their deployed SELF_STUDY implementation
commits are tests, documentation, and this interpreted watchdog source. The
watchdog source is now loaded by PID `98932`. The reservoir changes after its
deployed collaboration source are changelog-only.

## Physical fallback interval and natural recovery

Stopping the false restart loop did not make the attached LifeCam deliver
frames or audio chunks. At the close of observation, both clients were alive
and connected but had zero successful samples. `sensory_source.json` therefore
reported physical camera and microphone unhealthy and selected the existing
host fallback. Fresh engine and bridge telemetry continued.

The observation was extended while the controller completed its integrity
verification. By 09:36:56 PDT, the same unchanged client PIDs had recovered
without operator action:

- camera PID `98903`: `streaming`, healthy, 81 frames, no last error
- microphone PID `98910`: `streaming`, healthy, 1,004 chunks, no last error
- `sensory_source.json`: both lanes `source=physical` and
  `physical_healthy=true`

No unplug, daemon reset, client kickstart, fallback-policy change, or reservoir
change occurred. The evidence is consistent with the restart storm having
denied the clients enough uninterrupted time to recover, but it does not prove
that as the only cause.

If the same hardware condition recurs, the first safe manual recovery step
remains a physical unplug/replug of the LifeCam. The repaired watcher should
observe a real successful removal followed by a real successful addition and
restart the clients once per transition. Verify the result with:

```bash
cd /Users/v/other/minime
python3 scripts/sensory_source_check.py --json
```

Do not reset CoreAudio, camera daemons, USB state, or reservoir controls merely
to make the status appear healthy. If the device still enumerates without
samples after a physical replug, investigate macOS device/TCC ownership as a
separate operator task.

## Authority boundary

This receipt establishes a software defect, its bounded repair, and live
process evidence. It does not infer what Astrid or Minime perceived, whether
host fallback is experientially equivalent to physical input, or whether any
later report is caused by this repair. No message, study, reservoir setting,
controller setting, sensory cadence, or fallback-selection rule changed.
