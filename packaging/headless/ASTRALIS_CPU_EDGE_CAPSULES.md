# Pinned Astralis compatibility capsules

The CPU-edge appliances use four external Astralis capsules whose SDK 0.6 ABI
must remain compatible with the Astrid 0.5 lifecycle linker:

- ReAct;
- prompt-builder;
- OpenAI-compatible provider;
- context-engine.

Their source is not vendored. `astralis-cpu-edge-capsules.toml` pins full
upstream commit IDs, every compatibility-patch SHA-256, exact final Git blob
IDs, Rust 1.94.1, `wasm32-wasip2`, and one reviewed Cargo lockfile per capsule.
The three ABI-sensitive crates (`astrid-sdk`, `astrid-sdk-macros`, and
`astrid-sys`) must each occur exactly once at 0.6.0.

Run the offline integrity checks without contacting upstream or compiling:

```bash
python3 scripts/build_astralis_cpu_edge_capsules.py --verify-only
python3 -m unittest scripts/test_build_astralis_cpu_edge_capsules.py
```

Build every archive from fresh pinned checkouts:

```bash
python3 scripts/build_astralis_cpu_edge_capsules.py \
  --output-dir dist/astralis-cpu-edge
```

For an air-gapped build, place clean Git mirrors in
`SOURCE_ROOT/{react,prompt-builder,openai-compat,context-engine}` and ensure the
locked crates are already in Cargo's cache:

```bash
python3 scripts/build_astralis_cpu_edge_capsules.py \
  --source-root SOURCE_ROOT \
  --offline \
  --output-dir dist/astralis-cpu-edge
```

The runner never installs or deploys a capsule. It emits deterministic
two-entry `.capsule` archives plus `MANIFEST.json`; deployment remains a
separate transactional operation. It refuses unrelated output files so a
manifest cannot silently omit a stale archive. React's terminal provenance
patch has an additional byte-exact preimage gate. Local safe fallback and
executor errors therefore cannot regain authored provenance through a fuzzy
patch replay.
