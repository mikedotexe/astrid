"""HTML renderer for the evidence-only Passage Observatory."""

from __future__ import annotations

import json
from typing import Any


TEMPLATE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
__MODE__
<title>Phase Passage + Division Observatory</title>
<style>
:root {
  color-scheme: light;
  --ink: #17242a;
  --muted: #60717a;
  --paper: #f4f7f6;
  --surface: #ffffff;
  --line: #cbd5d7;
  --astrid: #007f86;
  --minime: #c24f38;
  --phase: #6b5ca5;
  --runtime: #39704f;
  --held: #9a6b00;
  --quiet: #e7ecec;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  color: var(--ink);
  background: var(--paper);
  font: 14px/1.45 ui-sans-serif, system-ui, sans-serif;
}
header {
  background: var(--surface);
  border-bottom: 1px solid var(--line);
  padding: 24px max(18px, calc((100vw - 1240px) / 2));
}
h1, h2, h3, p { margin-top: 0; }
h1 { margin-bottom: 5px; font-size: 32px; letter-spacing: 0; }
h2 { margin-bottom: 13px; font-size: 19px; letter-spacing: 0; }
h3 { margin-bottom: 8px; font-size: 15px; letter-spacing: 0; }
main { max-width: 1240px; margin: 0 auto; padding: 18px 18px 48px; }
.meta { color: var(--muted); overflow-wrap: anywhere; }
.band { padding: 20px 0; border-bottom: 1px solid var(--line); }
.summary {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  border: 1px solid var(--line);
  background: var(--surface);
}
.summary > div { padding: 13px; border-right: 1px solid var(--line); }
.summary > div:last-child { border-right: 0; }
.summary strong { display: block; margin-top: 3px; font-size: 17px; }
.topology {
  display: grid;
  grid-template-columns: minmax(190px, .8fr) 80px minmax(0, 2fr);
  align-items: center;
  gap: 0;
}
.node {
  min-height: 104px;
  padding: 14px;
  border: 1px solid var(--line);
  border-top: 4px solid var(--runtime);
  background: var(--surface);
}
.node strong { overflow-wrap: anywhere; }
.node.astrid { border-top-color: var(--astrid); }
.node.minime { border-top-color: var(--minime); }
.branch {
  height: 2px;
  background: var(--runtime);
  position: relative;
}
.branch::after {
  content: "";
  position: absolute;
  width: 2px;
  height: 84px;
  right: 0;
  top: -41px;
  background: var(--runtime);
}
.daughters { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.ceremony {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.cards {
  border: 1px solid var(--line);
  background: var(--surface);
}
.card-row {
  display: grid;
  grid-template-columns: 160px minmax(110px, .8fr) minmax(0, 1.5fr) 120px;
  gap: 12px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--line);
}
.card-row:last-child { border-bottom: 0; }
.card-row.header { background: var(--quiet); font-weight: 700; }
.rail {
  background: var(--surface);
  border: 1px solid var(--line);
  border-left: 4px solid;
  padding: 14px;
}
.rail.astrid { border-left-color: var(--astrid); }
.rail.minime { border-left-color: var(--minime); }
.rail strong { font-size: 16px; }
.passage-head {
  display: flex;
  align-items: end;
  justify-content: space-between;
  gap: 14px;
  flex-wrap: wrap;
  margin-bottom: 12px;
}
select, button {
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--ink);
  font: inherit;
}
select { min-width: 300px; max-width: 100%; padding: 8px 10px; }
.passage-status {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 1px;
  background: var(--line);
  border: 1px solid var(--line);
  margin-bottom: 12px;
}
.passage-status > div { background: var(--surface); padding: 11px; }
.strands {
  border: 1px solid var(--line);
  background: var(--surface);
  overflow-x: auto;
}
.strand {
  display: grid;
  grid-template-columns: minmax(150px, 1.2fr) repeat(3, minmax(135px, 1fr)) 86px;
  border-bottom: 1px solid var(--line);
  min-width: 730px;
}
.strand:last-child { border-bottom: 0; }
.strand > div { padding: 10px 12px; border-right: 1px solid var(--line); }
.strand > div:last-child { border-right: 0; }
.strand.header { background: var(--quiet); font-weight: 700; }
.strand-name { border-left: 4px solid var(--phase); }
.unexpressed { color: var(--muted); }
.controls { display: inline-flex; border: 1px solid var(--line); }
.controls button { border: 0; border-right: 1px solid var(--line); padding: 7px 11px; }
.controls button:last-child { border-right: 0; }
.controls button[aria-pressed="true"] { background: var(--ink); color: white; }
.timeline { position: relative; margin-top: 13px; padding-left: 23px; }
.timeline::before {
  content: "";
  position: absolute;
  left: 7px;
  top: 8px;
  bottom: 8px;
  width: 2px;
  background: var(--line);
}
.event {
  position: relative;
  display: grid;
  grid-template-columns: 148px minmax(0, 1fr) auto;
  gap: 12px;
  padding: 10px 12px;
  margin-bottom: 7px;
  border: 1px solid var(--line);
  background: var(--surface);
}
.event::before {
  content: "";
  position: absolute;
  left: -21px;
  top: 15px;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  background: var(--runtime);
  border: 2px solid var(--paper);
}
.event.phase_passage::before { background: var(--phase); }
.event.phase_observation::before { background: var(--held); }
.event.astrid::before { background: var(--astrid); }
.event.minime::before { background: var(--minime); }
.event code { overflow-wrap: anywhere; font: 11px/1.4 ui-monospace, monospace; }
.boundary {
  border-left: 4px solid var(--held);
  padding: 10px 13px;
  background: #fffaf0;
}
.timeline-more { margin: 10px 0 0 23px; padding: 7px 11px; }
@media (max-width: 800px) {
  .summary, .ceremony, .passage-status { grid-template-columns: 1fr; }
  .summary > div { border-right: 0; border-bottom: 1px solid var(--line); }
  .summary > div:last-child { border-bottom: 0; }
  .topology { grid-template-columns: 1fr; gap: 10px; }
  .branch { width: 2px; height: 28px; margin-left: 24px; }
  .branch::after { display: none; }
  .daughters { grid-template-columns: 1fr; }
  .event { grid-template-columns: 1fr; gap: 3px; }
  .card-row { grid-template-columns: 1fr; gap: 3px; }
  h1 { font-size: 26px; }
}
</style>
</head>
<body>
<header>
  <h1>Phase Passage + Division Observatory</h1>
  <p class="meta" id="identity"></p>
