use dioxus::prelude::*;

use super::ModalFrame;

#[derive(Clone, Debug, PartialEq)]
pub struct InspectorModalViewData {
    pub is_open: bool,
    pub project_title: String,
    pub project_meta: String,
    pub selected_node_label: String,
    pub selected_role_label: String,
    pub mode_label: String,
    pub tile_preview_label: String,
    pub tile_preview_sub: String,
    pub detail_lines: Vec<String>,
    pub param_lines: Vec<String>,
    pub editor_active: bool,
}

impl Default for InspectorModalViewData {
    fn default() -> Self {
        Self {
            is_open: false,
            project_title: "Tile Inspector".into(),
            project_meta: "No project loaded.".into(),
            selected_node_label: "Node: none".into(),
            selected_role_label: "Role: unknown".into(),
            mode_label: "GRAPH".into(),
            tile_preview_label: "No Tile Selected".into(),
            tile_preview_sub: "Pick or place a tile on the grid.".into(),
            detail_lines: Vec::new(),
            param_lines: Vec::new(),
            editor_active: false,
        }
    }
}

#[derive(Props, Clone, PartialEq)]
pub struct InspectorModalProps {
    pub view_data: InspectorModalViewData,
    pub on_close: EventHandler<MouseEvent>,
    pub on_delete: EventHandler<MouseEvent>,
}

#[component]
pub fn InspectorModal(props: InspectorModalProps) -> Element {
    let editor_class = if props.view_data.editor_active {
        "code-editor-host editor-host-active"
    } else {
        "code-editor-host"
    };
    let detail_lines = props
        .view_data
        .detail_lines
        .iter()
        .map(|line| {
            rsx! {
                p {
                    key: "detail-{line}",
                    class: "tile-param-hint",
                    "{line}"
                }
            }
        })
        .collect::<Vec<_>>();
    let param_cards = props
        .view_data
        .param_lines
        .iter()
        .map(|line| {
            rsx! {
                div {
                    key: "param-{line}",
                    class: "tile-param-card",
                    p { class: "tile-param-title", "{line}" }
                }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        ModalFrame {
            id: "tile-inspector-modal",
            title: "Tile",
            title_id: Some("modal-inspector-title"),
            aria_label: "Tile inspector",
            close_button_id: "modal-inspector-close",
            body_id: "modal-inspector-body",
            is_open: props.view_data.is_open,
            on_close: props.on_close,

            section {
                class: "inspector-header",
                h1 { id: "project-title", "{props.view_data.project_title}" }
                p { id: "project-meta", class: "meta", "{props.view_data.project_meta}" }
                div {
                    class: "node-head",
                    span {
                        id: "selected-node-label",
                        class: "node-chip",
                        "{props.view_data.selected_node_label}"
                    }
                    span {
                        id: "selected-role-label",
                        class: "role-chip",
                        "{props.view_data.selected_role_label}"
                    }
                }
            }

            section {
                id: "node-code-window",
                class: "node-code-window",
                "aria-label": "Tile control panel",

                header {
                    id: "node-code-head",
                    class: "node-code-head",
                    div {
                        class: "node-code-head-left",
                        strong { id: "node-code-title", "Tile Controls" }
                        span {
                            id: "node-code-mode",
                            class: "node-code-mode",
                            "{props.view_data.mode_label}"
                        }
                    }
                }

                div {
                    class: "node-code-body",
                    div {
                        id: "tile-preview",
                        class: "tile-preview-box",
                        "aria-hidden": "true",
                        span {
                            class: "tile-preview-label",
                            "{props.view_data.tile_preview_label}"
                        }
                        span {
                            class: "tile-preview-sub",
                            "{props.view_data.tile_preview_sub}"
                        }
                    }
                    label {
                        class: "field",
                        r#for: "node-code-editor",
                        "Tile Info"
                    }
                    div {
                        id: "node-code-editor",
                        class: editor_class,

                        div {
                            class: "tile-data-scroll",

                            if props.view_data.detail_lines.is_empty() && props.view_data.param_lines.is_empty() {
                                p {
                                    class: "tile-empty-message",
                                    "Pick or place a tile on the grid."
                                }
                            } else {
                                if !props.view_data.detail_lines.is_empty() {
                                    div {
                                        class: "tile-info-card",
                                        p { class: "tile-section-title", "Tile Info" }
                                        {detail_lines.into_iter()}
                                    }
                                }

                                if !props.view_data.param_lines.is_empty() {
                                    div {
                                        class: "tile-controls-stack",
                                        p { class: "tile-section-title", "Parameters" }
                                        {param_cards.into_iter()}
                                    }
                                }
                            }
                        }
                    }
                }
            }

            div {
                class: "inspector-actions",
                button {
                    id: "delete-node",
                    r#type: "button",
                    onclick: move |event| props.on_delete.call(event),
                    "Delete"
                }
            }
        }
    }
}
