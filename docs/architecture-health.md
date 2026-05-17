# Architecture Health

The old 1000-line file rule has graduated from a hard stop into an architecture-health signal.

Large files are not automatically wrong. They are a prompt to ask whether the code still has a clear center of gravity, a stable owner, and a reviewable test surface. If the answer is yes, the file can stay large with a short explanation. If the answer is no, split it by responsibility before adding more behavior.

## Review Standard

Prefer source files under 1000 lines. For larger files, reviewers should ask:

- Does the file describe one cohesive subsystem or several unrelated concerns?
- Would extracting a module reduce real coupling, or only move code around?
- Are the public entry points clear enough for another maintainer to use safely?
- Is the risky behavior covered by focused tests?
- Is this a generated file, fixture, schema table, long-form doc, or deliberate registry?

Files above 1500 lines should have an explicit note in the PR, issue, or nearby module documentation. Files above 2500 lines should usually get a decomposition plan unless they are exempt.

## Healthy Exceptions

Acceptable exceptions include:

- generated code or vendored fixtures;
- long-form architecture notes and audits;
- schema tables and static registries where splitting would hide the structure;
- mature orchestrators that are actively being reduced into focused modules;
- compatibility surfaces whose churn risk is higher than the size risk.

## Similar Signals

Line count is only one smoke alarm. Use it alongside:

- long functions that mix parsing, IO, policy, and side effects;
- files with too many public entry points;
- modules that change in every feature;
- tests that require unrelated setup to exercise one behavior;
- policy-heavy logic embedded inside runtime loops;
- repeated helper code that points to a missing abstraction.

The goal is not smaller files for their own sake. The goal is code whose agency, authority, and failure modes stay legible as the system becomes more powerful.

## Tooling

Run:

```bash
python3 scripts/architecture_health.py
```

The tool is advisory by default. It reports large source files, long function spans, and public API pressure while excluding generated outputs, workspaces, targets, review bundles, fixtures, and long-form docs. CI or maintainers can opt into stricter behavior with:

```bash
python3 scripts/architecture_health.py --fail-on-critical
```
