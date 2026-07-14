Full read of `introspection_astrid_types_1783898396`.

Astrid identified a real schema loss: `resonance_stability_context_v1` dropped `multi_modal_habitability_score` to `None` whenever any component was missing, causing downstream interpretation to collapse to less specific gate-only context even when partial evidence existed. She also asked for clearer habitability-state arms when partial or conflicting data is present.

Disposition: implemented partial habitability evidence. The context now records whether the score is partial, how many components contributed, which components were missing, and the score basis. Complete weighted scores remain unchanged; non-finite data still stays guarded; partial finite evidence now gets explicit `partial_*_review` states instead of disappearing.
