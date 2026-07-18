"""Living felt-contract graph core.

The package is dependency-free and domain-neutral. Astrid-specific event
adapters live in :mod:`felt_contracts.sources`.
"""

from .identity import contract_id_for_anchor, edge_id, node_id
from .model import (
    ContractActivityV1,
    ContractContradictionV1,
    ClaimDispositionV1,
    EvidenceSufficiencyV1,
    FeltContractEdgeV1,
    FeltContractNodeV1,
    FeltContractV1,
    FeltReviewOutcomeV1,
    FeltSignalRefV1,
    ImplementationReceiptV1,
    InterventionBoundaryV1,
    TechnicalDispositionV1,
    build_contract,
    build_contradiction,
    build_edge,
    build_implementation_receipt,
    build_intervention_boundary,
    build_node,
    build_signal_ref,
)

__all__ = [
    "ContractActivityV1",
    "ContractContradictionV1",
    "ClaimDispositionV1",
    "EvidenceSufficiencyV1",
    "FeltContractEdgeV1",
    "FeltContractNodeV1",
    "FeltContractV1",
    "FeltReviewOutcomeV1",
    "FeltSignalRefV1",
    "ImplementationReceiptV1",
    "InterventionBoundaryV1",
    "TechnicalDispositionV1",
    "build_contract",
    "build_contradiction",
    "build_edge",
    "build_implementation_receipt",
    "build_intervention_boundary",
    "build_node",
    "build_signal_ref",
    "contract_id_for_anchor",
    "edge_id",
    "node_id",
]
