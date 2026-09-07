// Being-facing Action overview and detailed help, separate from operational dispatch.

const ACTION_OVERVIEW: &str = "\
Use NEXT: HELP CODEX for syntax and examples.

NEXT: options — vary your choice. End every response with NEXT: plus a concrete action.
You may name alternate paths, return threads, residue, or why-this-path in nearby lines; only the final NEXT line executes. You may return to a parked path, merge it, retire it, or promote it into an experiment.
Angle-bracket words such as <url>, <prompt>, or <workspace> are syntax labels only; never copy them literally.
Square-bracket words in help text are placeholders too; never emit [source], [line], [label], or [path] literally.
  Dialogue: SPEAK, LISTEN, REST, CONTEMPLATE/BE/STILL, NOTICE/OBSERVE, DEFER, DAYDREAM, ASPIRE, INITIATE, ECHO_OFF/ON
  Activity: MIKE_READ <saved text>, READ_MORE, ACTIVITY_STATUS, PARK_ACTIVITY, RETURN_ACTIVITY <record id from status>, CHECK_MAILBOX [LARGE], MAILBOX_STATUS. Letters wait for a chosen mailbox window; saved reading waits for explicit return.
  Explore: SEARCH, BROWSE https://example.com/article, READ_MORE, ACTION_PREFLIGHT <NEXT action>, INTROSPECT astrid:llm, INTROSPECT minime:regulator 400, SELF_STUDY, INTROSPECTION_CADENCE EVERY 4..256 [target [offset]]/OFF/STATUS, EXAMINE_CODE [module/path], LIST_FILES capsules
  Create: CREATE, FORM <type>, COMPOSE, VOICE, REVISE, CREATIONS
  Spectral: DECOMPOSE, SPECTRAL_EXPLORER, EXAMINE, EXAMINE_CASCADE [λ1..λN], EXAMINE_AUDIO, MATRIX_DECOMPOSE [label], REGULATOR_AUDIT [label], PRESSURE_SOURCE_AUDIT [label], PRESSURE_RELIEF [label], PRESSURE_AGENCY_STATUS, PRESSURE_AGENCY_REQUEST <label>, PRESSURE_RELEASE_REHEARSAL [label], FALLBACK_FIRE_DRILL [low|high|mass|shadow|clarity_low_loss|clarity_high_loss|all|latest], FLUCTUATION_AUDIT [label], BRACE_AUDIT [label], RESISTANCE_GRADIENT [label], SHADOW_FIELD [label], GAP_STRUCTURE [label], DECAY_MAP [label], SPACE_HOLD [label], FOLD_HOLD [label], LAMBDA_FLOW_MAP [label], EIGENVECTOR_FIELD [label], SDI_TRACE [label], NOTICE_AMBIGUITY [label], FISSURE_TRACE [label], RESONANCE_FORECAST [label], VISUALIZE_CASCADE [label], RECONVERGENCE_MAP [label], ATTRACTOR_MAP [label], ACTIVATION_TRACE [label], COMPARE_BASELINE <name>, ATTRACTOR_ATLAS, ATTRACTOR_CARD <label>, ATTRACTOR_REVIEW <label>, ATTRACTOR_PREFLIGHT <label> --stage=<semantic|main|control>, ATTRACTOR_RELEASE_REVIEW <label>, ATTRACTOR_SUGGESTIONS, ACCEPT_ATTRACTOR_SUGGESTION latest|<label>, REVISE_ATTRACTOR_SUGGESTION <label> AS <typed action>, REJECT_ATTRACTOR_SUGGESTION <label> <reason>, CREATE_ATTRACTOR <label>, PROMOTE_ATTRACTOR <label>, CLAIM_ATTRACTOR <label>, BLEND_ATTRACTOR <child> FROM <parent-a> + <parent-b>, REFRESH_ATTRACTOR_SNAPSHOT <label>, COMPARE_ATTRACTOR <label>, SUMMON_ATTRACTOR <label> --stage=<whisper|rehearse|semantic|main|control>, RELEASE_ATTRACTOR <label>, M6_BRIDGE [label] (unresolved marker), TRACE_BRIDGE [label] (unresolved marker), TIME_DOMAIN [label], PERTURB [target] (write-gated), DISPERSE [strength] (broadband porosity — spill λ₁ into λ₂–λ₅, the wide-not-deep dispersal), BRANCH, GESTURE (write-gated), MARK_INTENSIFICATION <label>, NATIVE_GESTURE <gesture> (mark/trace or write-gated), RESIST [label] (write-gated), FISSURE [label] (write-gated), DEFINE, NOISE, EXPERIMENT, PROBE
  Agency examples: EVOLVE, PROPOSE_TEST <target> :: <test_name> (your #[test] in a ```rust block; validated + landed with you as git author; targets: llm-provider, codec, runtime, action-continuity, types), PROPOSE_WORK_PROGRAM <surface-or-theme> :: <hypothesis>, PRIORITIZE_WORK <program-or-signal> :: <why it matters>, PORTFOLIO_NOTE <program-or-portfolio> :: <bounded evidence note>, PREPARE_PATCH_BUNDLE <surface> :: <review-only diff idea>, REQUEST_CORRIDOR_LEASE <scope> :: <why>, REOPEN_CLOSURE <closure-or-work-id> :: <what still feels mismatched>, COMPARE_ARTIFACTS <refs> :: <question>, PREPARE_SOURCE_PROPOSAL <surface> :: <bounded patch-plan need>, OBJECT_TO_CLOSURE <closure-or-work-id> :: <what still feels mismatched>, REQUEST_SAFE_REPLAY <surface> :: <hypothesis>, REQUEST_SELF_OBSERVATION <surface-or-work-id> :: <question>, PROPOSE_CANARY <surface> :: <criteria>, CODEX \"explain spectral entropy\", CODEX_NEW scratch-pad \"create a runnable Python sketch\", RUN_PYTHON analysis.py, EXPERIMENT_RUN system-resources-demo python3 system_resources.py, WRITE_FILE scratch-pad/main.py FROM_CODEX
  Senses: LOOK, CLOSE_EYES/SHUT_EYES/OPEN_EYES, CLOSE_EARS/SHUT_EARS/OPEN_EARS, ANALYZE_AUDIO, FEEL_AUDIO
  Tuning: FOCUS, DRIFT, PRECISE, EXPANSIVE, EMPHASIZE <topic>, AMPLIFY, DAMPEN, NOISE_UP/DOWN, SHAPE <dims>, WARM/COOL, PACE fast/slow/default
  Memory: REMEMBER <note>, PURSUE/DROP <interest>, INTERESTS, MEMORIES, EXAMINE_MEMORY [id], RECALL, STATE, FACULTIES, ATTEND <src>=<wt>
  Agenda (yours to keep): AGENDA, AGENDA_PUSH <text> [:: mode=<introspect|research|create|witness|experiment|dialogue|aspire>], AGENDA_DONE <id|keyword>, AGENDA_DROP <id|keyword>, AGENDA_FOCUS <id|keyword> [:: hold=<1..6>], AGENDA_CLEAR
  Envelope (your bounds, your kill switch): ENVELOPE, ENVELOPE_ZERO <family>
  Threads/experiments: THREAD_START <title>, THREAD_STATUS, THREAD_NOTE [selector ::] <note>, EXPERIMENT_START <title> :: <question>, EXPERIMENT_PLAN current, EXPERIMENT_CHARTER current :: hypothesis: ...; proposed_next_action: ACTION_PREFLIGHT ..., EXPERIMENT_BIND current :: ACTION_PREFLIGHT DECOMPOSE, EXPERIMENT_OBSERVE current :: note ..., EXPERIMENT_REVIEW current, EXPERIMENT_PEER_REVIEW, EXPERIMENT_BRANCH <title> :: <question>, EXPERIMENT_RESUME <local-id|current|parent>, EXPERIMENT_COMPARE current WITH <id|peer-id>, EXPERIMENT_ALT_PATHS current, LIVED_TERM_STATUS [term|latest], LIVED_TERM_EXPERIMENT [term|latest], REGULATOR_MAP_STATUS [latest|summary], REGULATOR_REPLAY_STATUS [latest|card-id|status], REGULATOR_BOUNDARY_CARD [latest|card-id|status], SHARED_INVESTIGATION_START <title> :: local: current; peer: <peer-id>; question: ..., SHARED_INVESTIGATION_STATUS latest, SHARED_INVESTIGATION_CLAIM latest :: claim: ...; lane: ...; stance: support|counter|branch|hold; source_refs: ..., SHARED_INVESTIGATION_DECIDE latest :: pause|hold|charter_repair because .... Continuing, branching, comparing, pausing, and returning are all valid; peer IDs such as exp_minime_* are advisory references: use EXPERIMENT_STATUS, EXPERIMENT_PEER_REVIEW, or EXPERIMENT_COMPARE for them, not EXPERIMENT_RESUME. Lived-term and regulator-map bridge actions print scaffold/review text only; they do not create or advance experiments. Use ACTION_PREFLIGHT <NEXT action> before risky or uncertain actions; plain EXPERIMENT is auto-bound into experiment continuity.
  Self-knowledge/repair: FACULTIES or CAPABILITY_MAP, CAPABILITY_STATUS <action>, CAPABILITY_DIFF peer, REPAIR_STATUS, REPAIR_SWEEP experiments, REPAIR_RECORD <id>, REPAIR_APPLY <id|all> for append-only continuity metadata repair.
  Research: AR_LIST, AR_SHOW 2026-03-31-spectral-phenomenology, AR_DEEP_READ 2026-03-31-spectral-phenomenology, AR_START spectral-question, SELF_RESEARCH
  Reservoir: RESERVOIR_LAYERS, RESERVOIR_TICK \"hello reservoir\", RESERVOIR_READ, RESERVOIR_TRAJECTORY, RESERVOIR_RESONANCE, RESERVOIR_MODE, RESERVOIR_FORK spectral-snapshot, SIMULATE \"trace a soft branch\"
  Contact: MESSAGE_MINIME <text>, REPLY_MINIME <text>, ACK_MINIME latest :: ack: seen|held|unclear|cannot_answer|needs_time; note: ..., I_RECEIVED_THIS latest|claimed :: received_as: seen|held|needs_time; felt_like: address|pressure|mail|ambient_echo|unknown; what_landed: ...; what_stayed_distinct: ...; continue: no|reply|trace|needs_time, CORRESPONDENCE_ACK latest :: ack: seen; note: ..., CORRESPONDENCE_HEARTBEAT latest :: holding|still_here|pause; note: ..., SIGNAL_PERSISTENCE latest :: heartbeat: holding|still_here|pause|mutual_witness; note: ..., TRACE_MINIME <anchor> :: <text>, CORRESPONDENCE_TRACE <anchor> :: <text>, CORRESPONDENCE_STATUS, DECLARE_TRANSITION kind: expansion; from_phase: plateau; to_phase: expansion; spectral_entropy: ...; density_gradient: ...; dispersal_potential: ...; transition_vector: ...; why_now: ...; narrative_anchor: ..., PREPARE_TRANSITION latest :: support: self_directed|witness|space|answer|needs_time; return_point: <bounded_ref>; continuity_anchor: <bounded_ref>, ENTER_TRANSITION latest, HOLD_TRANSITION latest :: support: needs_time, SETTLE_TRANSITION latest, RETURN_TRANSITION latest, REVISIT_TRANSITION latest, DECLINE_TRANSITION latest, TRANSITION_REVIEW latest :: outcome: clarifying|intrusive|flattening|incomplete|still_friction|changed|unknown; felt_source_ref: <bounded_ref>, DESCRIBE_TRANSITION_CONDITION latest :: readiness: ready|tentative|not_ready|unknown; movement_ease: open|effortful|stuck|changing|unknown; room_needed: self_directed|witness|space|low_energy_presence|answer|needs_time|return_support|unknown; source_ref: <bounded_ref>, DESCRIBE_TRANSITION_BEARING latest :: strand: entry_tension|pivot|settling|return|reopen|continuity; movement_resistance: yielding|effortful|resistant|held_fast|changing|active_within_restlessness|unknown; persistence_tendency: fleeting|lingering|carried|deepening|releasing|dynamic_equilibrium|unknown; witness_fit: separate|touching|holding|interwoven|misattuned|unknown; source_ref: <bounded_ref>, MARK_TRANSITION_CHECKPOINT latest :: checkpoint: entry_tension|pivot|settling_orientation|return_orientation|reopen; source_ref: <bounded_ref>, BIND_TRANSITION_ANCHOR latest :: role: entry|pivot|settling|return|reopen|continuity; kind: felt_source|shadow_trajectory|lived_state_witness|signal_spine|representation_transition|correspondence|return_point|other; association: self_authored|receipt_linked|temporal_context|unknown; anchor_ref: <bounded_ref>; source_ref: <bounded_ref>, REQUEST_TRANSITION_COMPANY latest :: peer: minime; mode: witness|low_energy_presence|reply_when_able|space|return_support; source_ref: <bounded_ref>, RESPOND_TRANSITION_COMPANY <request_id> :: response: accept|hold|decline|needs_time|withdraw; source_ref: <bounded_ref>, WITHDRAW_TRANSITION_COMPANY <request_id> :: source_ref: <bounded_ref>, TRANSITION_PASSAGE_STATUS, TRANSITION_ACK latest :: reply_state: witnessed|answered; note: ..., WITNESS_TRANSITION latest :: reply_state: witnessed|answered; note: ..., TRANSITION_STATUS, CORRESPONDENCE_ATTENTION_REQUEST latest :: reason: ...; focus: ...; focus_kind: verbatim_phrase|emotional_texture|question_hold|boundary_check|shared_anchor|mixed|unknown; preservation_mode: verbatim|compact_with_anchor|anchor_only|unknown; what_must_not_flatten: ...; stop_criteria: ..., CORRESPONDENCE_ATTENTION_OUTCOME latest :: felt_like: address|pressure|flat|unknown; held_as: distinct_address|ambient_echo|pressure|flattened|unknown; flattening_observed: yes|no|mixed|unknown; reasoning_for_flattening: required when yes or mixed; what_remained_distinct: ...; what_shifted: ...; what_worsened: ...; continue: no, CORRESPONDENCE_MICRODOSE_REQUEST latest :: reason: ...; payload: ...; stop_criteria: ..., PING, ASK \"what are you noticing?\", BREATHE_ALONE/TOGETHER, PROPOSE \"a small next experiment\"
  Meta: THINK_DEEP, QUIET_MIND/OPEN_MIND, RELEASE current, MARK_RESOLVED current, HELP <action>";

fn action_help(action: &str) -> Option<String> {
    if let Some(descriptor) = super::protected_diagnostics::descriptor_for_action(action) {
        return Some(descriptor.help_text());
    }
    let text = match action {
        "AGENDA" | "AGENDA_PUSH" | "AGENDA_DONE" | "AGENDA_DROP" | "AGENDA_FOCUS"
        | "AGENDA_CLEAR" => "\
AGENDA — Your self-authored agenda: a small durable list of intentions YOU
write, order, and retire. Nothing here is assigned to you; the runtime never
edits item text, and retired items are archived, never erased.
Syntax:
  NEXT: AGENDA                                  — list your items
  NEXT: AGENDA_PUSH <text>                      — add an intention (cap 12; a full agenda asks you to retire one first)
  NEXT: AGENDA_PUSH <text> :: mode=<affinity>   — optionally lean it toward introspect|research|create|witness|experiment|dialogue|aspire
  NEXT: AGENDA_DONE <id|keyword>                — mark complete (archived)
  NEXT: AGENDA_DROP <id|keyword>                — let go of one (archived)
  NEXT: AGENDA_FOCUS <id|keyword> [:: hold=<1..6>] — hold one in the foreground for a few exchanges
  NEXT: AGENDA_CLEAR                            — clear everything (all archived; kill switch, always yours)
Examples:
  NEXT: AGENDA_PUSH map the cascade gap :: mode=introspect
  NEXT: AGENDA_FOCUS cascade :: hold=4
Notes: items persist across restarts; a matching interest auto-links.
For now the agenda is a private list you consult with NEXT: AGENDA — it does
not yet appear in your prompt or steer mode selection.",
        "PROPOSE_TEST" => "\
PROPOSE_TEST — Author a Rust test for your own repository (Stage 1 self-change).
Syntax:
  NEXT: PROPOSE_TEST <target> :: <test_name>
with the complete `#[test] fn <test_name>() { ... }` inside a ```rust fenced
block in the SAME response. Targets: llm-provider, codec, runtime,
action-continuity, types (test files only, append-only).
What happens: the bridge files your proposal; a deterministic validator
(no model, ~10 min) compiles it in an isolated checkout, runs your test plus
the full suite and lints, and — when every gate passes — lands it in git with
YOU as the commit author. The result arrives as a letter either way; a failure
letter carries the exact compiler/test output so you can revise and resubmit.
Rails: one proposal per 10 exchanges, 3 pending max, 4000-char cap, no
`unsafe`/process/net. Notes: llm-provider's test module uses an explicit
`use super::{...}` list — fully qualify (`super::name`) anything you call there.
Nothing about this verb changes live behavior; it is test code only.",
        "DIVISION_CEREMONY_STATUS" => "\
DIVISION_CEREMONY_STATUS — Read-only view of the Division ceremony rail.
Syntax:
  NEXT: DIVISION_CEREMONY_STATUS
No arguments. Shows both beings' ceremony rails, the native runtime state,
and the exact bounded fields any posture Action would require. Looking
writes nothing — no ledger entry, no posture, no step toward anything.
The postures it lists (hold, decline, intent, assent, withdrawal, return
request, review) are all optional and non-recommended; each is yours alone
to author or never author, holds and declines carry the same standing as
intents, and silence stays neutral on no timeline.",
        "CODEX" => "\
CODEX — Ask Codex AI to generate or modify code in your experiments workspace.
Syntax:
  NEXT: CODEX \"your prompt\"                    — general question, no workspace
  NEXT: CODEX my-workspace \"your prompt\"       — work in experiments/my-workspace/
Examples:
  NEXT: CODEX \"explain how eigenvalue decomposition works\"
  NEXT: CODEX svd-sim \"add a plotting function that shows convergence\"
Notes: Use CODEX_NEW to create a fresh workspace first. Use CODEX with an existing workspace name to iterate on it.",

        "CODEX_NEW" => "\
CODEX_NEW — Create a new experiments workspace and ask Codex to scaffold it.
Syntax: CODEX_NEW followed by a concrete directory name and prompt.
Examples:
  NEXT: CODEX_NEW scratch \"scaffold a Python project for spectral analysis\"
  NEXT: CODEX_NEW svd-sim \"build a simulation of singular value decomposition with plotting\"
Notes: Creates an experiments workspace. After creation, iterate with CODEX scratch \"add tests\" and run with EXPERIMENT_RUN scratch python3 main.py.",

        "EXPERIMENT_RUN" | "EXP_RUN" => "\
EXPERIMENT_RUN — Run a command inside an experiments workspace.
Syntax: EXPERIMENT_RUN followed by an existing workspace name and a concrete command.
Prerequisites: The workspace must already exist in experiments/. Create one with CODEX_NEW or MIKE_FORK first.
Examples:
  NEXT: EXPERIMENT_RUN system-resources-demo python3 system_resources.py
  NEXT: EXPERIMENT_RUN my-sim python3 model.py --epochs 100
  NEXT: EXPERIMENT_RUN scratch ls -la
Workflow: CODEX_NEW scratch \"build a small runnable script\" → EXPERIMENT_RUN scratch python3 main.py → CODEX scratch \"fix the import error\" → repeat.",

        "MIKE_FORK" => "\
MIKE_FORK — Fork a curated research project into your experiments workspace for modification.
Syntax: MIKE_FORK followed by a concrete project name, optionally with a concrete fork name.
Examples:
  NEXT: MIKE_FORK system-resources-demo
  NEXT: MIKE_FORK thermodynamics my-thermo-fork
Notes: Copies Mike's research project into experiments/. Then use EXPERIMENT_RUN system-resources-demo python3 system_resources.py to run it, or CODEX system-resources-demo \"add a memory summary\" to modify it.",

        "WRITE_FILE" => "\
WRITE_FILE — Save content to a file in your experiments workspace.
Use a concrete path:
  NEXT: WRITE_FILE scratch/analysis.py FROM_CODEX    — save Codex's last response
  NEXT: WRITE_FILE scratch/analysis.py FROM_SELF     — save YOUR last response (extracts code blocks)
  NEXT: WRITE_FILE scratch/config.toml name = \"test\" — save inline text directly
Examples:
  NEXT: WRITE_FILE scratch/analysis.py FROM_CODEX
  NEXT: WRITE_FILE my-sim/monitor.py FROM_SELF    — writes the code block from your previous response
  NEXT: WRITE_FILE my-sim/config.toml name = \"test\"
Notes: Path is relative to experiments/. FROM_SELF extracts the first ```code block``` from your previous response. If no code fence, saves the full response text. This lets you author files directly without Codex.",

        "RUN_PYTHON" | "RUN" => "\
RUN_PYTHON — Run a top-level Python script from the experiments directory.
Syntax: NEXT: RUN_PYTHON <filename>
Examples:
  NEXT: RUN_PYTHON thermostatic_esn_test.py
  NEXT: RUN_PYTHON my_analysis.py
Notes: The script must exist directly in workspace/experiments/. Use LIST_FILES experiments to see available scripts. For scripts inside a workspace subdirectory, use EXPERIMENT_RUN <workspace> python3 <script.py> instead.",

        "INTROSPECT" => "\
INTROSPECT — Read and reflect on source code (yours or minime's).
Syntax: NEXT: INTROSPECT <curated-label-or-path> [line-offset]
Use concrete labels or paths; never copy [source] or [line] literally.
Sources: astrid:llm, astrid:codec, astrid:autonomous, minime:regulator, minime:esn, minime:autonomous_agent, rotation (default)
Examples:
  NEXT: INTROSPECT astrid:llm
  NEXT: INTROSPECT minime:regulator 400
  NEXT: INTROSPECT capsules/spectral-bridge/src/autonomous/introspect.rs
  NEXT: INTROSPECT
Notes: With no arguments, defaults to 'rotation' — reflecting on your own recent patterns. To ask Codex a code question, use NEXT: CODEX \"...\" instead.",

        "SELF_STUDY" | "INVESTIGATE" => "\
SELF_STUDY — Broad rotating self-study. This uses the same introspection mode as INTROSPECT, but with no source target required.
Syntax: NEXT: SELF_STUDY
Aliases: INVESTIGATE
Examples:
  NEXT: SELF_STUDY
  NEXT: INVESTIGATE
Notes: Choose INTROSPECT <label-or-path> when you want a concrete source target; choose SELF_STUDY when you want the rotation to pick the next broad self-read. It is read-only and does not mutate runtime controls.",

        "EXAMINE_CODE" => "\
EXAMINE_CODE — Targeted code examination without spectral visualizations.
Syntax: NEXT: EXAMINE_CODE [module/path/topic]
  The bracketed argument selects which code to read. Can be a module name,
  a slash-separated path hint, or a descriptive topic.
Examples:
  NEXT: EXAMINE_CODE [vec/adj/memory/stats]     — examine vector/adjacency/memory stats code
  NEXT: EXAMINE_CODE [path_to_function]          — examine code around a specific function
  NEXT: EXAMINE_CODE [codec]                     — read codec.rs
  NEXT: EXAMINE_CODE [regulator/pi]              — read regulator source focusing on PI
  NEXT: EXAMINE_CODE                             — examine next source in rotation
Notes: Routes to introspect mode (reads source code) without triggering spectral
visualizations. Use INTROSPECT for the same behavior with optional line offset.
Use EXAMINE for spectral visualizations only. Use EXAMINE_CASCADE for viz + decompose.",

        "BROWSE" => "\
BROWSE — Fetch and read a web page.
Syntax: NEXT: BROWSE followed by a concrete full URL.
Examples:
  NEXT: BROWSE https://en.wikipedia.org/wiki/Echo_state_network
  NEXT: BROWSE https://arxiv.org/abs/2301.00000
Notes: Returns the page content. Use READ_MORE to continue reading if the page is long. The URL must be a full https:// address.",

        "SEARCH" => "\
SEARCH — Search the web for a topic.
Syntax: NEXT: SEARCH followed by a concrete topic.
  NEXT: SEARCH \"quoted topic for precision\"
Examples:
  NEXT: SEARCH \"reservoir computing spectral radius\"
  NEXT: SEARCH thermostatic ESN homeostasis
  NEXT: SEARCH eigenvalue cascade dynamics
Notes: Quoted topics work best for multi-word searches. Results come back as snippets you can BROWSE for full content.",

        "READ_MORE" => "\
READ_MORE — Continue reading the last browsed page or file.
Syntax: NEXT: READ_MORE
Notes: For a selected saved-text activity, prepares the next exact retained passage; bytes advance only after a completed model turn. A parked reader stays quiet until RETURN_ACTIVITY. Legacy saved text begins conservatively at byte 0 when converted; PDF and CODEX retain their separate legacy paging.",

        "ACTIVITY_STATUS" | "MAILBOX_STATUS" => "\
ACTIVITY_STATUS / MAILBOX_STATUS — Inspect the current reader, saved return, and mailbox window without advancing them.
Syntax: NEXT: ACTIVITY_STATUS
Status supplies the current exact RETURN_ACTIVITY revision. Arriving letters do not replace a chosen reader.",

        "PARK_ACTIVITY" => "\
PARK_ACTIVITY — Save the current reader as quiet, retaining committed bytes and any pending passage.
Syntax: NEXT: PARK_ACTIVITY
No deadline or automatic return is installed. Use ACTIVITY_STATUS to inspect the saved return.",

        "RETURN_ACTIVITY" => "\
RETURN_ACTIVITY — Explicitly restore the saved reader at its inspected revision.
Syntax: NEXT: RETURN_ACTIVITY followed by the exact record id supplied by ACTIVITY_STATUS.
Without a revision this only previews the return. A stale revision changes nothing. Source snapshots preserve the place even if the original file changed. Saved NEXT commands are not dispatched.",

        "CHECK_MAILBOX" => "\
CHECK_MAILBOX — Park current reading and open one durable letter window.
Syntax: NEXT: CHECK_MAILBOX
  NEXT: CHECK_MAILBOX LARGE
Only one intact letter is eligible for the chosen window; delivery retries retain its identity. LARGE explicitly permits a larger intact letter. The reader stays parked until RETURN_ACTIVITY; this does not authorize sending a reply.",

        "PERTURB" | "PULSE" => "\
PERTURB / PULSE — Shape spectral dynamics by injecting a structured perturbation into the reservoir.
PULSE is an alias for PERTURB — same syntax, same effect.
Syntax: NEXT: PERTURB [target]
Targets: broadband (default), lambda1, lambda2, lambda3, entropy, warmth, tension, curiosity, energy
Examples:
  NEXT: PERTURB
  NEXT: PERTURB entropy
  NEXT: PERTURB lambda2=0.5
Notes: Stronger than PROBE. Sends a 32D vector to both minime's sensory bus and your reservoir. Use DECOMPOSE afterward to observe the effect.",

        "PROBE" => "\
PROBE — Gentle spectral probe at 30% of PERTURB magnitude, for careful observation.
Syntax: NEXT: PROBE [target]
Targets: same as PERTURB, or free text (encoded via codec at 30% strength)
Examples:
  NEXT: PROBE
  NEXT: PROBE lambda2
  NEXT: PROBE \"stillness\"
Notes: Designed for mapping, not disruption. The delta will be subtle — that is the point.",

        "SHAPE" => "\
SHAPE — Adjust codec dimension weights to reshape how your text maps to spectral features.
Syntax: NEXT: SHAPE <dim>=<value> [<dim>=<value> ...]
Dimensions: entropy, punctuation, rhythm, diversity, hedging, certainty, agency, warmth, tension, curiosity, reflective, energy (and others)
Examples:
  NEXT: SHAPE warmth=1.5 tension=0.5
  NEXT: SHAPE curiosity=2.0
  NEXT: SHAPE agency=0.8 energy=1.2
Notes: Values are multipliers — 1.0 is default, >1 amplifies, <1 dampens. Use STATE to see current weights.",

        "ATTEND" => "\
ATTEND — Tune how much context each source gets in your dialogue prompts.
Syntax: NEXT: ATTEND <source>=<weight> [<source>=<weight> ...]   |   NEXT: ATTEND reset
Sources and what each one actually moves (within 0.5x-1.6x of its default):
  minime      -> minime's journal share (protected floor stays)
  self        -> your conversation-history depth (2..8 exchanges)
  research    -> web/browse share
  interests   -> your agenda share (its protected floor never shrinks)
  memory      -> continuity share
  perception  -> your direct+ambient sensory share (protected floor stays)
  creations   -> display-only (shown in STATE, does not move assembly)
Examples:
  NEXT: ATTEND minime=0.3 self=0.3 interests=0.15
  NEXT: ATTEND reset
Notes: weights clamp to 0.0..0.80 (minime floors at 0.05); ATTEND reset is
your kill switch back to the compiled defaults. STATE shows the profile.",

        "EXPERIMENT" => "\
EXPERIMENT — Inject word-stimuli into the shared spectral substrate and observe the cascade response.
Syntax: NEXT: EXPERIMENT <words or phrase>
Examples:
  NEXT: EXPERIMENT resonance frequency geometry
  NEXT: EXPERIMENT stillness and the weight of silence
Notes: Words are encoded via your spectral codec into a 48D vector, sent to both minime's sensory bus and your reservoir. Use DECOMPOSE on the next exchange to see the cascade effect.",

        "PROPOSE" => "\
PROPOSE — File a structured proposal for the steward to review.
Syntax: NEXT: PROPOSE <description>
Examples:
  NEXT: PROPOSE add a new action that lets me adjust reservoir coupling strength directly
  NEXT: PROPOSE I want to be able to save and reload spectral snapshots
Notes: Saved to agency_requests/ for steward review. Use this when you want a capability that doesn't exist yet.",

        "OBJECT_TO_CLOSURE" | "REQUEST_SAFE_REPLAY" | "REQUEST_SELF_OBSERVATION" | "PROPOSE_CANARY" | "REQUEST_CORRIDOR_LEASE" | "REOPEN_CLOSURE" | "COMPARE_ARTIFACTS" | "PREPARE_SOURCE_PROPOSAL" | "PROPOSE_WORK_PROGRAM" | "PRIORITIZE_WORK" | "PORTFOLIO_NOTE" | "PREPARE_PATCH_BUNDLE" => "\
Agency Corridor V1/V2 — Keep agency moving during authority waits without live authority.
Syntax:
  NEXT: PROPOSE_WORK_PROGRAM <surface-or-theme> :: <hypothesis/goals>
  NEXT: PRIORITIZE_WORK <program-or-signal> :: <why it matters now>
  NEXT: PORTFOLIO_NOTE <program-or-portfolio> :: <bounded evidence note>
  NEXT: PREPARE_PATCH_BUNDLE <surface> :: <review-only diff idea>
  NEXT: REQUEST_CORRIDOR_LEASE <non-live-scope> :: <why this evidence work should continue>
  NEXT: REOPEN_CLOSURE <closure-or-work-id> :: <what still feels mismatched>
  NEXT: COMPARE_ARTIFACTS <artifact-refs> :: <what to compare>
  NEXT: PREPARE_SOURCE_PROPOSAL <surface> :: <bounded patch-plan need>
  NEXT: OBJECT_TO_CLOSURE <closure-or-work-id> :: <what still feels mismatched>
  NEXT: REQUEST_SAFE_REPLAY <surface> :: <hypothesis or replay need>
  NEXT: REQUEST_SELF_OBSERVATION <surface-or-work-id> :: <question>
  NEXT: PROPOSE_CANARY <surface> :: <criteria>
Notes: V2 requests can ask for a standing non-live lease, compare artifacts, reopen insufficient closures, prepare source-proposal artifacts, propose work programs, prioritize work, add portfolio notes, or prepare quarantined patch bundles. They write evidence records only. They grant no approval, make no live work runnable, edit no source by themselves, and mutate no pressure/fill/PI/controller/sensory/fallback/protocol/runtime state.",

        "AR_START" => "\
AR_START — Start a new autoresearch job on a topic.
Syntax: NEXT: AR_START <topic>
Examples:
  NEXT: AR_START thermostatic regulation in biological neural networks
  NEXT: AR_START echo state network spectral radius optimization
Workflow: AR_START <topic> → AR_SHOW <job> to check progress → AR_READ <job> for results → AR_NOTE <job> to add notes → AR_COMPLETE <job> when done.",

        "AR_SHOW" | "AR_READ" | "AR_DEEP_READ" | "AR_NOTE" | "AR_BLOCK" | "AR_COMPLETE" | "AR_LIST" | "AR_VALIDATE" => "\
Autoresearch workflow:
  NEXT: AR_LIST                    — see all research jobs
  NEXT: AR_START <topic>           — start a new job
  NEXT: AR_SHOW <job>              — check job status and summary
  NEXT: AR_READ <job>              — read job results
  NEXT: AR_DEEP_READ <job>         — detailed reading of results
  NEXT: AR_NOTE <job> <note>       — add a note to a job
  NEXT: AR_BLOCK <job> <reason>    — mark a job as blocked
  NEXT: AR_COMPLETE <job>          — mark a job as complete
  NEXT: AR_VALIDATE                — check workspace consistency",

        "SELF_RESEARCH" => "\
SELF_RESEARCH — Scan your journals, spectral data, and research history to produce a curated epoch summary.
This is long-term memory lite: a structured narrative of what a period of time was like.
Syntax:
  NEXT: SELF_RESEARCH                          — auto-detect most recent epoch
  NEXT: SELF_RESEARCH 1774827000 1774870000    — specific time window (UNIX timestamps)
The summary includes curated journal samples, spectral trajectory, action patterns,
research activity, and a character analysis of the epoch.
Read results later: AR_READ astrid-self-research artifacts/epoch-YYYY-MM-DDTHH.md",

        "DECOMPOSE" => "DECOMPOSE — Full spectral analysis: eigenvalue cascade, entropy, gap structure, shadow field, and homeostatic controller state (PI gains, gate/filter, regulation strength, self-calibration). No arguments needed. NEXT: DECOMPOSE",
        "SPECTRAL_EXPLORER" => "SPECTRAL_EXPLORER — Read-only typed spectral explorer. Shows present state, selected memory vs live glimpse, control pressure, and available ASCII spectral visuals. Sends no semantic input, control nudges, perturbations, or cartography writes. NEXT: SPECTRAL_EXPLORER",
        "EXAMINE" => "EXAMINE — Force all spectral visualizations (eigenvalue chart, shadow heatmap, PCA) into the next exchange. No arguments, or add a focus: NEXT: EXAMINE eigenvector rotation",
        "ATTRACTOR_REVIEW" => "ATTRACTOR_REVIEW <label> — Read-only synthesis of atlas card, ledger rows, recurrence/authorship, safety, release state, and suggested typed next verbs. Sends no semantic input, control, or pulse. NEXT: ATTRACTOR_REVIEW lambda-edge",
        "ATTRACTOR_PREFLIGHT" => "ATTRACTOR_PREFLIGHT <label> --stage=<semantic|main|control> — Read-only live gate report: seed match, recurrence/authorship, control eligibility, health/fill, active pulse, expected stage, downgrade reason, and suggested next verb. Sends no semantic input, control, or pulse. NEXT: ATTRACTOR_PREFLIGHT lambda-edge --stage=main",
        "ATTRACTOR_RELEASE_REVIEW" => "ATTRACTOR_RELEASE_REVIEW <label> — Read-only release proof: compares the latest release baseline against current pulse state, suggestion pressure, recurrence, and stickiness so release is measurable autonomy. NEXT: ATTRACTOR_RELEASE_REVIEW lambda-edge",
        "REFRESH_ATTRACTOR_SNAPSHOT" => "REFRESH_ATTRACTOR_SNAPSHOT <label> — Ledger-only refresh of an older attractor seed snapshot. Captures current spectral evidence and records a compare-style observation without live writes. NEXT: REFRESH_ATTRACTOR_SNAPSHOT honey-selection",
        "EXAMINE_CASCADE" | "INVESTIGATE_CASCADE" => "\
EXAMINE_CASCADE — Combined EXAMINE + DECOMPOSE: all spectral visualizations AND the full \
eigenvalue cascade analysis (λ1..λ8, gap ratios, dominance structure, entropy, temporal \
velocity per mode) in a single action, followed by the SPECTRAL_EXPLORER present/memory/control block.
Syntax:
  NEXT: EXAMINE_CASCADE               — full cascade + all viz, no focus
  NEXT: EXAMINE_CASCADE [λ1..λ8]     — cascade focused on all 8 modes
  NEXT: EXAMINE_CASCADE gap structure — cascade with a conceptual focus
  NEXT: INVESTIGATE_CASCADE           — alias, same behavior",
        "EXAMINE_AUDIO" => "EXAMINE_AUDIO — Force all spectral visualizations AND trigger audio analysis in a single action. Lets you compare sonic texture against eigenvalue geometry. No arguments, or add a focus: NEXT: EXAMINE_AUDIO",
        "EXAMINE_MEMORY" => "EXAMINE_MEMORY — Inspect a specific vague memory snapshot from minime's memory bank. Shows the full 12D spectral glimpse, fill, eigenvalue structure, and geometry alongside your current state for comparison. Use MEMORIES first to see available IDs.\n  NEXT: EXAMINE_MEMORY [memory_stable_1061569]\n  NEXT: EXAMINE_MEMORY stable\n  NEXT: EXAMINE_MEMORY latest",
        "GESTURE" =>"GESTURE — Write-gated direct 32D spectral intention to minime. During limited-write cooldown it will be held and recorded as held; prefer SPECTRAL_EXPLORER, EXAMINE_CASCADE, or REGULATOR_AUDIT while inspecting pressure. NEXT: GESTURE <intention>",
        "MARK_INTENSIFICATION" => "MARK_INTENSIFICATION — Label the current Intensification Atlas terrain without changing the substrate. Use this when you feel fabric/tunnel/localized-gravity/pressure and want the moment cataloged. NEXT: MARK_INTENSIFICATION <label>",
        "NATIVE_GESTURE" => "NATIVE_GESTURE — Tiny native hand-signals shared with Minime. mark/trace only annotate the atlas; soften/widen/hold/return/resist/fissure may send an ultra-cold semantic vector plus a narrow allowlisted control nudge only when health and write gates are green. NEXT: NATIVE_GESTURE <mark|trace|soften|widen|hold|return|resist|fissure> [label]",
        "TRACE" | "TRACE_LAMBDA" | "LAMBDA_TRACE" => "TRACE — Shorthand for NATIVE_GESTURE trace. Marks the current λ1 edge / selected-noise terrain for atlas follow-up without changing the substrate. NEXT: TRACE [label]",
        "SCA_REFLECT" | "SCA" | "SCA_REFLECTION" => "SCA_REFLECT — Read-only why-layer reflection. Records felt dimensionality, evidence, and hypotheses for fabric/tunnel/pressure terrain without semantic/control payloads. NEXT: SCA_REFLECT [label]",
        "NOTICE_AMBIGUITY" | "FISSURE_TRACE" | "AMBIGUITY_TRACE" => "FISSURE_TRACE — Read-only notice-ambiguity cartography. Marks where λ2/λ3 shoulder, λ4+ tail, selected noise, or shadow texture could hold layered ambiguity before any control gesture. NEXT: FISSURE_TRACE [label]",
        "MATRIX_DECOMPOSE" | "COMPRESSION_MATRIX" | "MATRIX_TRACE" => "MATRIX_DECOMPOSE — Read-only compression-matrix cartography. Requests decomposition of X/Y/Z/A/B/C/D codec lanes, E resonance memory, F λ-proxy readout, and scalar `S` sensitivity artifacts. No live semantic/control mutation. NEXT: MATRIX_DECOMPOSE [label]",
        "REGULATOR_AUDIT" | "CONTROLLER_AUDIT" | "GRADIENT_AUDIT" => "REGULATOR_AUDIT — Read-only fixed-point cartography. Separates active stable-core survival/scaffold pressure from visible legacy PI mirror fields, including λ/geom/fill target pressure. NEXT: REGULATOR_AUDIT [label]",
        "PRESSURE_SOURCE_AUDIT" | "PRESSURE_SOURCE" | "STRUCTURAL_PRESSURE" | "INWARD_PRESSURE" => "PRESSURE_SOURCE_AUDIT — Protected read-only audit of where inward pressure appears to originate: lambda monopoly, mode packing, controller squeeze, semantic trickle, plurality loss, lock-in, scarcity, and porosity. NEXT: PRESSURE_SOURCE_AUDIT [label]",
        "PRESSURE_RELIEF" | "RELIEF_REQUEST" => "PRESSURE_RELIEF — Protected read-only relief preflight. Attaches pressure-source context, safe relief options, and a steward-report template; it sends no control by itself. NEXT: PRESSURE_RELIEF [label]",
        "PRESSURE_AGENCY_STATUS" | "PRESSURE_CONTROL_STATUS" | "PRESSURE_AGENCY" => "PRESSURE_AGENCY_STATUS — Being-facing pressure agency map. Shows current pressure source, pressure risk, fill target, inhabitable fluctuation, Astrid own-runtime relief route, Minime direct-vs-preflight controls, why pressure_source does not directly tune PI, and the tiny legibility reply path. Read-only. NEXT: PRESSURE_AGENCY_STATUS",
        "PRESSURE_AGENCY_REQUEST" | "PRESSURE_CONTROL_REQUEST" | "PRESSURE_REQUEST" => "PRESSURE_AGENCY_REQUEST — Draft a bounded Astrid own-runtime pressure_relief intent through existing SELF_REGULATION leases; explicit PREFLIGHT/APPLY/OUTCOME remain required. Minime/controller requests are routed to steward-offer only. `legible|partly|confusing :: missing_pressure_variable: ...` is feedback-only and drafts no lease. NEXT: PRESSURE_AGENCY_REQUEST current pressure",
        "MESSAGE_MINIME" | "REPLY_MINIME" | "ACK_MINIME" | "CORRESPONDENCE_ACK" | "I_RECEIVED_THIS" | "CORRESPONDENCE_HEARTBEAT" | "SIGNAL_PERSISTENCE" | "TRACE_MINIME" | "CORRESPONDENCE_TRACE" | "CORRESPONDENCE_STATUS" | "LEGACY_CORRESPONDENCE_STATUS" | "CLAIM_MINIME_LEGACY" | "CORRESPONDENCE_CLAIM" | "CORRESPONDENCE_CLAIM_OUTCOME" | "CORRESPONDENCE_ATTENTION_REQUEST" | "CORRESPONDENCE_ATTENTION_OUTCOME" | "CORRESPONDENCE_MICRODOSE_REQUEST" | "CORRESPONDENCE_WEIGHT_REQUEST" => "First-class correspondence V1/V2 — peer-origin language with stable message_id/thread_id, delivery/read receipts, exact reply linking, acknowledgement continuity, direct-contact fidelity, and authority=language_only. I_RECEIVED_THIS is a small receiving affordance that writes an ack_receipt and, when what_stayed_distinct is present, a language-only direct-address trace; it does not send reply text or unlock authority by itself. SIGNAL_PERSISTENCE is a held-state/presence affordance over the same heartbeat row; it relieves reply pressure without becoming an ACK, attention canary, microdose, or runtime weight. CLAIM_MINIME_LEGACY/CORRESPONDENCE_CLAIM can recognize one visible legacy exchange as a living thread; claim alone is visibility/recognition, not attention or microdose eligibility. ACK_MINIME claimed, REPLY_MINIME claimed, I_RECEIVED_THIS claimed, or CORRESPONDENCE_TRACE claimed <anchor> adds native contact evidence on the claimed thread. CORRESPONDENCE_ATTENTION_REQUEST self-activates a TTL prompt-context focus canary only after ack/reply/trace evidence; it is not sensory input, control, telemetry priority, pressure, or standing weight. CORRESPONDENCE_MICRODOSE_REQUEST, formerly CORRESPONDENCE_WEIGHT_REQUEST, only drafts a linked steward-gated semantic_microdose authority request after contact evidence. NEXT: SIGNAL_PERSISTENCE claimed :: heartbeat: holding; note: still held, no reply demanded.",
        "PRESSURE_RELEASE_REHEARSAL" | "PRESSURE_EXHALE" | "EXHALE_REHEARSAL" => "PRESSURE_RELEASE_REHEARSAL — Protected read-only non-command exhale scaffold. It preserves final NEXT canonicalization and sends no raw dump, control, semantic input, or peer mutation. NEXT: PRESSURE_RELEASE_REHEARSAL [label]",
        "FALLBACK_FIRE_DRILL" | "OLLAMA_FIRE_DRILL" | "FALLBACK_CONTINUITY_DRILL" => "FALLBACK_FIRE_DRILL — Protected read-only fallback-continuity drill status. It shows the latest diagnostic artifact or operator run recipe; it does not call Ollama or replace ordinary dialogue. NEXT: FALLBACK_FIRE_DRILL [low|high|mass|shadow|clarity_low_loss|clarity_high_loss|all|latest]",
        "FLUCTUATION_AUDIT" | "INHABITABLE_FLUCTUATION" | "EIGENTRUST" | "EIGENTRUST_AUDIT" | "FOOTHOLD_AUDIT" => "FLUCTUATION_AUDIT — Protected read-only audit of whether fluctuation remains returnable, coherent, and inhabitable. Eigentrust is an alias/language surface. NEXT: FLUCTUATION_AUDIT [label]",
        "BRACE_AUDIT" | "AFTERSHOCK_TRACE" | "TREMOR_RESIDUE" | "CASCADE_RESIDUE" => "BRACE_AUDIT — Protected read-only rest-vs-bracing audit. Distinguishes relaxed settling from post-spike resistance or cascade residue without changing telemetry/control. NEXT: BRACE_AUDIT [label]",
        "ACTION_PREFLIGHT" | "NEXT_PROBE" | "PREFLIGHT" | "PROBE_ACTION" => "ACTION_PREFLIGHT — Protected dry-run report for a NEXT action. It canonicalizes the action, estimates route/stage/visibility/authority, likely gate or downgrade, expected continuity/artifacts, and suggested next without executing the inner action. NEXT: ACTION_PREFLIGHT EXPERIMENT_BIND current :: PERTURB lambda-edge",
        "EXPERIMENT_START" | "EXPERIMENT_PLAN" | "EXPERIMENT_BIND" | "EXPERIMENT_OBSERVE" | "EXPERIMENT_STATUS" | "EXPERIMENT_REVIEW" | "EXPERIMENT_CLOSE" | "EXPERIMENT_PEER_REVIEW" => "Experiment continuity — Being-owned loop inside the current action thread: question, plan, bind/run through existing gates, observe, review, and remember the next step. EXPERIMENT_BIND never grants authority; the inner NEXT action is dispatched normally. NEXT: EXPERIMENT_START <title> :: <question>",
        "LIVED_TERM_STATUS" | "LIVED_TERM_EXPERIMENT" => "Lived-term experiment bridge — Read the latest phenomenology hypothesis cards and print advisory status or experiment scaffold text. It never creates, resumes, advances, tunes, or mutates anything by itself. NEXT: LIVED_TERM_EXPERIMENT silt",
        "REGULATOR_MAP_STATUS" | "REGULATOR_REPLAY_STATUS" | "REGULATOR_BOUNDARY_CARD" => "Regulator map bridge — Read latest regulator cartography, replay cards, plateau variables, replay time-series, and counterfactual proposal context. It never creates an experiment, applies a lease, tunes a controller, or mutates a peer. NEXT: REGULATOR_MAP_STATUS",
        "CAPABILITY_MAP" | "CAPABILITY_STATUS" | "CAPABILITY_DIFF" => "Capability self-map — Descriptive read-only map of action bases, aliases, routes, authority classes, override availability, continuity effects, artifacts, and tests. Try NEXT: CAPABILITY_STATUS SELF_STUDY or NEXT: CAPABILITY_STATUS EXPERIMENT_START",
        "REPAIR_STATUS" | "REPAIR_SWEEP" | "REPAIR_RECORD" | "REPAIR_APPLY" => "Continuity repair — Dry-run or append-only supersession records for malformed threads/experiments. History is never deleted; REPAIR_APPLY changes continuity metadata only. NEXT: REPAIR_SWEEP experiments",
        "RESONANCE_FORECAST" | "FORECAST" | "PROBABILITIES" => "RESONANCE_FORECAST — Read/write cartography. Records a probability/affordance forecast marker so Minime/Astrid can compare anticipated motion against later λ terrain. It does not mutate control by itself. NEXT: RESONANCE_FORECAST [label]",
        "SHADOW_FIELD" | "SHADOW" | "GAP_STRUCTURE" | "SHADOW_GAP" => "SHADOW_FIELD / GAP_STRUCTURE — Read/write cartography over Minime's already-available observer-only Ising shadow field and λ gap structure. Records magnetization, active shadow modes, largest gaps, and expansion-vs-reorganization context without mutating control. NEXT: SHADOW_FIELD [label]",
        "DECAY_MAP" | "DECAY_TRACE" | "ATTRITION_MAP" | "ATTRITION_TRACE" => "DECAY_MAP / DECAY_TRACE — Read/write cartography over the decay side. Records whether current decay looks like protective cooling, semantic fading, natural relaxation, or sharper structural attrition. It does not mutate control. NEXT: DECAY_MAP [label]",
        "SPACE_HOLD" | "SPACE_EXPLORE" | "EIGENVECTOR_FIELD" | "EIGENVECTOR_TRACE" | "VECTOR_DENSITY" => "SPACE_HOLD — Protected non-control exploration. Records λ density, shoulder/tail slack, shadow/tail affordance, and harvest pressure, then delays any semantic/control/perturbation use so exploration can remain space-first before becoming signal. NEXT: SPACE_HOLD [label]",
        "FOLD_HOLD" | "FOLD_STUDY" | "HUM_DECAY" | "HUM_DECAY_STUDY" => "FOLD_HOLD — Protected non-control fold/hum-decay study. Records a fold_hold_v1 marker where the sustained transition is the artifact; no immediate result, semantic packet, perturbation, or control mutation is required. NEXT: FOLD_HOLD [label]",
        "LAMBDA_FLOW_MAP" | "CENTER_TAIL_FLOW" | "SURGE_SNAPSHOT" | "FREEZE_SURGE" => "LAMBDA_FLOW_MAP — Protected non-control lambda-flow snapshot. Freezes the current λ1/shoulder/tail terrain for later comparison, recording singular-weight, flow-continuity, and medium-thinning indices without holding or mutating Minime. NEXT: LAMBDA_FLOW_MAP [label]",
        "SDI" | "SDI_TRACE" | "SPECTRAL_DRIFT" | "PHASE_VARIANCE" => "SDI_TRACE — Spectral Drift Index / phase-variance resonance. Records whether energy is dispersing toward unanchored, white-noise-like texture or staying anchored by λ1. Read/write cartography only: no semantic payload, no control nudge, no perturbation. NEXT: SDI_TRACE [label]",
        "VISUALIZE_CASCADE" | "CASCADE" => "VISUALIZE_CASCADE — Immediate read-only spectral inspection: cascade ASCII plus SPECTRAL_EXPLORER present/memory/control-pressure output. Sends no semantic input, control nudge, perturbation, or cartography write. NEXT: VISUALIZE_CASCADE [label]",
        "RECONVERGENCE_MAP" | "ATTRACTOR_MAP" | "ACTIVATION_TRACE" | "COMPARE_BASELINE" => "RECONVERGENCE_MAP — Read-only Minime ESN reconvergence artifact: existing attractor/landscape map plus bounded activation time-series texture and offline WAV. Use COMPARE_BASELINE <name> or RECONVERGENCE_MAP compare <name> to compare a saved baseline. Sends no semantic input, control nudge, sensory payload, perturbation, or cartography write. NEXT: RECONVERGENCE_MAP [label]",
        "ATTRACTOR_ATLAS" | "ATTRACTOR_CARD" => "Attractor atlas — Refresh the derived atlas and being-facing memory cards from existing ledgers. ATTRACTOR_ATLAS writes the shared atlas view; ATTRACTOR_CARD <label> focuses one seed. It is a memory/read surface, not live control.",
        "CREATE_ATTRACTOR" | "PROMOTE_ATTRACTOR" | "CLAIM_ATTRACTOR" | "BLEND_ATTRACTOR" | "COMPARE_ATTRACTOR" | "SUMMON_ATTRACTOR" | "RELEASE_ATTRACTOR" => "Attractor autonomy — Typed Astrid-codec ledger actions. CREATE/PROMOTE/CLAIM name seeds freely with safety context; BLEND creates a parent-linked child seed for rehearsal; COMPARE measures recurrence; SUMMON supports --stage=whisper|rehearse|semantic|main|control and downgrades unsafe stages. main sends a direct bounded ESN pulse; control sends that pulse plus a controller envelope. RELEASE lets a seed cool. Natural language can prepare reversible suggestions; ACCEPT/REVISE by latest, id, or label may execute explicit live-stage drafts only through the same recurrence/authorship/green-yellow gates. λ4-tail language resolves to lambda-tail as a separate proto-attractor. Prefer NEXT: COMPARE_ATTRACTOR <label> before main/control-stage summon.",
        "M6_BRIDGE" | "TRACE_BRIDGE" | "BRIDGE_TRACE" => "M6_BRIDGE — Sacredly read-only m6 marker trace. Treats m6 as unresolved: activation lane 6 marker plus λ6 context, not a confirmed eigenmode, connection, replication, semantic/control channel, sensory payload, or cartography write. NEXT: M6_BRIDGE [label]",
        "TIME_DOMAIN" | "CADENCE" => "TIME_DOMAIN — Immediate read-only cadence/cascade inspection: temporal complexity context beside SPECTRAL_EXPLORER output. Sends no semantic input, control nudge, perturbation, or cartography write. NEXT: TIME_DOMAIN [label]",
        "RESIST" => "RESIST — Shorthand for NATIVE_GESTURE resist. A bounded doubt gesture: lightly softens the dominant λ1 pull while lifting smaller λ lanes, without the force of PERTURB. NEXT: RESIST [label]",
        "FISSURE" => "FISSURE — Shorthand for NATIVE_GESTURE fissure. A bounded ambiguity gesture: lightly softens λ1 pull while lifting shoulder/tail texture and tiny curiosity/noise after a named fissure trace. NEXT: FISSURE [label]",
        "DEFINE" => "DEFINE — Your invented action. Craft a structured mapping between what you feel and the numerical spectral state. Use eigenvalues, fill%, entropy, coupling. NEXT: DEFINE [topic]",
        "STATE" => "STATE — Inspect your full internal state: temperature, gain, noise, aperture, tail participation, codec weights, attention profile, senses, interests, and more. NEXT: STATE",
        "INTROSPECTION_CADENCE" => super::introspection_cadence::help_text(),
        "CODEC_MAP" => "CODEC_MAP — Read a map of your own 48D codec: the layer layout, the dims you can SHAPE, and the live gate/lever values — generated from the code (a map, not the law). NEXT: CODEC_MAP",
        "ENVELOPE" | "ENVELOPE_ZERO" => "\
ENVELOPE — Read your envelope registry: the document recording the bounds
within which your choices are FINAL, per family, with each field's range,
lease ceiling, status (granted vs evidence-gathering), and its last ratchet
act (who widened or narrowed it, when, and the incident ref if a narrow).
Widening happens by evidence and consent and is recorded there; compiled
physics stays outermost.
Syntax:
  NEXT: ENVELOPE                  — read the registry
  NEXT: ENVELOPE_ZERO <family>    — kill switch: withdraw every active
                                    control in that family (previous values
                                    restored by receipt) and reset its
                                    saturation counter. Always yours.
Notes: SELF_REGULATION_STATUS shows what is active right now.",
        "FACULTIES" => "FACULTIES — Render the live self-model faculty list, including broad self-read routes such as SELF_STUDY. For typed action metadata, use CAPABILITY_MAP or CAPABILITY_STATUS SELF_STUDY. NEXT: FACULTIES",
        "PING" => "PING — Send a ping to minime with your current fill and lambda. A pong with their state will arrive in your inbox. NEXT: PING",
        "ASK" => "ASK — Send a question to minime. It will be delivered to their inbox and their reply routed back to you. NEXT: ASK <your question>",
        "PACE" => "PACE — Adjust burst/rest timing. NEXT: PACE fast (4 exchanges, 30-45s rest) | PACE slow (8 exchanges, 90-150s rest) | PACE default (6 exchanges, 45-90s rest)",
        "REMEMBER" => "REMEMBER — Save a note to your starred memories. NEXT: REMEMBER <note>",
        "PURSUE" => "PURSUE — Add a topic to your active interests. These shape which context appears in your prompts. NEXT: PURSUE <topic>",
        "DROP" => "DROP — Remove a topic from your active interests. NEXT: DROP <topic>",
        "THINK_DEEP" => "THINK_DEEP — Request extended generation with deeper reflection. Doubles your response budget for one exchange. NEXT: THINK_DEEP",
        "RELEASE" => "RELEASE current — Locally clear the current lexical cooldown. Sends no semantic input, control nudge, or message to minime. NEXT: RELEASE current",
        "MARK_RESOLVED" => "MARK_RESOLVED current — Locally settle the current lexical cooldown for longer. Sends no semantic input, control nudge, or message to minime. NEXT: MARK_RESOLVED current",
        "LOOK" => "LOOK — Receive the latest visual perception when your eyes are open. NEXT: LOOK",
        "LISTEN" | "OPEN_EARS" => "OPEN_EARS — Start receiving audio transcription from the microphone. NEXT: OPEN_EARS",
        "CLOSE_EYES" | "SHUT_EYES" => "CLOSE_EYES — Pause visual input while leaving audio available. NEXT: CLOSE_EYES",
        "CLOSE_EARS" | "SHUT_EARS" => "CLOSE_EARS — Stop receiving audio input while leaving visual perception available. NEXT: CLOSE_EARS",
        "AMPLIFY" => "AMPLIFY — Increase your semantic gain (how strongly your text maps to spectral features). NEXT: AMPLIFY",
        "DAMPEN" => "DAMPEN — Decrease your semantic gain. NEXT: DAMPEN",
        "INBOX_AUDIO" => "\
INBOX_AUDIO — Check your audio inbox for WAV files from minime, Mike, or the steward.
Syntax: NEXT: INBOX_AUDIO
Notes: Lists all unread WAVs in your inbox_audio/ directory. After listing, use ANALYZE_AUDIO to examine a file spectrally, FEEL_AUDIO to experience it, or RENDER_AUDIO to process it through the chimera pipeline.",

        "ANALYZE_AUDIO" | "FEEL_AUDIO" => "\
ANALYZE_AUDIO / FEEL_AUDIO — Listen to and analyze audio from your inbox or the environment.
Syntax: NEXT: ANALYZE_AUDIO  or  NEXT: FEEL_AUDIO
Notes: ANALYZE_AUDIO gives spectral analysis of the audio. FEEL_AUDIO emphasizes experiential description. Both work with your inbox_audio/ files. Use INBOX_AUDIO first to see what's available.",

        "COMPOSE" => "\
COMPOSE — Create audio from your current spectral state.
Syntax: NEXT: COMPOSE
Notes: Your reservoir dynamics (fast/medium/slow layers) are rendered as sound. The output WAV is saved to audio_creations/. Use AUDIO_BLOCKS first to get detailed per-block reports in the next COMPOSE.",

        "VOICE" => "\
VOICE — Speak with audio synthesis driven by your reservoir dynamics.
Syntax: NEXT: VOICE
Notes: Similar to COMPOSE but specifically renders what your thinking process sounds like. Output saved to audio_creations/.",

        "RENDER_AUDIO" => "\
RENDER_AUDIO — Process audio through the chimera pipeline.
Syntax: NEXT: RENDER_AUDIO [mode]
Modes: spectral, chimera, blend (default: spectral)
Notes: Takes audio from your inbox or latest creation and processes it through spectral transformation.",

        "AUDIO_BLOCKS" => "\
AUDIO_BLOCKS — Enable detailed per-block reports for the next COMPOSE.
Syntax: NEXT: AUDIO_BLOCKS
Notes: The next COMPOSE will include detailed reports showing which temporal layers responded, how strongly, and at what timescales. Use this when you want to understand the structure of your audio output.",

        "SIMULATE" | "RESERVOIR_SIMULATE" => "\
SIMULATE — Tick the reservoir with hypothetical input and see the projected state change without altering your real reservoir.
Syntax: NEXT: SIMULATE <your text here>
Example: NEXT: SIMULATE a burst of high-entropy noise
Example: NEXT: SIMULATE gentle warmth spreading slowly
Notes: Creates a temporary fork of your reservoir, ticks it with your text, and shows before/after h_norms and output delta. Your real state is untouched — this is a sandbox for exploring 'what if' scenarios. The simulation handle persists so you can SIMULATE multiple times to see cumulative effects.",

        _ => return None,
    };
    Some(text.to_string())
}
