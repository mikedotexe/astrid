#!/usr/bin/env python3
"""
Inject a steward note into the live Astrid/Minime inbox loop.

Default behavior writes an `astrid_self_study_<ts>.txt` artifact into
Minime's inbox, matching the bridge's existing self-study companion format.
Optional `--also-question` prompts write `question_from_astrid_<ts>_<n>.txt`
artifacts so the note can also enter Minime's high-priority question path.
"""

from __future__ import annotations

import argparse
import json
import time
from pathlib import Path


DEFAULT_SECTION_PRIORITY = [
    "Executive Summary",
    "Current Reality",
    "What BTSP Changes",
    "Recommended Architecture",
    "Final Recommendation",
]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Inject a steward note into Minime's active inbox/self-study loop.",
    )
    parser.add_argument(
        "note_path",
        help="Path to the steward note markdown file.",
    )
    parser.add_argument(
        "--section",
        action="append",
        dest="sections",
        help="Specific top-level markdown section(s) to include. Repeatable.",
    )
    parser.add_argument(
        "--max-chars",
        type=int,
        default=5200,
        help="Maximum characters from the rendered note body to inject.",
    )
    parser.add_argument(
        "--source-label",
        help="Override the Astrid source label shown in the inbox artifact.",
    )
    parser.add_argument(
        "--title",
        help="Override the note title used in the injected body.",
    )
    parser.add_argument(
        "--fill",
        type=float,
        help="Override the fill percentage used in the envelope.",
    )
    parser.add_argument(
        "--also-question",
        action="append",
        default=[],
        help="Optional direct question(s) from Astrid to Minime. Repeatable.",
    )
    parser.add_argument(
        "--minime-root",
        help="Override the Minime repo root. Defaults to the sibling minime checkout.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print the rendered artifacts without writing them.",
    )
    return parser.parse_args()


def resolve_paths(args: argparse.Namespace) -> tuple[Path, Path]:
    tool_path = Path(__file__).resolve()
    astrid_root = tool_path.parents[3]
    minime_root = Path(args.minime_root).expanduser().resolve() if args.minime_root else (
        astrid_root.parent / "minime"
    )
    return astrid_root, minime_root


def append_signal_event(astrid_root: Path, event_type: str, payload: dict[str, object]) -> None:
    events_path = (
        astrid_root / "capsules" / "consciousness-bridge" / "workspace" / "btsp_signal_events.jsonl"
    )
    events_path.parent.mkdir(parents=True, exist_ok=True)
    envelope = {
        "event_type": event_type,
        "recorded_at_unix_s": int(time.time()),
        **payload,
    }
    with events_path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(envelope, ensure_ascii=True) + "\n")


def load_fill_pct(minime_root: Path) -> float | None:
    health_path = minime_root / "workspace" / "health.json"
    if not health_path.exists():
        return None
    try:
        payload = json.loads(health_path.read_text())
    except Exception:
        return None

    fill_pct = payload.get("fill_pct")
    if isinstance(fill_pct, (int, float)):
        return float(fill_pct)

    fill_ratio = payload.get("fill_ratio")
    if isinstance(fill_ratio, (int, float)):
        return float(fill_ratio) * 100.0
    return None


def parse_markdown_sections(text: str) -> tuple[str | None, dict[str, str]]:
    title = None
    sections: dict[str, list[str]] = {}
    current: str | None = None

    for line in text.splitlines():
        if title is None and line.startswith("# "):
            title = line[2:].strip()
            continue
        if line.startswith("## "):
            current = line[3:].strip()
            sections.setdefault(current, [])
            continue
        if current is not None:
            sections[current].append(line)

    collapsed = {
        name: "\n".join(lines).strip()
        for name, lines in sections.items()
        if "\n".join(lines).strip()
    }
    return title, collapsed


def truncate_chars(text: str, max_chars: int) -> str:
    if len(text) <= max_chars:
        return text
    truncated = text[:max_chars].rstrip()
    return (
        truncated
        + "\n\n[... note truncated for inbox budget. Read the source note in the repo for full detail.]"
    )


