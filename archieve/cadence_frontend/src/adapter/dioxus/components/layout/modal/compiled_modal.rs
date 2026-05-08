use dioxus::prelude::*;

use super::{ModalFrame, ModalHeaderAction};

use crate::application::editor::{CompileModalViewData, CompileTimelineLaneView};

#[derive(Props, Clone, PartialEq)]
pub struct CompileTimelineProps {
    pub lanes: Vec<CompileTimelineLaneView>,
    pub playhead_pct: f64,
    pub playhead_label: String,
    #[props(default)]
    pub compact: bool,
}

#[component]
pub fn CompileTimeline(props: CompileTimelineProps) -> Element {
    let scroll_class = if props.compact {
        "timeline-scroll is-compact"
    } else {
        "timeline-scroll"
    };
    let timeline_lanes = props
        .lanes
        .iter()
        .map(|lane| {
            let rows = lane
                .rows
                .iter()
                .map(|row| {
                    let events = row
                        .events
                        .iter()
                        .map(|event| {
                            let class = if event.active {
                                "timeline-event is-active"
                            } else {
                                "timeline-event"
                            };
                            let style = format!(
                                "left: {:.3}%; width: {:.3}%;",
                                event.left_pct, event.width_pct
                            );
                            rsx! {
                                button {
                                    key: "{event.key}",
                                    r#type: "button",
                                    class: class,
                                    title: "{event.title}",
                                    style: "{style}",
                                    span {
                                        class: "timeline-event-label",
                                        "{event.label}"
                                    }
                                }
                            }
                        })
                        .collect::<Vec<_>>();
                    let row_class = if row.is_pitch_row {
                        "timeline-row timeline-row-pitched"
                    } else {
                        "timeline-row"
                    };
                    let playhead_style = format!("left: {:.3}%;", props.playhead_pct * 100.0);
                    rsx! {
                        div {
                            key: "{row.key}",
                            class: row_class,

                            div {
                                class: "timeline-row-label",
                                "{row.label}"
                            }

                            div {
                                class: "timeline-row-track",

                                div {
                                    class: "timeline-row-grid",
                                    span { class: "timeline-grid-mark", "0" }
                                    span { class: "timeline-grid-mark", "1/4" }
                                    span { class: "timeline-grid-mark", "1/2" }
                                    span { class: "timeline-grid-mark", "3/4" }
                                    span { class: "timeline-grid-mark", "1" }
                                }

                                div {
                                    class: "timeline-playhead",
                                    title: "{props.playhead_label}",
                                    style: "{playhead_style}",
                                }

                                {events.into_iter()}
                            }
                        }
                    }
                })
                .collect::<Vec<_>>();
            rsx! {
                section {
                    key: "{lane.key}",
                    class: "timeline-lane",

                    header {
                        class: "timeline-lane-head",
                        strong { "{lane.label}" }
                        span { class: "timeline-lane-subtitle", "{lane.subtitle}" }
                    }

                    div {
                        class: "timeline-lane-body",
                        {rows.into_iter()}
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        div {
            class: scroll_class,
            {timeline_lanes.into_iter()}
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct CompileModalProps {
    pub view_data: CompileModalViewData,
    pub on_close: EventHandler<MouseEvent>,
    pub on_refresh: EventHandler<MouseEvent>,
}

#[component]
pub fn CompileModal(props: CompileModalProps) -> Element {
    let mut show_debug = use_signal(|| false);
    let header_action = ModalHeaderAction {
        id: "timeline-refresh",
        label: "Refresh",
        container_class: "text-view-actions",
        button_class: "",
    };

    let banner_class = if props.view_data.banner_text.is_some() {
        "compile-banner"
    } else {
        "compile-banner is-hidden"
    };

    let timeline_class = if props.view_data.editor_active {
        "code-editor-host editor-host-active timeline-editor-host"
    } else {
        "code-editor-host timeline-editor-host"
    };
    let diagnostics = props
        .view_data
        .diagnostics
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let severity_class = format!("text-status-pill status-{}", row.severity.to_lowercase());
            rsx! {
                tr {
                    key: "diag-{index}",
                    td {
                        span {
                            class: severity_class,
                            "{row.severity}"
                        }
                    }
                    td { "{row.location}" }
                    td { "{row.summary}" }
                }
            }
        })
        .collect::<Vec<_>>();
    let delay_edges = props
        .view_data
        .delay_edges
        .iter()
        .map(|item| {
            rsx! {
                li {
                    key: "delay-{item}",
                    class: "compile-meta-item",
                    "{item}"
                }
            }
        })
        .collect::<Vec<_>>();
    let domain_bridges = props
        .view_data
        .domain_bridges
        .iter()
        .map(|item| {
            rsx! {
                li {
                    key: "bridge-{item}",
                    class: "compile-meta-item",
                    "{item}"
                }
            }
        })
        .collect::<Vec<_>>();
    let has_compile_meta = !props.view_data.delay_edges.is_empty()
        || !props.view_data.domain_bridges.is_empty()
        || props.view_data.preview_event_count > 0;
    let has_timeline = !props.view_data.timeline_lanes.is_empty();
    let tab_timeline_class = if !*show_debug.read() {
        "compile-tab is-active"
    } else {
        "compile-tab"
    };
    let tab_debug_class = if *show_debug.read() {
        "compile-tab is-active"
    } else {
        "compile-tab"
    };

    rsx! {
        ModalFrame {
            id: "compile-modal",
            title: "Compiled",
            title_id: None,
            aria_label: "Compiled code",
            close_button_id: "modal-compile-close",
            body_id: "modal-compile-body",
            is_open: props.view_data.is_open,
            on_close: props.on_close,
            header_action,
            on_header_action: Some(props.on_refresh),

            p {
                id: "compile-banner",
                class: banner_class,
                {props.view_data.banner_text.unwrap_or_default()}
            }

            div {
                class: "compile-tabs",

                button {
                    r#type: "button",
                    class: tab_timeline_class,
                    onclick: move |_| show_debug.set(false),
                    "Timeline"
                }

                button {
                    r#type: "button",
                    class: tab_debug_class,
                    onclick: move |_| show_debug.set(true),
                    "Debug Text"
                }
            }

            if !*show_debug.read() {
                section {
                    id: "timeline-script-editor",
                    class: timeline_class,
                    "aria-label": "Compiled timeline preview",

                    div {
                        class: "timeline-summary",

                        div {
                            class: "timeline-summary-copy",
                            strong { "One-cycle preview" }
                            span {
                                class: "timeline-summary-sub",
                                "{props.view_data.preview_event_count} events | {props.view_data.playhead_label}"
                            }
                        }
                    }

                    if has_timeline {
                        CompileTimeline {
                            lanes: props.view_data.timeline_lanes.clone(),
                            playhead_pct: props.view_data.playhead_pct,
                            playhead_label: props.view_data.playhead_label.clone(),
                        }
                    } else {
                        div {
                            class: "timeline-empty",
                            "Compile a playable graph to see clip lanes and note rows here."
                        }
                    }
                }
            } else {
                textarea {
                    id: "text-script-input",
                    class: "text-script-input",
                    spellcheck: "false",
                    wrap: "off",
                    placeholder: "Cadence debug text appears here...",
                    value: props.view_data.debug_text,
                    readonly: true,
                }
            }

            details {
                id: "text-diagnostics-section",
                class: "dock-panel text-diagnostics",
                "aria-label": "Compile diagnostics",

                summary {
                    class: "dock-panel-summary",
                    strong { "Diagnostics" }
                    span { id: "text-import-summary", "{props.view_data.diagnostic_count}" }
                }

                div {
                    class: "dock-panel-body text-diagnostics-table-wrap",
                    table {
                        class: "text-diagnostics-table",
                        thead {
                            tr {
                                th { "Sev" }
                                th { "Where" }
                                th { "Summary" }
                            }
                        }
                        tbody {
                            id: "text-import-table-body",
                            {diagnostics.into_iter()}
                        }
                    }
                }
            }

            section {
                id: "compile-meta-section",
                class: if has_compile_meta { "dock-panel compile-meta" } else { "dock-panel compile-meta is-hidden" },
                "aria-label": "Compile metadata",

                div {
                    class: "dock-panel-body compile-meta-body",

                    div {
                        id: "compile-delay-edges-group",
                        class: if props.view_data.delay_edges.is_empty() { "compile-meta-group is-hidden" } else { "compile-meta-group" },

                        p {
                            id: "compile-delay-edges-summary",
                            class: "compile-meta-summary",
                            "Delay edges ({props.view_data.delay_edges.len()})"
                        }

                        ul {
                            id: "compile-delay-edges-list",
                            class: "compile-meta-list",
                            {delay_edges.into_iter()}
                        }
                    }

                    div {
                        id: "compile-domain-bridges-group",
                        class: if props.view_data.domain_bridges.is_empty() { "compile-meta-group is-hidden" } else { "compile-meta-group" },

                        p {
                            id: "compile-domain-bridges-summary",
                            class: "compile-meta-summary",
                            "Domain bridges ({props.view_data.domain_bridges.len()})"
                        }

                        ul {
                            id: "compile-domain-bridges-list",
                            class: "compile-meta-list",
                            {domain_bridges.into_iter()}
                        }
                    }

                    div {
                        id: "compile-preview-events-group",
                        class: if props.view_data.preview_event_count == 0 { "compile-meta-group is-hidden" } else { "compile-meta-group" },

                        p {
                            id: "compile-preview-events-summary",
                            class: "compile-meta-summary",
                            "Preview events ({props.view_data.preview_event_count})"
                        }

                        p {
                            class: "compile-meta-note",
                            "The playhead and tile highlights reuse these one-cycle preview events."
                        }
                    }
                }
            }
        }
    }
}
