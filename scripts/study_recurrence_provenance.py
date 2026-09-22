"""Bounded, read-only provenance for public-study recurrence, not felt truth."""

from collections import Counter, defaultdict
from datetime import datetime, timedelta, timezone
import hashlib
import json
from pathlib import Path
import re

MAX_RECEIPTS = 2048
MAX_RECEIPT_BYTES = 2_097_152


def mirror_source(text):
    header = text.split("\n\n", 1)[0]
    fields = dict(re.findall(r"(?m)^([\w-]+):\s*([^\n]*)$", header))
    if (fields.get("Mode-role") == "mirror_other_expression"
            and fields.get("Authorship", "").endswith("_owned_reflected_without_reauthoring")):
        return fields.get("Source-ID") or "unidentified_mirror"
    return None


def eligible_terms(entry, terms):
    metadata = entry.readback_provenance
    if metadata.get("mirrored_source_id"):
        return []
    provenance = metadata.get("recurrence_v1", {})
    if provenance.get("duplicate_of") or provenance.get("supplied_or_derived_artifact"):
        return []
    excluded = {term.casefold() for term in provenance.get("supplied_terms", [])}
    authored = provenance.get("authored_terms")
    return [term for term in terms if term.casefold() not in excluded
            and (authored is None or term in authored)]


def _receipt_index(entries, workspaces):
    targets = {str(Path(entry.path).resolve()): entry for entry in entries}
    candidates = set()
    for entry in entries:
        root = workspaces.get(entry.being)
        if root is None or entry.readback_provenance.get("mirrored_source_id"):
            continue
        date = datetime.fromtimestamp(entry.mtime_unix_s, timezone.utc)
        for offset in (-1, 0, 1):
            directory = root / "generations" / (date + timedelta(days=offset)).strftime("%Y-%m-%d")
            candidates.update(directory.glob("gen_*.json"))
    index = defaultdict(list)
    examined = 0
    for path in sorted(candidates, reverse=True)[:MAX_RECEIPTS]:
        try:
            if path.is_symlink() or path.stat().st_size > MAX_RECEIPT_BYTES:
                continue
            raw = path.read_bytes()
            data = json.loads(raw)
            examined += 1
            if not isinstance(data, dict) or data.get("status") != "ok":
                continue
            links = data.get("linked_artifacts", [])
            if not isinstance(links, list):
                continue
            for link in links:
                if not isinstance(link, dict) or link.get("kind") != "journal" or link.get("match") != "content":
                    continue
                target = link.get("path")
                if not isinstance(target, str) or not Path(target).is_absolute():
                    continue
                target = str(Path(target).resolve())
                entry = targets.get(target)
                if entry is not None and data.get("being") == entry.being:
                    index[target].append((path, hashlib.sha256(raw).hexdigest(), data))
        except (OSError, ValueError, TypeError):
            continue
    return index, {"receipts_examined": examined, "receipt_limit": MAX_RECEIPTS,
                   "candidate_receipts": len(candidates), "bounded_scan": True,
                   "absence_of_receipt_is_not_absence_of_supplied_input": True}


def annotate(entries, workspaces, terms, text_of, body_of):
    """Retain accounts; exclude known re-supply/copies from recurrence counts.

    Unmatched or hashed-only input remains unknown, not proof of independence.
    No prompt content, response text or private journal is exported here.
    """
    index, scan = _receipt_index(entries, workspaces)
    seen = {}
    counts = defaultdict(Counter)
    records = []
    for entry in sorted(entries, key=lambda item: (item.mtime_unix_s, item.path)):
        text = text_of(entry)
        body = body_of(text)
        body = re.split(r"(?m)^\[(?:Agency-vernacular notice|Pressure-vocabulary cooldown)", body, maxsplit=1)[0]
        found = [term for term in terms if term.casefold() in body.casefold()]
        metadata = {"input_origin": "unknown", "supplied_terms": [],
                    "authored_terms": found, "duplicate_of": None}
        role = entry.readback_provenance.get("role")
        metadata["supplied_or_derived_artifact"] = role in {
            "derived_bridge_summary_for_readback", "steward_or_operator_prompt",
            "received_peer_or_external_artifact", "steward_facing_external_observation",
        }
        mirror = entry.readback_provenance.get("mirrored_source_id")
        matches = index.get(str(Path(entry.path).resolve()), [])
        if len(matches) == 1:
            path, digest, receipt = matches[0]
            metadata.update(receipt_path=str(path), receipt_sha256=digest,
                            generation_id=receipt.get("generation_id"))
            messages = receipt.get("messages")
            response = receipt.get("response_text")
            bound = (isinstance(response, str) and bool(response.strip())
                     and hashlib.sha256(response.encode()).hexdigest() == receipt.get("response_sha256")
                     and response.strip() in text)
            metadata["response_binding"] = "verified_hash_and_journal_content" if bound else "unverified"
            if bound and receipt.get("messages_source") == "adapted" and isinstance(messages, list) and messages:
                inputs = [message for message in messages if isinstance(message, dict)]
                visible = [message["content"] for message in inputs if isinstance(message.get("content"), str)]
                metadata["input_origin"] = ("adapted_complete" if len(visible) == len(messages)
                                            else "adapted_partial")
                metadata["supplied_terms"] = [term for term in found
                    if any(term.casefold() in value.casefold() for value in visible)]
        elif len(matches) > 1:
            metadata["input_origin"] = "ambiguous_receipts"
        key = (entry.being, hashlib.sha256(body.strip().encode()).hexdigest())
        if not mirror:
            metadata["duplicate_of"] = seen.get(key)
            seen.setdefault(key, entry.path)
        entry.readback_provenance["recurrence_v1"] = metadata
        for term in found:
            classification = (
                "mirrored_expression" if mirror else
                "supplied_or_derived_review_text" if metadata["supplied_or_derived_artifact"] else
                "duplicate_authored_text" if metadata["duplicate_of"] else
                "authored_response_to_supplied_term" if term in metadata["supplied_terms"] else
                "authored_return_without_term_in_recorded_input" if metadata["input_origin"] == "adapted_complete" else
                "authored_return_input_unknown" if role in {
                    "being_authored_public_memory", "being_self_interpretation_summary",
                } else "return_authorship_or_input_unknown"
            )
            counts[term][classification] += 1
        records.append({"path": entry.path, "being": entry.being, "terms": found,
                        "mirrored_source_id": mirror, **metadata})
    return {"policy": "study_recurrence_provenance_v1", "authority": "read_only_review",
            "scope": "phenomenology hypotheses/cards and afterimage/absence recurrence",
            "term_counts": {term: dict(count) for term, count in sorted(counts.items())},
            "records": records, "scan": scan,
            "boundary": "Authored interpretations remain evidence. Known supplied terms, exact copies and mirrors do not establish independent recurrence. Unknown inputs remain unknown; absence in one recorded input does not establish novel discovery or causation."}
