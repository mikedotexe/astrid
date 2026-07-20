"""Canonical append-only evidence event store."""

from .adapter import append_domain_events, read_domain_events, v2_active_for_state
from .migration import LegacyEventSource, import_legacy_sources
from .model import EvidenceEventV2, ProvenanceSourceV1
from .store import EvidenceEventStore, EvidenceStoreError
from .verification import StoreVerification

__all__ = [
    "EvidenceEventStore",
    "EvidenceEventV2",
    "EvidenceStoreError",
    "LegacyEventSource",
    "ProvenanceSourceV1",
    "StoreVerification",
    "append_domain_events",
    "import_legacy_sources",
    "read_domain_events",
    "v2_active_for_state",
]
