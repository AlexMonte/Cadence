use dioxus::prelude::*;

use crate::application::editor::*;
use crate::adapter::dioxus::components::layout::modal::compiled_modal::CompileTimeline;

use crate::adapter::dioxus::theme as shell_theme;

#[component]
pub(crate) fn ActivityDrawerRegion(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let compile_view = compile_modal_view(&snapshot);
    let runtime_boot_label = format_runtime_boot_label(&snapshot.runtime_boot);
    let runtime_playback_label = format_runtime_playback_label(&snapshot);
    let console_output = render_console_output(&snapshot);
    let activity_open = snapshot.compile_open;
    let playback_issue_count = compile_view
        .diagnostics
        .iter()
        .filter(|row| row.severity == "error")
        .count();
    let live_feedback_count = snapshot.diagnostics.entries.len();
    let activity_drawer_class = shell_theme::activity_drawer(activity_open);
    let issues_tab_class = shell_theme::activity_tab(snapshot.activity_tab == ActivityTab::Issues);
    let preview_tab_class =
        shell_theme::activity_tab(snapshot.activity_tab == ActivityTab::Preview);
    let diagnostics_tab_class =
        shell_theme::activity_tab(snapshot.activity_tab == ActivityTab::Diagnostics);

    rsx! {
        section {
            id: "activity-drawer",
            class: activity_drawer_class,
            "aria-label": "Activity drawer",
            div {
                class: "activity-head",
                div {
                    class: "panel-head-copy",
                    strong { "Activity" }
                    span {
                        class: "panel-head-subtitle",
                        "{playback_issue_count} playback issue(s) and {live_feedback_count} live event(s)"
                    }
                }
                div {
                    class: "activity-tabs",
                    button {
                        r#type: "button",
                        class: issues_tab_class,
                        onclick: move |_| {
                            let mut current = state.write();
                            current.compile_open = true;
                            current.activity_tab = ActivityTab::Issues;
                        },
                        "Issues"
                    }
                    button {
                        r#type: "button",
                        class: preview_tab_class,
                        onclick: move |_| {
                            let mut current = state.write();
                            current.compile_open = true;
                            current.activity_tab = ActivityTab::Preview;
                        },
                        "Preview"
                    }
                    button {
                        r#type: "button",
                        class: diagnostics_tab_class,
                        onclick: move |_| {
                            let mut current = state.write();
                            current.compile_open = true;
                            current.activity_tab = ActivityTab::Diagnostics;
                        },
                        "Diagnostics"
                    }
                }
                button {
                    id: "mini-console-toggle",
                    r#type: "button",
                    onclick: move |_| {
                        state.write().compile_open = false;
                    },
                    "Hide"
                }
            }
            div {
                class: "activity-body",
                if snapshot.activity_tab == ActivityTab::Issues {
                    div {
                        class: "activity-scroll",
                        if let Some(message) = snapshot.status_message.clone() {
                            article {
                                class: "activity-card activity-card-primary",
                                strong { "Latest update" }
                                p { "{message}" }
                            }
                        }
                        if compile_view.diagnostics.is_empty() && snapshot.status_message.is_none() {
                            p {
                                class: "activity-empty",
                                "No issues right now. Keep shaping the song and feedback will appear here when it matters."
                            }
                        } else {
                            if !compile_view.diagnostics.is_empty() {
                                section {
                                    class: "activity-card",
                                    strong { "Playback checks" }
                                    div { class: "activity-list",
                                        for row in &compile_view.diagnostics {
                                            div {
                                                key: "{row.location}-{row.summary}",
                                                class: "activity-list-item",
                                                span { class: activity_severity_pill_class(&row.severity), "{row.severity}" }
                                                div {
                                                    strong { "{row.summary}" }
                                                    p { "{row.location}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if snapshot.activity_tab == ActivityTab::Preview {
                    div {
                        class: "activity-scroll",
                        if let Some(banner_text) = compile_view.banner_text.clone() {
                            p {
                                id: "compile-banner",
                                class: "compile-banner",
                                "{banner_text}"
                            }
                        }
                        div {
                            class: "timeline-summary",
                            div {
                                class: "timeline-summary-copy",
                                strong { "Song preview" }
                                span {
                                    class: "timeline-summary-sub",
                                    "{compile_view.preview_event_count} event(s) • {compile_view.playhead_label}"
                                }
                            }
                            button {
                                r#type: "button",
                                class: "panel-toggle-button",
                                onclick: move |_| {
                                    let next = !state.read().show_activity_diagnostics;
                                    state.write().show_activity_diagnostics = next;
                                },
                                if snapshot.show_activity_diagnostics {
                                    "Hide preview details"
                                } else {
                                    "Show preview details"
                                }
                            }
                        }
                        if compile_view.timeline_lanes.is_empty() {
                            p {
                                class: "activity-empty",
                                "Preview the song to see timing lanes, event shapes, and playback progress here."
                            }
                        } else {
                            CompileTimeline {
                                lanes: compile_view.timeline_lanes.clone(),
                                playhead_pct: compile_view.playhead_pct,
                                playhead_label: compile_view.playhead_label.clone(),
                            }
                        }
                        if snapshot.show_activity_diagnostics {
                            section {
                                class: "activity-card",
                                strong { "Preview details" }
                                p { class: "activity-meta", "Delay edges: {compile_view.delay_edges.len()} • Domain bridges: {compile_view.domain_bridges.len()}" }
                                if !compile_view.delay_edges.is_empty() {
                                    ul { class: "compile-meta-list",
                                        for edge in &compile_view.delay_edges {
                                            li { key: "delay-{edge}", "{edge}" }
                                        }
                                    }
                                }
                                if !compile_view.domain_bridges.is_empty() {
                                    ul { class: "compile-meta-list",
                                        for bridge in &compile_view.domain_bridges {
                                            li { key: "bridge-{bridge}", "{bridge}" }
                                        }
                                    }
                                }
                                if !compile_view.debug_text.trim().is_empty() {
                                    pre {
                                        class: "mini-console-output",
                                        "{compile_view.debug_text}"
                                    }
                                }
                            }
                        }
                    }
                } else {
                    div {
                        class: "activity-scroll",
                        div {
                            class: "activity-card activity-card-primary",
                            strong { "Live diagnostics" }
                            p { "{console_output}" }
                        }
                        button {
                            class: "panel-toggle-button",
                            r#type: "button",
                            onclick: move |_| {
                                let next = !state.read().show_activity_diagnostics;
                                state.write().show_activity_diagnostics = next;
                            },
                            if snapshot.show_activity_diagnostics {
                                "Hide engine details"
                            } else {
                                "Show engine details"
                            }
                        }
                        if snapshot.show_activity_diagnostics {
                            div {
                                class: "activity-card",
                                strong { "Runtime details" }
                                p { class: "activity-meta", "{runtime_boot_label}" }
                                p { class: "activity-meta", "{runtime_playback_label}" }
                                p { class: "activity-meta", "Samples ready: {snapshot.sample_readiness.loaded}/{snapshot.sample_readiness.attempted}" }
                                p { class: "activity-meta", "Cache hits: {snapshot.sample_cache.hits} • misses: {snapshot.sample_cache.misses} • writes: {snapshot.sample_cache.writes}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
