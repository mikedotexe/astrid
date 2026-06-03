# Astralis Restored Capsules

These crates are the in-repo Component Model replacements for the default
astralis user-space capsules. Build them with:

```bash
astrid build --type rust-component capsules/astralis/astrid-capsule-agents
```

They use `crates/astrid-guest` directly and export the required WIT stubs.
Only capsules whose manifest explicitly declares daemon/uplink behavior should
run a long-lived `run` loop.
