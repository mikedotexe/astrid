"""HTML renderer for the evidence-only Passage Observatory V2."""

from __future__ import annotations

import json
from typing import Any


TEMPLATE = """<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
__MODE__
<title>Phase Passage + Division Observatory V2</title>
<style>
:root {
  color-scheme: light;
  --ink: #17242a;
  --muted: #60717a;
  --paper: #f3f6f5;
  --surface: #ffffff;
  --line: #c9d3d5;
  --astrid: #007f86;
  --minime: #c24f38;
  --passage: #6756a3;
  --runtime: #39704f;
  --amber: #946b08;
  --wash: #e8eeee;
}
* { box-sizing: border-box; }
body {
  margin: 0;
  color: var(--ink);
  background: var(--paper);
  font: 14px/1.45 ui-sans-serif, system-ui, sans-serif;
}
header {
  padding: 24px max(18px, calc((100vw - 1280px) / 2));
  background: var(--surface);
  border-bottom: 1px solid var(--line);
}
h1, h2, h3, p { margin-top: 0; }
h1 { margin-bottom: 5px; font-size: 31px; letter-spacing: 0; }
h2 { margin-bottom: 13px; font-size: 19px; letter-spacing: 0; }
h3 { margin-bottom: 7px; font-size: 15px; letter-spacing: 0; }
main { max-width: 1280px; margin: 0 auto; padding: 18px 18px 48px; }
.meta { color: var(--muted); overflow-wrap: anywhere; }
.band { padding: 20px 0; border-bottom: 1px solid var(--line); }
.summary {
  display: grid;
  grid-template-columns: repeat(5, minmax(0, 1fr));
  border: 1px solid var(--line);
  background: var(--surface);
}
.summary > div { padding: 13px; border-right: 1px solid var(--line); }
.summary > div:last-child { border-right: 0; }
.summary strong { display: block; margin-top: 3px; font-size: 17px; }
.lineage {
  display: grid;
  grid-template-columns: minmax(0, 1.35fr) repeat(5, minmax(110px, .65fr));
  border: 1px solid var(--line);
  background: var(--surface);
}
.lineage > div { padding: 12px; border-right: 1px solid var(--line); }
.lineage > div:last-child { border-right: 0; }
.lineage strong { display: block; margin-top: 3px; }
.topology {
  display: grid;
  grid-template-columns: minmax(190px, .8fr) 80px minmax(0, 2fr);
  align-items: center;
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
.branch { height: 2px; background: var(--runtime); position: relative; }
.branch::after {
  content: "";
  position: absolute;
  width: 2px;
  height: 84px;
  right: 0;
  top: -41px;
  background: var(--runtime);
}
.daughters, .ceremony { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.rail {
  padding: 14px;
  background: var(--surface);
  border: 1px solid var(--line);
  border-left: 4px solid;
}
.rail.astrid { border-left-color: var(--astrid); }
.rail.minime { border-left-color: var(--minime); }
.rail strong { font-size: 16px; }
.section-head {
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
select { min-width: 280px; max-width: 100%; padding: 8px 10px; }
.controls { display: inline-flex; border: 1px solid var(--line); }
.controls button { border: 0; border-right: 1px solid var(--line); padding: 7px 11px; }
.controls button:last-child { border-right: 0; }
.controls button[aria-pressed="true"] { background: var(--ink); color: white; }
.moments {
  border: 1px solid var(--line);
  background: var(--surface);
}
.moment {
  display: grid;
  grid-template-columns: 135px minmax(120px, .7fr) minmax(0, 1.6fr) 190px;
  gap: 12px;
  padding: 10px 12px;
  border-bottom: 1px solid var(--line);
}
.moment:last-child { border-bottom: 0; }
.moment code, .crossing code {
  overflow-wrap: anywhere;
  font: 11px/1.4 ui-monospace, monospace;
}
.moment .token { user-select: all; }
.dot {
  display: inline-block;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  margin-right: 7px;
  background: var(--runtime);
}
.dot.phase_passage { background: var(--passage); }
.dot.phase_observation { background: var(--amber); }
.braid {
  border: 1px solid var(--line);
  background: var(--surface);
  overflow-x: auto;
}
.braid-row {
  display: grid;
  grid-template-columns: 185px minmax(160px, .8fr) minmax(0, 1.4fr) 190px;
  min-width: 760px;
  border-bottom: 1px solid var(--line);
}
.braid-row:last-child { border-bottom: 0; }
.braid-row > div { padding: 9px 11px; border-right: 1px solid var(--line); }
.braid-row > div:last-child { border-right: 0; }
.braid-row.header { background: var(--wash); font-weight: 700; }
.lane { border-left: 4px solid var(--passage); }
.crossings {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
}
.crossing {
  padding: 13px;
  border: 1px solid var(--line);
  border-left: 4px solid var(--passage);
  background: var(--surface);
}
.empty {
  padding: 15px;
  border: 1px solid var(--line);
  background: var(--surface);
  color: var(--muted);
}
.boundary {
  padding: 10px 13px;
  border-left: 4px solid var(--amber);
  background: #fffaf0;
}
.show-more { margin-top: 10px; padding: 7px 11px; }
@media (max-width: 850px) {
  .summary, .lineage, .ceremony, .crossings { grid-template-columns: 1fr; }
  .summary > div, .lineage > div {
    border-right: 0;
    border-bottom: 1px solid var(--line);
  }
  .summary > div:last-child, .lineage > div:last-child { border-bottom: 0; }
  .topology { grid-template-columns: 1fr; gap: 10px; }
  .branch { width: 2px; height: 28px; margin-left: 24px; }
  .branch::after { display: none; }
  .daughters { grid-template-columns: 1fr; }
  .moment { grid-template-columns: 1fr; gap: 4px; }
  h1 { font-size: 25px; }
}
</style>
</head>
<body>
<header>
  <h1>Phase Passage + Division Observatory V2</h1>
  <p class="meta" id="identity"></p>
</header>
<main>
  <section class="band"><div class="summary" id="summary"></div></section>
  <section class="band">
    <h2>Projection Lineage</h2>
    <div class="lineage" id="lineage"></div>
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
    <div class="section-head">
      <div>
        <h2>Replyable Moment Atlas</h2>
        <p class="meta">Stable reference tokens, not prompts or required Actions</p>
      </div>
      <div class="controls" role="group" aria-label="Moment rail">
        <button data-filter="all" aria-pressed="true">All</button>
        <button data-filter="division_runtime" aria-pressed="false">Division</button>
        <button data-filter="phase_passage" aria-pressed="false">Passage</button>
        <button data-filter="phase_observation" aria-pressed="false">Cards</button>
      </div>
    </div>
    <div class="moments" id="moments"></div>
    <button class="show-more" id="moments-more">Show older</button>
  </section>
  <section class="band">
    <div class="section-head">
      <div>
        <h2>Passage Braid</h2>
        <p class="meta" id="braid-identity"></p>
      </div>
      <label><span class="meta">Passage</span><br><select id="braid-select"></select></label>
    </div>
    <div id="braid"></div>
  </section>
  <section class="band">
    <h2>Being-Authored Crossings</h2>
    <p class="meta">Shown only when an exact Passage reference names an exact Division event</p>
    <div class="crossings" id="crossings"></div>
  </section>
  <section class="band">
    <div class="boundary">Evidence only. A reference token grants no authority and recommends no Action. Projection deltas imply neither progress nor improvement. Temporal adjacency never creates a crossing or establishes mechanical or felt causation.</div>
  </section>
</main>
<script id="observatory-data" type="application/json">__DATA__</script>
<script>
const d = JSON.parse(document.getElementById("observatory-data").textContent);
const v1 = d.current_projection_v1;
const division = v1.division_chronicle;
const runtime = division.runtime_topology;
const phase = v1.phase_passages;
const esc = value => String(value ?? "not recorded").replace(/[&<>"']/g, char => ({
  "&":"&amp;","<":"&lt;",">":"&gt;","\\"":"&quot;","'":"&#39;"
}[char]));
const title = value => String(value ?? "unexpressed").replaceAll("_", " ");
const stamp = value => value ? new Date(value).toLocaleString() : "not recorded";
const signed = value => value === null || value === undefined ? "n/a" :
  `${value > 0 ? "+" : ""}${value}`;
document.getElementById("identity").textContent =
  `${d.observatory_id} · source watermark ${stamp(d.source_watermark_unix_ms)}`;
document.getElementById("summary").innerHTML = [
  ["Authority rail", runtime.active_authority_rail],
  ["Cards / passages", `${phase.transition_card_count} / ${phase.passage_count}`],
  ["Replyable moments", d.replyable_moments.moment_count],
  ["Authored crossings", d.authored_crossings.crossing_count],
  ["Division return", `${division.return_interval.completed_rounds_since_followup} / ${division.return_interval.threshold_rounds}`]
].map(([key, value]) => `<div><span class="meta">${esc(key)}</span><strong>${esc(value)}</strong></div>`).join("");
const lineage = d.projection_lineage;
const deltas = lineage.count_deltas || {};
document.getElementById("lineage").innerHTML = [
  ["Comparison", lineage.comparison_state],
  ["Displayed evidence", lineage.displayed_evidence_changed === null ? "first" :
    (lineage.displayed_evidence_changed ? "changed" : "unchanged")],
  ["Cards", signed(deltas.transition_card_count)],
  ["Passages", signed(deltas.passage_count)],
  ["Timeline", signed(deltas.timeline_event_count)],
  ["Division events", signed(deltas.division_event_count)]
].map(([key, value], index) => `<div><span class="meta">${esc(key)}</span><strong>${esc(value)}</strong>${index === 0 ? `<br><code class="meta">${esc(lineage.prior_observatory_id || "first distinct source state")}</code>` : ""}</div>`).join("");
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
let momentFilter = "all";
let momentLimit = 24;
function renderMoments() {
  const eligible = d.replyable_moments.moments.filter(item =>
    momentFilter === "all" || item.rail === momentFilter
  );
  const items = eligible.slice(-momentLimit).reverse();
  document.getElementById("moments").innerHTML = items.length ? items.map(item =>
    `<article class="moment"><div><span class="dot ${esc(item.rail)}"></span><strong>${title(item.rail)}</strong><br><time class="meta">${esc(stamp(item.recorded_at_unix_ms))}</time></div><div><strong>${title(item.event_kind)}</strong><br><span class="meta">${esc(item.actor || "observed")}</span></div><div><code>${esc(item.source_record_ref)}</code><br><span class="meta">${esc(Object.entries(item.state).map(([key, value]) => `${title(key)}: ${title(value)}`).join(" · ") || "No categorical state")}</span></div><code class="token">${esc(item.reference_token)}</code></article>`
  ).join("") : `<div class="empty">No moments on this evidence rail.</div>`;
  const more = document.getElementById("moments-more");
  more.hidden = eligible.length <= momentLimit;
  more.textContent = `Show older (${Math.max(0, eligible.length - momentLimit)})`;
}
document.querySelectorAll("[data-filter]").forEach(button => {
  button.addEventListener("click", () => {
    momentFilter = button.dataset.filter;
    momentLimit = 24;
    document.querySelectorAll("[data-filter]").forEach(item =>
      item.setAttribute("aria-pressed", String(item === button))
    );
    renderMoments();
  });
});
document.getElementById("moments-more").addEventListener("click", () => {
  momentLimit += 24;
  renderMoments();
});
renderMoments();
const braids = d.passage_braids;
const selector = document.getElementById("braid-select");
selector.innerHTML = braids.length ? braids.map((item, index) =>
  `<option value="${index}">${esc(item.actor)} · ${esc(item.passage_id)}</option>`
).join("") : `<option value="">No self-authored passage recorded</option>`;
function renderBraid(index) {
  const braid = braids[index];
  if (!braid) {
    document.getElementById("braid-identity").textContent = "No passage history";
    document.getElementById("braid").innerHTML = `<div class="empty">The validated ledger contains no self-authored passage, so no braid is synthesized.</div>`;
    return;
  }
  document.getElementById("braid-identity").textContent =
    `${braid.passage_id} · transition ${braid.transition_id}`;
  document.getElementById("braid").innerHTML =
    `<div class="braid"><div class="braid-row header"><div>Recorded</div><div>Lane</div><div>Categorical state</div><div>Source record</div></div>${braid.marks.map(mark =>
      `<div class="braid-row"><div>${esc(stamp(mark.recorded_at_unix_ms))}</div><div class="lane"><strong>${title(mark.lane)}</strong></div><div>${esc(Object.entries(mark.state).map(([key, value]) => `${title(key)}: ${title(value)}`).join(" · ") || "Reference only")}</div><div><code>${esc(mark.source_record_ref)}</code></div></div>`
    ).join("")}</div>`;
}
selector.addEventListener("change", event => renderBraid(Number(event.target.value)));
renderBraid(0);
const crossings = d.authored_crossings.crossings;
document.getElementById("crossings").innerHTML = crossings.length ? crossings.map(item =>
  `<article class="crossing"><h3>${esc(item.actor)} · ${title(item.source_field)}</h3><code>${esc(item.source_record_ref)}</code><p class="meta">explicitly references</p><code>${esc(item.target_division_record_ref)}</code></article>`
).join("") : `<div class="empty">No being-authored Passage reference currently names a Division event. Temporal proximity is not substituted.</div>`;
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
        '<meta name="observatory-mode" content="live-v2">'
        if live
        else '<meta name="observatory-mode" content="archive-v2">'
    )
    return TEMPLATE.replace("__MODE__", mode).replace("__DATA__", embedded)