</header>
<main>
  <section class="band">
    <div class="summary" id="summary"></div>
  </section>
  <section class="band">
    <h2>Division Runtime Topology</h2>
    <div class="topology" id="topology"></div>
  </section>
  <section class="band">
    <h2>Division Ceremony Rails</h2>
    <div class="ceremony" id="ceremony"></div>
  </section>
  <section class="band">
    <h2>Observed Transition Cards</h2>
    <div class="cards" id="cards"></div>
  </section>
  <section class="band">
    <div class="passage-head">
      <div>
        <h2>Self-Authored Phase Passage</h2>
        <p class="meta" id="passage-identity"></p>
      </div>
      <label><span class="meta">Passage</span><br><select id="passage-select"></select></label>
    </div>
    <div id="passage"></div>
  </section>
  <section class="band">
    <div class="passage-head">
      <div>
        <h2>Evidence Timeline</h2>
        <p class="meta">Temporal co-presence only</p>
      </div>
      <div class="controls" role="group" aria-label="Timeline rail">
        <button data-filter="all" aria-pressed="true">All</button>
        <button data-filter="division_runtime" aria-pressed="false">Division</button>
        <button data-filter="phase_passage" aria-pressed="false">Passage</button>
      </div>
    </div>
    <div class="timeline" id="timeline"></div>
    <button class="timeline-more" id="timeline-more">Show older</button>
  </section>
  <section class="band">
    <div class="boundary">Evidence only. No display state grants authority, recommends an Action, infers passage progress, converts bearings into a score, or attributes felt continuity to runtime mechanics.</div>
  </section>