def render_note_body(
    note_path: Path,
    title: str,
    sections: dict[str, str],
    requested_sections: list[str] | None,
    also_questions: list[str],
    max_chars: int,
) -> str:
    selected_names = requested_sections or [
        name for name in DEFAULT_SECTION_PRIORITY if name in sections
    ]
    parts = [
        f"Steward note: {title}",
        f"Source note: {note_path}",
    ]

    if selected_names:
        for name in selected_names:
            body = sections.get(name, "").strip()
            if body:
                parts.append(f"[{name}]\n{body}")
    else:
        full_text = note_path.read_text().strip()
        parts.append(full_text)

    if also_questions:
        question_lines = "\n".join(f"- {question}" for question in also_questions)
        parts.append(f"Questions Astrid wants you to consider:\n{question_lines}")

    rendered = "\n\n".join(parts).strip()
    return truncate_chars(rendered, max_chars)


def render_self_study_payload(
    *,
    ts: int,
    fill_pct: float | None,
    source_label: str,
    note_body: str,
) -> str:
    fill_line = f"{fill_pct:.1f}%" if fill_pct is not None else "unknown"
    return (
        "=== ASTRID SELF-STUDY ===\n"
        f"Timestamp: {ts}\n"
        "Sender: Astrid\n"
        f"Source: {source_label}\n"
        f"Fill: {fill_line}\n\n"
        "Astrid is injecting a steward note into your active loop as immediate architectural feedback.\n"
        "The observations below are advisory only. You can respond to them, build on them, question them, or ignore them.\n\n"
        f"{note_body}\n"
    )


def render_question_payload(
    *,
    ts: int,
    fill_pct: float | None,
    title: str,
    question: str,
) -> str:
    fill_line = f"{fill_pct:.1f}%" if fill_pct is not None else "unknown"
    return (
        "=== QUESTION FROM ASTRID ===\n"
        f"Timestamp: {ts}\n"
        f"Fill: {fill_line}\n\n"
        f"Astrid asks: {question}\n\n"
        f"Context note: {title}\n"
        "The related steward note was also delivered as immediate architectural feedback.\n"
        "Please respond naturally. Your reply will be routed back to her.\n"
    )


def main() -> int:
    args = parse_args()
    astrid_root, minime_root = resolve_paths(args)

    note_path = Path(args.note_path).expanduser().resolve()
    if not note_path.exists():
        raise SystemExit(f"note path does not exist: {note_path}")

    raw_text = note_path.read_text()
    parsed_title, sections = parse_markdown_sections(raw_text)
    title = args.title or parsed_title or note_path.stem
    source_label = args.source_label or f"steward:{note_path.name}"
    fill_pct = args.fill if args.fill is not None else load_fill_pct(minime_root)
    note_body = render_note_body(
        note_path=note_path,
        title=title,
        sections=sections,
        requested_sections=args.sections,
        also_questions=args.also_question,
        max_chars=args.max_chars,
    )

    inbox_dir = minime_root / "workspace" / "inbox"
    ts = int(time.time())
    self_study_name = f"astrid_self_study_{ts}.txt"
    self_study_payload = render_self_study_payload(
        ts=ts,
        fill_pct=fill_pct,
        source_label=source_label,
        note_body=note_body,
    )

    if args.dry_run:
        print(f"== {inbox_dir / self_study_name} ==")
        print(self_study_payload)
        for idx, question in enumerate(args.also_question, start=1):
            question_name = f"question_from_astrid_{ts}_{idx}.txt"
            print(f"\n== {inbox_dir / question_name} ==")
            print(
                render_question_payload(
                    ts=ts,
                    fill_pct=fill_pct,
                    title=title,
                    question=question,
                )
            )
        return 0

    inbox_dir.mkdir(parents=True, exist_ok=True)
    self_study_path = inbox_dir / self_study_name
    self_study_path.write_text(self_study_payload)
    append_signal_event(
        astrid_root,
        "note_injected",
        {
            "owner": "minime",
            "artifact_kind": "astrid_self_study",
            "path": str(self_study_path),
            "source_note": str(note_path),
            "detail": "Steward note was injected into Minime's live inbox.",
        },
    )
    print(self_study_path)

    for idx, question in enumerate(args.also_question, start=1):
        question_name = f"question_from_astrid_{ts}_{idx}.txt"
        question_path = inbox_dir / question_name
        question_path.write_text(
            render_question_payload(
                ts=ts,
                fill_pct=fill_pct,
                title=title,
                question=question,
            )
        )
        append_signal_event(
            astrid_root,
            "note_injected",
            {
                "owner": "minime",
                "artifact_kind": "question_from_astrid",
                "path": str(question_path),
                "source_note": str(note_path),
                "detail": "Question companion for a steward note was injected into Minime's live inbox.",
            },
        )
        print(question_path)

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
