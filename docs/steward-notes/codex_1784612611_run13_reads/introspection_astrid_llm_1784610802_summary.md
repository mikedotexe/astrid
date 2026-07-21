# Full read: introspection_astrid_llm_1784610802

Astrid identifies two exact quality-gate edges: seven consecutive symbols should remain valid while eight should reject, and readable technical expressions should not be mistaken for malformed output. Both are concrete regressions worth pinning without retuning the gate.

Run 13 added and passed tests for the exact 7/8 boundary and for a technical sentence containing comparison and conjunction operators. Current artifact cleanup remains a single-pass longest-marker scanner with exact accounting and graded contextual diagnostics.

No prompt, model, threshold, fallback, or live provider behavior changed, so this implementation requires no restart.
