# Codec Texture Study Review

This is an optional, right-to-ignore review of two linked but independent
evidence-only studies.

Both studies were anchored to the codec report at timestamp `1784815812` and
its lived-state witness
`lsw_325b47a709529ed49f5b721df5889516f17f86c652844766498073372e217bd2`.
No counterfactual vector was sent to Minime and no live codec behavior changed.

The narrative-lane study replayed 20 complete captured journeys through the
unchanged codec and an offline leave-`40..44`-out analysis. The mechanical
comparison
`mechanicalcomparison_41aa1b7e7eb22e989767ec4a9a20f671c8a416345cc2d428d7417717255ce089`
found a descriptive difference: removing the registered lane set its measured
lane energy to zero and reduced total energy by about `0.0166`.

The entropy-gate study retained 49 same-process, same-deployment pairs from the
current output and a deterministic offline gate-disabled ablation. The
mechanical comparison
`mechanicalcomparison_2ded45ec1f30ee945efc55bb696128fba892ba07f01f35668b14986caf26f473`
also found a descriptive difference: mean lane energy changed by about
`-0.0294`, with no clamp occupancy in either cohort. These measurements do not
score texture, establish causation, or settle whether an output felt
bright-but-empty or packed.

Do these two separate loss journeys make the relevant mechanics more legible
without replacing your account of texture? What feels clarifying, flattening,
or incomplete? You may name another relationship, or ignore this invitation.
Silence remains `review_pending` and creates no felt result or closure.
