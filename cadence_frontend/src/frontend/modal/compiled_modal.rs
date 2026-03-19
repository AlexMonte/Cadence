use dioxus::prelude::*;

use super::{ModalFrame, ModalHeaderAction};

#[derive(Clone, Debug, PartialEq)]
pub struct CompileDiagnosticRow {
    pub severity: String,
    pub location: String,
    pub summary: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompileModalViewData {
    pub is_open: bool,
    pub banner_text: Option<String>,
    pub compiled_text: String,
    pub diagnostic_count: usize,
    pub diagnostics: Vec<CompileDiagnosticRow>,
    pub delay_slots: Vec<String>,
    pub domain_bridges: Vec<String>,
    pub activity_events: Vec<String>,
    pub editor_active: bool,
}

#[derive(Props, Clone, PartialEq)]
pub struct CompileModalProps {
    pub view_data: CompileModalViewData,
    pub on_close: EventHandler<MouseEvent>,
    pub on_refresh: EventHandler<MouseEvent>,
}

#[component]
pub fn CompileModal(props: CompileModalProps) -> Element {
    let header_action = ModalHeaderAction {
        id: "text-refresh",
        label: "Refresh",
        container_class: "text-view-actions",
        button_class: "",
    };

    let banner_class = if props.view_data.banner_text.is_some() {
        "compile-banner"
    } else {
        "compile-banner is-hidden"
    };

    let editor_class = if props.view_data.editor_active {
        "code-editor-host editor-host-active"
    } else {
        "code-editor-host"
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
    let delay_slots = props
        .view_data
        .delay_slots
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
    let activity_events = props
        .view_data
        .activity_events
        .iter()
        .map(|item| {
            rsx! {
                li {
                    key: "activity-{item}",
                    class: "compile-meta-item",
                    "{item}"
                }
            }
        })
        .collect::<Vec<_>>();
    let has_compile_meta = !props.view_data.delay_slots.is_empty()
        || !props.view_data.domain_bridges.is_empty()
        || !props.view_data.activity_events.is_empty();

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
                id: "text-script-editor",
                class: editor_class,
            }

            textarea {
                id: "text-script-input",
                class: "text-script-input",
                spellcheck: "false",
                wrap: "off",
                placeholder: "Compiled Strudel appears here...",
                value: props.view_data.compiled_text,
                readonly: true,
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
                        id: "compile-delay-slots-group",
                        class: if props.view_data.delay_slots.is_empty() { "compile-meta-group is-hidden" } else { "compile-meta-group" },

                        p {
                            id: "compile-delay-slots-summary",
                            class: "compile-meta-summary",
                            "Delay slots ({props.view_data.delay_slots.len()})"
                        }

                        ul {
                            id: "compile-delay-slots-list",
                            class: "compile-meta-list",
                            {delay_slots.into_iter()}
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
                        id: "compile-activity-events-group",
                        class: if props.view_data.activity_events.is_empty() { "compile-meta-group is-hidden" } else { "compile-meta-group" },

                        p {
                            id: "compile-activity-events-summary",
                            class: "compile-meta-summary",
                            "Activity events ({props.view_data.activity_events.len()})"
                        }

                        ul {
                            id: "compile-activity-events-list",
                            class: "compile-meta-list",
                            {activity_events.into_iter()}
                        }
                    }
                }
            }
        }
    }
}
