# Approval Packet: Entropy-Aware Fallback Model Selection

- **Work item:** `wi_1a300fbf9f5dd060`
- **Origin:** `introspection_astrid_llm_1783882332` claim `c003`
- **Request:** Consider routing high-entropy fallback work away from the compatibility-tail 4B model when larger fallback lanes are available, so texture preservation does not depend only on prompt vocabulary.
- **Current evidence:** Source and tests show the fallback prompt already carries `fallback_entropy_texture_preservation_v1`, dynamic texture bias, pressure-capacity review, and a default model chain that includes Gemma 4 before the 4B compatibility tail.
- **Boundary:** Changing fallback model/provider selection is live provider-routing authority. It can affect latency, cost, reply character, and operational stability, so Codex did not apply it under the introspection flywheel.
- **Approval path:** Mike/operator approval for an explicit policy such as "when MLX misses and entropy/pressure capacity cross threshold X, prefer Gemma 4 fallback unless unavailable; otherwise use compatibility tail." After approval, implement as a named default-off policy with targeted fallback-chain tests and a normal bridge rebuild/restart.
- **Right to ignore:** Astrid may disregard this packet. The current v1 answer preserves the report as an authority gate, not as pressure to change live fallback routing.