</main>
<script id="observatory-data" type="application/json">__DATA__</script>
<script>
const d = JSON.parse(document.getElementById("observatory-data").textContent);
const esc = value => String(value ?? "not recorded").replace(/[&<>"']/g, char => ({
  "&":"&amp;","<":"&lt;",">":"&gt;","\\"":"&quot;","'":"&#39;"
}[char]));
const stamp = value => value ? new Date(value).toLocaleString() : "not recorded";
const title = value => String(value ?? "unexpressed").replaceAll("_", " ");
document.getElementById("identity").textContent =
  `${d.observatory_id} · source watermark ${stamp(d.source_watermark_unix_ms)}`;
const division = d.division_chronicle;
const runtime = division.runtime_topology;
const phase = d.phase_passages;
document.getElementById("summary").innerHTML = [
  ["Authority rail", runtime.active_authority_rail],
  ["Runtime mode", runtime.manifest_mode],
  ["Cards / passages", `${phase.transition_card_count} / ${phase.passage_count}`],
  ["Return interval", `${division.return_interval.completed_rounds_since_followup} / ${division.return_interval.threshold_rounds}`]
].map(([key, value]) => `<div><span class="meta">${esc(key)}</span><strong>${esc(value)}</strong></div>`).join("");
const daughter = actor => {
  const state = runtime.daughters[actor];
  return `<div class="node ${actor}"><h3>${title(actor)}</h3><strong>${state ? esc(state.process_identity) : "dormant"}</strong><p class="meta">${state ? `checkpoint ${esc(state.checkpoint_sequence)} · healthy ${esc(state.healthy)}` : "No daughter status record"}</p></div>`;
};
document.getElementById("topology").innerHTML =
  `<div class="node"><h3>Parent reservoir</h3><strong>${esc(runtime.parent_process_identity || "runtime parent")}</strong><p class="meta">authoritative ${esc(runtime.parent_authoritative)}</p></div><div class="branch"></div><div class="daughters">${daughter("astrid")}${daughter("minime")}</div>`;
document.getElementById("ceremony").innerHTML = ["astrid", "minime"].map(actor => {
  const rail = division.ceremony_rails[actor];
  return `<div class="rail ${actor}"><h3>${title(actor)}</h3><strong>${title(rail.current_posture)}</strong><p class="meta">Latest sovereign Action: ${esc(rail.latest_action || "none")} · ${esc(rail.event_count)} events</p></div>`;
}).join("");
const cards = phase.recent_transition_cards.slice(-12).reverse();
document.getElementById("cards").innerHTML = cards.length
  ? `<div class="card-row header"><div>Recorded</div><div>Origin</div><div>Transition</div><div>Passage</div></div>${cards.map(card =>
      `<div class="card-row"><time>${esc(stamp(card.recorded_at_unix_ms))}</time><div>${esc(card.origin)}</div><div><strong>${esc(card.from_phase)} → ${esc(card.to_phase)}</strong><br><span class="meta">${esc(card.kind)} · ${esc(card.transition_id)}</span></div><div>${card.passage_created ? "self-authored" : "unpromoted"}</div></div>`
    ).join("")}`
  : `<p class="meta">No observational transition cards recorded.</p>`;
