"""Historical additive receipts retain exact values and do not gain authority."""

import copy
import unittest

from lived_state_witness.qualitative_texture import (
    EXPECTED, SEMANTIC_EXTENSION, SEMANTIC_EXTENSION_GROUPS,
    SUBJECTIVE_CONTINUITY, LEGACY_SUBJECTIVE_CONTINUITY,
    validate_qualitative_texture_anchor,
)


class HistoricalShapeTests(unittest.TestCase):
    def anchor(self):
        return {**EXPECTED, "canonical_body_sha256": "a" * 64,
                "canonical_body_byte_count": 12}

    def errors(self, value):
        errors = []
        validate_qualitative_texture_anchor(value, errors)
        return errors

    def test_additive_generations_validate_without_rewriting(self):
        value = self.anchor()
        self.assertEqual(self.errors(value), [])
        for group in SEMANTIC_EXTENSION_GROUPS:
            value.update({key: SEMANTIC_EXTENSION[key] for key in group})
            original = copy.deepcopy(value)
            self.assertEqual(self.errors(value), [])
            self.assertEqual(value, original)

    def test_partial_or_changed_declaration_groups_fail(self):
        for group in SEMANTIC_EXTENSION_GROUPS:
            value = self.anchor() | {key: SEMANTIC_EXTENSION[key] for key in group}
            for key in group:
                with self.subTest(key=key):
                    partial = value.copy()
                    del partial[key]
                    self.assertTrue(self.errors(partial))
                    changed = value | {key: "claims_a_measurement"}
                    self.assertTrue(self.errors(changed))

    def test_both_continuity_shapes_are_exact_and_noninferential(self):
        for shape in (LEGACY_SUBJECTIVE_CONTINUITY, SUBJECTIVE_CONTINUITY):
            value = self.anchor() | {"subjective_continuity_v1": shape.copy()}
            self.assertEqual(self.errors(value), [])
            for key, invalid in (("authority_effect", True),
                                 ("subjective_continuity_index", 0.8),
                                 ("automatically_inferred", True)):
                modified = copy.deepcopy(value)
                modified["subjective_continuity_v1"][key] = invalid
                self.assertTrue(self.errors(modified))
        partial = LEGACY_SUBJECTIVE_CONTINUITY | {"texture_fidelity_score": None}
        self.assertTrue(self.errors(self.anchor() | {"subjective_continuity_v1": partial}))

    def test_core_boundaries_and_unknown_fields_still_fail_closed(self):
        for key, invalid in (("direct_causation_claimed", True),
                             ("raw_prose_included", True),
                             ("canonical_body_sha256", "broken"),
                             ("unreviewed_measurement", 0.5)):
            self.assertTrue(self.errors(self.anchor() | {key: invalid}))


if __name__ == "__main__":
    unittest.main()
