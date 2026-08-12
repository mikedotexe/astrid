# Full-read summary: introspection_minime_regulator_1785550224

Astrid correctly identifies `regulator/core.rs` as a 24-line include shell and
warns that hollow source presentation is not evidence of missing regulator
behavior. The complete canonical report was read at SHA-256
`e7349c7438eb9539f84091026505231cf4bc785440546454ba11f1877351617e`
(36 lines, 2,591 bytes), with lived-state witness
`lsw_c144bbed21f93b1ff30886e6af6bf577e2f0de93be8eaf54efdd36955d994d1c`.
The complete root remains at report-bound SHA-256
`46828f4c813eb88aae30212793f698285c696c108dd405604ffb6b5129827d97`.

The requested targets were read fully. All 413 lines of `core/pi.rs` at
SHA-256
`72e1bbf47c1ec141ac67b6d55363cf992eae64be684e661c0659b397c65c90b2`
accept fill, `lambda1_rel`, and geometric-radius inputs; calculate separate
errors against configured targets; update conditional, leaky integrators; and
drive bounded gate, filter, and geometric-brake outputs. All 329 lines of
`core/rate_gate.rs` at SHA-256
`2ccc9d72f20c7b173df1e94858de204825db5ac9ffc8939ae2dabc8cde3def4e`
implement bounded per-modality token accumulation and defer when a request
cannot pay its token cost, followed by a separate projection penalty with
admit, attenuate, or defer outcomes.

This completes the report's structural verification. It proves source
mechanics, not which branch was active, whether regulation caused the felt
texture, or whether any output felt suitable. No PI target, gain, integral,
rate, token bucket, admission decision, source, or live process changed.
