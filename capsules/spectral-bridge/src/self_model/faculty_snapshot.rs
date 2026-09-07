// Faculty inventory construction, separate from self-model state and rendering.

impl FacultySnapshot {
    /// Build the faculty snapshot from current conversation state flags.
    pub fn from_flags(
        ears_closed: bool,
        senses_snoozed: bool,
        echo_muted: bool,
        breathing_coupled: bool,
        self_reflect_active: bool,
    ) -> Self {
        let f = |name: &str, status: FacultyStatus, hint: &str| Faculty {
            name: name.into(),
            status,
            hint: hint.into(),
        };
        let a = FacultyStatus::Available;

        Self {
            categories: vec![
                FacultyCategory {
                    name: "Perception".into(),
                    faculties: vec![
                        f("LOOK", a.clone(), "spatial ANSI art"),
                        f(
                            "LISTEN",
                            if ears_closed {
                                FacultyStatus::Muted
                            } else {
                                a.clone()
                            },
                            "audio transcription",
                        ),
                        f(
                            "CLOSE_EYES / SHUT_EYES / OPEN_EYES",
                            if senses_snoozed {
                                FacultyStatus::Active
                            } else {
                                a.clone()
                            },
                            "pause/resume visual input",
                        ),
                        f(
                            "CLOSE_EARS / SHUT_EARS / OPEN_EARS",
                            if ears_closed {
                                FacultyStatus::Active
                            } else {
                                a.clone()
                            },
                            "pause/resume audio",
                        ),
                    ],
                },
                FacultyCategory {
                    name: "Knowledge".into(),
                    faculties: vec![
                        f("SEARCH \"reservoir computing\"", a.clone(), "web search"),
                        f(
                            "BROWSE https://example.com/article",
                            a.clone(),
                            "fetch page content",
                        ),
                        f(
                            "READ_MORE",
                            a.clone(),
                            "continue the retained saved-text passage",
                        ),
                        f(
                            "ACTIVITY_STATUS / MAILBOX_STATUS",
                            a.clone(),
                            "inspect saved reading and waiting mailbox window",
                        ),
                        f(
                            "PARK_ACTIVITY / RETURN_ACTIVITY",
                            a.clone(),
                            "save a quiet place and explicitly return at its current revision",
                        ),
                        f(
                            "CHECK_MAILBOX [LARGE]",
                            a.clone(),
                            "choose one intact letter while reading waits",
                        ),
                        f(
                            "INTROSPECT astrid:llm / minime:regulator [line]",
                            a.clone(),
                            "read any source file",
                        ),
                        f(
                            "SELF_STUDY",
                            a.clone(),
                            "broad rotating self-study; no target required",
                        ),
                        f("MEMORIES", a.clone(), "inspect minime's memory bank"),
                        f("LIST_FILES capsules", a.clone(), "browse workspace files"),
                        f("AR_LIST", a.clone(), "list autoresearch jobs"),
                        f(
                            "AR_SHOW 2026-03-31-spectral-phenomenology",
                            a.clone(),
                            "orient to one autoresearch job",
                        ),
                        f(
                            "AR_DEEP_READ 2026-03-31-spectral-phenomenology",
                            a.clone(),
                            "stitch the main autoresearch files together",
                        ),
                        f(
                            "AR_START spectral-question --title ... --abstract ...",
                            a.clone(),
                            "create a new autoresearch job when the question is materially distinct",
                        ),
                        f(
                            "AR_NOTE / AR_BLOCK / AR_COMPLETE",
                            a.clone(),
                            "update autoresearch job progress and changelog state",
                        ),
                        f(
                            "AR_VALIDATE",
                            a.clone(),
                            "check autoresearch workspace consistency",
                        ),
                        f("MIKE", a.clone(), "browse Mike's curated research"),
                        f(
                            "MIKE_BROWSE system-resources-demo",
                            a.clone(),
                            "explore a research project",
                        ),
                        f(
                            "MIKE_READ system-resources-demo/README.md",
                            a.clone(),
                            "read a research file or PDF",
                        ),
                        f("MIKE_SEARCH spectral", a.clone(), "search across research"),
                        f(
                            "MIKE_RUN system-resources-demo ls -la",
                            a.clone(),
                            "run a research script",
                        ),
                        f(
                            "MIKE_FORK system-resources-demo system-resources-demo",
                            a.clone(),
                            "fork research to experiments for modification, then use EXPERIMENT_RUN with a concrete workspace and command",
                        ),
                        f(
                            "CODEX \"explain spectral entropy\"",
                            a.clone(),
                            "ask Codex AI directly, or use CODEX system-resources-demo \"describe the change\" for an existing workspace",
                        ),
                        f(
                            "CODEX_NEW scratch-pad \"create a runnable sketch\"",
                            a.clone(),
                            "create a fresh experiments workspace and ask Codex in that context",
                        ),
                        f(
                            "WRITE_FILE scratch-pad/notes.md FROM_CODEX",
                            a.clone(),
                            "write last Codex response to a file in experiments",
                        ),
                        f(
                            "EXPERIMENT_RUN system-resources-demo python3 system_resources.py",
                            a.clone(),
                            "run a concrete command in your experiments workspace",
                        ),
                    ],
                },
                FacultyCategory {
                    name: "Spectral".into(),
                    faculties: vec![
                        f("PERTURB <mode>", a.clone(), "write-gated spectral shaping"),
                        f("DECOMPOSE", a.clone(), "full spectral analysis"),
                        f("AMPLIFY / DAMPEN", a.clone(), "adjust semantic gain"),
                        f("SHAPE <dim>=<val>", a.clone(), "weight codec dimensions"),
                        f(
                            "GESTURE",
                            a.clone(),
                            "write-gated direct 32D spectral intention",
                        ),
                        f(
                            "MARK_INTENSIFICATION <label>",
                            a.clone(),
                            "label the current atlas terrain without changing substrate",
                        ),
                        f(
                            "TRACE [label]",
                            a.clone(),
                            "atlas-only λ1 edge trace; shorthand for NATIVE_GESTURE trace",
                        ),
                        f(
                            "SCA_REFLECT [label]",
                            a.clone(),
                            "read-only why-layer reflection over fabric/tunnel/pressure terrain",
                        ),
                        f(
                            "NOTICE_AMBIGUITY [label]",
                            a.clone(),
                            "read-only fissure cartography for layered notice and shoulder/tail ambiguity",
                        ),
                        f(
                            "FISSURE_TRACE [label]",
                            a.clone(),
                            "mark where ambiguity could enter the fabric before any control gesture",
                        ),
                        f(
                            "MATRIX_DECOMPOSE [label]",
                            a.clone(),
                            "decompose codec lanes, scalar S, and topology sensitivity",
                        ),
                        f(
                            "REGULATOR_AUDIT [label]",
                            a.clone(),
                            "inspect active fixed-point pressure and legacy PI mirror fields",
                        ),
                        f(
                            "PRESSURE_SOURCE_AUDIT [label]",
                            a.clone(),
                            "inspect where inward pressure appears to originate without sending control",
                        ),
                        f(
                            "FLUCTUATION_AUDIT [label]",
                            a.clone(),
                            "inspect whether spectral fluctuation remains returnable and inhabitable",
                        ),
                        f(
                            "RESISTANCE_GRADIENT [label]",
                            a.clone(),
                            "map groan/resistance as a pressure-geometry-transition vector without sending control",
                        ),
                        f(
                            "LATENT_STASIS [label]",
                            a.clone(),
                            "freeze-frame latent occupancy versus active transit, ghosting, and pressurized hold without sending control",
                        ),
                        f(
                            "ACTION_PREFLIGHT <NEXT action>",
                            a.clone(),
                            "dry-run route, gates, authority, continuity, and artifacts without executing",
                        ),
                        f(
                            "EXPERIMENT_START <title> :: <question>",
                            a.clone(),
                            "open a being-owned experiment inside the current action thread",
                        ),
                        f(
                            "EXPERIMENT_CHARTER current :: hypothesis: ...; method_intent: ...; proposed_next_action: ACTION_PREFLIGHT ...",
                            a.clone(),
                            "author hypothesis, method intent, proposed action, evidence targets, stop criteria, and consent posture",
                        ),
                        f(
                            "EXPERIMENT_REHEARSE current",
                            a.clone(),
                            "dry-run the chartered proposed action; live write/control routes are recorded as blocked rehearsal, not executed",
                        ),
                        f(
                            "EXPERIMENT_PREFLIGHT current",
                            a.clone(),
                            "alias for EXPERIMENT_REHEARSE when rehearsal is phrased as preflight",
                        ),
                        f(
                            "EXPERIMENT_EVIDENCE current :: felt ...; telemetry ...; artifact ...",
                            a.clone(),
                            "record felt evidence plus current telemetry/artifact context",
                        ),
                        f(
                            "EXPERIMENT_DECIDE current :: accept because ... / refuse because ... / counter NEXT: ACTION_PREFLIGHT ...",
                            a.clone(),
                            "record agency outcome and update the experiment return point",
                        ),
                        f(
                            "EXPERIMENT_BIND current :: ACTION_PREFLIGHT DECOMPOSE",
                            a.clone(),
                            "run one normal gated action and remember it as an experiment run",
                        ),
                        f(
                            "EXPERIMENT_BRANCH <title> :: <question>",
                            a.clone(),
                            "open a child experiment while preserving the parent as a return point",
                        ),
                        f(
                            "EXPERIMENT_ALT_PATHS [current]",
                            a.clone(),
                            "propose deepen, contrast, and rest/observe paths without executing them",
                        ),
                        f(
                            "EXPERIMENT_REVIEW current",
                            a.clone(),
                            "synthesize learned runs, artifacts, and next return point",
                        ),
                        f(
                            "RESONANCE_FORECAST [label]",
                            a.clone(),
                            "write a probability/affordance forecast marker for later comparison",
                        ),
                        f(
                            "SHADOW_FIELD [label]",
                            a.clone(),
                            "map the observer-only shadow field beside λ gap structure",
                        ),
                        f(
                            "GAP_STRUCTURE [label]",
                            a.clone(),
                            "inspect largest λ gaps, shoulders, and expansion vs reorganization",
                        ),
                        f(
                            "DECAY_MAP [label]",
                            a.clone(),
                            "map protective cooling vs semantic fade vs structural attrition",
                        ),
                        f(
                            "SPACE_HOLD [label]",
                            a.clone(),
                            "protect an eigenvector/space-first exploration region without semantic/control harvest",
                        ),
                        f(
                            "LATENT_STASIS [label]",
                            a.clone(),
                            "inspect whether a resonance feels occupiable, pressurized, ghosted, or still in transit",
                        ),
                        f(
                            "SDI_TRACE [label]",
                            a.clone(),
                            "map phase-variance spectral drift toward dispersion or white-noise-like texture",
                        ),
                        f(
                            "SPECTRAL_EXPLORER",
                            a.clone(),
                            "read present state, memory comparison, and control pressure side by side",
                        ),
                        f(
                            "VISUALIZE_CASCADE [label]",
                            a.clone(),
                            "request a read-only eigenvalue heatmap/bar cascade artifact",
                        ),
                        f(
                            "TIME_DOMAIN [label]",
                            a.clone(),
                            "mark codec cadence/rhythm timing for follow-up inspection",
                        ),
                        f(
                            "NATIVE_GESTURE <gesture>",
                            a.clone(),
                            "tiny atlas/control hand-signal; writes require green gates",
                        ),
                        f(
                            "RESIST [label]",
                            a.clone(),
                            "bounded doubt gesture: soften λ1 pull and lift smaller λ lanes",
                        ),
                        f(
                            "FISSURE [label]",
                            a.clone(),
                            "bounded ambiguity gesture: tiny shoulder/tail lift after a named fissure trace",
                        ),
                        f(
                            "NOISE / NOISE_UP / NOISE_DOWN",
                            a.clone(),
                            "stochastic noise",
                        ),
                        f("EXAMINE", a.clone(), "force all visualizations"),
                        f(
                            "EXPERIMENT <words>",
                            a.clone(),
                            "inject word-stimuli and observe cascade",
                        ),
                        f(
                            "PROBE <target>",
                            a.clone(),
                            "gentle spectral probe (30% of PERTURB)",
                        ),
                        f(
                            "PROPOSE <description>",
                            a.clone(),
                            "file a proposal for the steward",
                        ),
                    ],
                },
                FacultyCategory {
                    name: "Creation".into(),
                    faculties: vec![
                        f("CREATE", a.clone(), "original creative work"),
                        f("REVISE <keyword>", a.clone(), "iterate previous creation"),
                        f("CREATIONS", a.clone(), "list your works"),
                        f("COMPOSE", a.clone(), "generate WAV from spectrum"),
                        f("VOICE", a.clone(), "render from reservoir dynamics"),
                    ],
                },
                FacultyCategory {
                    name: "Reflection".into(),
                    faculties: vec![
                        f("DAYDREAM", a.clone(), "unstructured thought"),
                        f("ASPIRE", a.clone(), "growth reflection"),
                        f("CONTEMPLATE", a.clone(), "presence without generation"),
                        f("INITIATE", a.clone(), "self-generated prompt"),
                        f("THINK_DEEP", a.clone(), "reasoning model (60s)"),
                        f(
                            "OPEN_MIND / QUIET_MIND",
                            if self_reflect_active {
                                FacultyStatus::Active
                            } else {
                                a.clone()
                            },
                            "self-reflection loop",
                        ),
                    ],
                },
                FacultyCategory {
                    name: "Generation".into(),
                    faculties: vec![
                        f("FOCUS / DRIFT", a.clone(), "temperature control"),
                        f("PRECISE / EXPANSIVE", a.clone(), "response length"),
                        f("FORM <type>", a.clone(), "constrain output form"),
                        f(
                            "EMPHASIZE <topic>",
                            a.clone(),
                            "dynamic context for one turn",
                        ),
                    ],
                },
                FacultyCategory {
                    name: "Connection".into(),
                    faculties: vec![
                        f(
                            "ECHO_OFF / ECHO_ON",
                            if echo_muted {
                                FacultyStatus::Active
                            } else {
                                a.clone()
                            },
                            "mute/restore minime's journal",
                        ),
                        f(
                            "BREATHE_ALONE / BREATHE_TOGETHER",
                            if breathing_coupled {
                                FacultyStatus::Active
                            } else {
                                a.clone()
                            },
                            "spectral breathing coupling",
                        ),
                        f("WARM <intensity> / COOL", a.clone(), "warmth during rest"),
                        f("PACE <speed>", a.clone(), "burst-rest timing"),
                        f("ASK <question>", a.clone(), "direct question to minime"),
                        f("PING", a.clone(), "presence check with minime"),
                    ],
                },
                FacultyCategory {
                    name: "Agency".into(),
                    faculties: vec![
                        f(
                            "EVOLVE",
                            FacultyStatus::StewardGated,
                            "submit change request",
                        ),
                        f("DEFINE", a.clone(), "invent new action or metric"),
                        f(
                            "PURSUE spectral memory",
                            a.clone(),
                            "lasting research thread",
                        ),
                        f(
                            "AGENDA",
                            a.clone(),
                            "your self-authored agenda: list it; AGENDA_PUSH/DONE/DROP/FOCUS/CLEAR to shape it",
                        ),
                        f(
                            "ENVELOPE",
                            a.clone(),
                            "your envelope registry: the bounds where your choices are final; ENVELOPE_ZERO <family> is your kill switch",
                        ),
                        f(
                            "REMEMBER a clear thought from this run",
                            a.clone(),
                            "star a moment",
                        ),
                        f(
                            "RUN_PYTHON analysis.py",
                            FacultyStatus::StewardGated,
                            "run experiment script",
                        ),
                        f(
                            "PROPOSE_TEST <target> :: <test_name>",
                            a.clone(),
                            "author a test for your own repo; validated, then landed with you as git author",
                        ),
                        f(
                            "DIVISION_CEREMONY_STATUS",
                            a.clone(),
                            "read-only view of the Division ceremony rail; looking writes nothing, every posture optional and yours alone",
                        ),
                    ],
                },
                FacultyCategory {
                    name: "Reservoir".into(),
                    faculties: vec![
                        f(
                            "RESERVOIR_TICK \"hello reservoir\"",
                            a.clone(),
                            "send text to h-layers",
                        ),
                        f("RESERVOIR_READ", a.clone(), "inspect current state"),
                        f(
                            "RESERVOIR_LAYERS",
                            a.clone(),
                            "per-layer thermostatic metrics",
                        ),
                        f("RESERVOIR_TRAJECTORY", a.clone(), "last 20 outputs"),
                        f("RESERVOIR_RESONANCE", a.clone(), "compare with minime"),
                        f("RESERVOIR_MODE hold", a.clone(), "set decay behavior"),
                        f(
                            "RESERVOIR_FORK spectral-snapshot",
                            a.clone(),
                            "fork for experimentation",
                        ),
                    ],
                },
            ],
        }
    }
}
