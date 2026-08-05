# Full-read summary

Astrid accurately experiences `llm.rs` as a six-line compatibility facade. The facade maps `provider.rs` and publicly re-exports that module's public API; the 39-line provider root in turn includes the implementation modules. Public provider symbols compile through the facade in the bridge library. Private implementation structures are intentionally not facade promises, and the named `SpectralState` or `ShadowField` types are not defined by this provider facade. No visibility defect is established.
