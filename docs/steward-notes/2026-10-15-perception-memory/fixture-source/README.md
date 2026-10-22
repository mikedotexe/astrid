# Retained fixture source — do not execute here

These files are exact archival copies of the reviewed isolated tests and owned
first-SIGINT fixture, plus the original source/provenance needed by their layout.
No archival copy was executed in this directory. The final staged manifest binds
all bytes and their original paths.

To reconstruct inside a new isolated root, create `capsules/perception/baseline/`
and copy the archived `baseline/` contents there. Copy archived
`baseline/perception.py` and `baseline/host_audio_fallback.py` into the parent
`capsules/perception/` directory too. Their expected original SHA256 values are:

- perception.py: 3b7baa2265373a39e8b46c8ad5a33781b21a86abf23d5959dc2393b53427a57a
- host_audio_fallback.py: aefe5019b21cba231c9c2d8722397667bce5b2e275e5838769b3e44be625bc69

Verify these hashes before applying `../camera-process-containment.patch` with
`patch -p1` from that isolated root. The patch modifies only `perception.py` and
adds `camera_capture.py`; it does not change the preserved `baseline/` copies.
Verify the resulting hashes against the final staged manifest. Copy the archived
`test_camera_capture_containment.py` and `first_sigint_fixture.py` beside those
resulting files. Only then run the tests using Python `-I -B`, with fresh evidence
output paths. The first-SIGINT fixture additionally requires a new `--work-dir`.

The tests require this candidate/baseline layout and must not run in this archive
or against the live perception directory. Neither production perception module
nor the archived original module should be imported: both have top-level
filesystem side effects. The tests extract the needed function via AST and use
fake workers, not real OpenCV, cameras, microphones, networks or models.

The first-SIGINT fixture signals only its own freshly spawned asyncio parent
once and uses a finite fake worker. It does not establish repeated-interrupt or
hardware behavior. Real-camera/TCC and sustained-memory acceptance remain deferred.