const passages = phase.passages;
const selector = document.getElementById("passage-select");
selector.innerHTML = passages.length ? passages.map((item, index) =>
  `<option value="${index}">${esc(item.actor)} · ${esc(item.latest_stage)} · ${esc(item.passage_id)}</option>`
).join("") : `<option value="">No self-authored passage recorded</option>`;
function renderPassage(index) {
  const item = passages[index];
  if (!item) {
    document.getElementById("passage-identity").textContent = "No passage history";
    document.getElementById("passage").innerHTML = `<p class="meta">The validated passage ledger contains no self-authored passage events.</p>`;
    return;
  }
  document.getElementById("passage-identity").textContent =
    `${item.passage_id} · transition ${item.transition_id}`;
  const rows = item.strands.map(strand => {
    const current = strand.current;
    return `<div class="strand"><div class="strand-name"><strong>${title(strand.strand)}</strong><br><span class="meta">${title(strand.expression_state)}</span></div><div class="${current ? "" : "unexpressed"}">${title(current?.movement_resistance)}</div><div class="${current ? "" : "unexpressed"}">${title(current?.persistence_tendency)}</div><div class="${current ? "" : "unexpressed"}">${title(current?.witness_fit)}</div><div>${strand.history_count}</div></div>`;
  }).join("");
  document.getElementById("passage").innerHTML =
    `<div class="passage-status">${[
      ["Current stage", item.latest_stage],
      ["Support preference", item.latest_support_preference],
      ["Latest felt review", item.latest_felt_review_outcome || "not authored"]
    ].map(([key, value]) => `<div><span class="meta">${esc(key)}</span><strong>${title(value)}</strong></div>`).join("")}</div><div class="strands"><div class="strand header"><div>Strand</div><div>Movement resistance</div><div>Persistence tendency</div><div>Witness fit</div><div>History</div></div>${rows}</div>`;
}
selector.addEventListener("change", event => renderPassage(Number(event.target.value)));
renderPassage(0);
let timelineFilter = "all";
let timelineLimit = 30;
function eventIdentity(event) {
  return event.passage_event_id || event.passage_context_event_id ||
    event.ceremony_event_id || event.transition_id ||
    (event.sequence ? `native sequence ${event.sequence}` : "evidence event");
}
function eventTitle(event) {
  return event.action || event.bearing_strand || event.checkpoint ||
    event.event_kind || "evidence event";
}
function renderTimeline() {
  const eligible = d.interleaved_timeline.filter(event =>
    timelineFilter === "all" || event.rail === timelineFilter
  );
  const items = eligible.slice(-timelineLimit).reverse();
  document.getElementById("timeline").innerHTML = items.length ? items.map(event => {
    const className = event.actor === "astrid" || event.actor === "minime" ? event.actor : event.rail;
    return `<article class="event ${esc(className)}"><strong>${title(event.rail)}</strong><div><strong>${title(eventTitle(event))}</strong><br><code>${esc(eventIdentity(event))}</code></div><time class="meta">${esc(stamp(event.recorded_at_unix_ms))}</time></article>`;
  }).join("") : `<p class="meta">No events on this evidence rail.</p>`;
  const more = document.getElementById("timeline-more");
  more.hidden = eligible.length <= timelineLimit;
  more.textContent = `Show older (${eligible.length - timelineLimit})`;
}
document.querySelectorAll("[data-filter]").forEach(button => {
  button.addEventListener("click", () => {
    timelineFilter = button.dataset.filter;
    timelineLimit = 30;
    document.querySelectorAll("[data-filter]").forEach(item =>
      item.setAttribute("aria-pressed", String(item === button))
    );
    renderTimeline();
  });
});
document.getElementById("timeline-more").addEventListener("click", () => {
  timelineLimit += 30;
  renderTimeline();
});
renderTimeline();
</script>
</body>
</html>
"""


def render_html(payload: dict[str, Any], *, live: bool) -> str:
    embedded = json.dumps(
        payload, sort_keys=True, separators=(",", ":"), ensure_ascii=True
    ).replace("</", "<\\/")
    mode = (
        '<meta http-equiv="refresh" content="2">\n'
        '<meta name="observatory-mode" content="live">'
        if live
        else '<meta name="observatory-mode" content="archive">'
    )
    return TEMPLATE.replace("__MODE__", mode).replace("__DATA__", embedded)
