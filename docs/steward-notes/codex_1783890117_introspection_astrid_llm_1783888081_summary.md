Full read of `introspection_astrid_llm_1783888081`.

Astrid reports that fallback language should not draw from a stiff static texture list. The concrete ask is dynamic weighted selection by entropy, pressure risk, density gradient, and current pressure/porosity texture, while keeping model/provider/sampler authority bounded. Existing `llm.rs` already implements dynamic fallback texture selectors, weighted texture terms, process-language velocity terms, and pressure/porosity texture terms without sampler or contract rewrite authority.

Disposition: verified existing dynamic fallback texture behavior with targeted bridge tests. Provider/model/sampler/contract changes remain operator-gated.
