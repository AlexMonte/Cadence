use dioxus::prelude::*;

use crate::adapter::backend::InitStageOp;
use crate::adapter::dioxus::editor_service::{
    apply_init_ops, create_trick, switch_workspace_mode, upsert_sample_load,
};
use crate::application::editor::{
    EditorShellState, SAMPLE_ALIAS_PLACEHOLDER, WorkspaceMode, filtered_sample_library_entries,
    sample_library_bank_hint, sample_library_display_name, sample_library_sound_hint,
    sample_library_tags_hint, sample_load_alias_hint, sample_load_sound_hint,
};

#[component]
pub(crate) fn ScoreSetup(state: Signal<EditorShellState>, snapshot: EditorShellState) -> Element {
    let filtered_sample_library = filtered_sample_library_entries(&snapshot);
    let sample_library_total = filtered_sample_library.len();
    let visible_sample_library = filtered_sample_library
        .into_iter()
        .take(48)
        .collect::<Vec<_>>();
    let hidden_sample_library_count =
        sample_library_total.saturating_sub(visible_sample_library.len());
    let sample_library_cards = visible_sample_library
        .iter()
        .map(|entry| {
            let sound_hint = sample_library_sound_hint(entry);
            let bank_hint = sample_library_bank_hint(entry);
            let tag_hint = sample_library_tags_hint(entry);
            rsx! {
                div {
                    key: "{entry.key}",
                    class: "init-card",
                    div {
                        class: "init-card-head",
                        strong { "{sample_library_display_name(entry)}" }
                        span {
                            class: "init-status is-ready",
                            "{entry.library.clone().unwrap_or_else(|| \"root\".to_string())}"
                        }
                    }
                    div {
                        class: "init-card-meta",
                        div {
                            strong { "Use in sound tile" }
                            code { class: "init-code", "{sound_hint}" }
                        }
                        if let Some(bank_hint) = bank_hint {
                            div {
                                strong { "Optional bank tile" }
                                code { class: "init-code", "{bank_hint}" }
                            }
                        }
                        if let Some(tag_hint) = tag_hint {
                            div {
                                strong { "Tags" }
                                span { "{tag_hint}" }
                            }
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let external_sample_cards = snapshot
        .init_stage
        .sample_loads
        .iter()
        .map(|sample| {
            let sample_id = sample.id.clone();
            let sound_hint = sample_load_sound_hint(sample);
            let alias_hint = sample_load_alias_hint(sample);
            rsx! {
                div {
                    key: "{sample.id}",
                    class: "init-card",
                    div {
                        class: "init-card-head",
                        strong { "{sample.id}" }
                        span { class: "init-status is-ready", "{sample.aliases.len()} aliases" }
                    }
                    div {
                        class: "init-card-meta",
                        div {
                            strong { "Source" }
                            span { "{sample.source}" }
                        }
                        div {
                            strong { "Use in sound tile" }
                            code { class: "init-code", "{sound_hint}" }
                        }
                        if let Some(alias_hint) = alias_hint {
                            div {
                                strong { "Also resolves" }
                                span { "{alias_hint}" }
                            }
                        }
                    }
                    div {
                        class: "init-card-actions",
                        button {
                            r#type: "button",
                            onclick: move |_| {
                                let sample_id = sample_id.clone();
                                spawn(async move {
                                    apply_init_ops(
                                        state,
                                        vec![InitStageOp::SampleLoadRemove { id: sample_id }],
                                    )
                                    .await;
                                });
                            },
                            "Remove Source"
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();
    let trick_cards = snapshot
        .init_stage
        .tricks
        .iter()
        .map(|trick| {
            let open_trick_id = trick.id.clone();
            let remove_trick_id = trick.id.clone();
            let open_trick_name = trick.name.clone();
            rsx! {
                div {
                    key: "{trick.id}",
                    class: "init-card",
                    div {
                        class: "init-card-head",
                        strong { "{trick.name}" }
                        span { class: "init-status is-ready", "N:{trick.node_count} E:{trick.edge_count}" }
                    }
                    div {
                        class: "init-card-actions",
                        button {
                            r#type: "button",
                            onclick: move |_| {
                                let trick_id = open_trick_id.clone();
                                let trick_name = open_trick_name.clone();
                                spawn(async move {
                                    switch_workspace_mode(
                                        state,
                                        WorkspaceMode::Trick {
                                            trick_id,
                                            name: trick_name,
                                        },
                                    )
                                    .await;
                                });
                            },
                            "Open Pattern"
                        }
                        button {
                            r#type: "button",
                            onclick: move |_| {
                                let trick_id = remove_trick_id.clone();
                                spawn(async move {
                                    apply_init_ops(state, vec![InitStageOp::TrickDelete { id: trick_id }]).await;
                                });
                            },
                            "Delete Pattern"
                        }
                    }
                }
            }
        })
        .collect::<Vec<_>>();

    rsx! {
        section {
            id: "init-workspace",
            class: "init-workspace",
            "aria-label": "Setup workspace",
            section {
                class: "init-section",
                header {
                    class: "init-section-head",
                    strong { "Setup" }
                    span { class: "role-chip", "Workspace" }
                }
                label { class: "field", r#for: "init-cps-input", "Tempo expression" }
                div {
                    class: "init-inline",
                    input {
                        id: "init-cps-input",
                        r#type: "text",
                        value: snapshot.init_cps_input.clone(),
                        placeholder: "113/60/4",
                        oninput: move |event| {
                            state.write().init_cps_input = event.value();
                        }
                    }
                    button {
                        id: "init-cps-save",
                        r#type: "button",
                        onclick: move |_| {
                            spawn(async move {
                                let expr = {
                                    let value = state.read().init_cps_input.trim().to_string();
                                    if value.is_empty() { None } else { Some(value) }
                                };
                                apply_init_ops(state, vec![InitStageOp::SetCps { expr }]).await;
                            });
                        },
                        "Refresh"
                    }
                }
            }
            div {
                class: "init-columns",
                section {
                    class: "init-section is-wide",
                    header {
                        class: "init-section-head",
                        strong { "Assets" }
                        span {
                            id: "init-sample-summary",
                            class: "node-chip",
                            "{snapshot.sample_library.entries.len()} library | {snapshot.init_stage.sample_loads.len()} loads"
                        }
                    }
                    p {
                        class: "init-helper",
                        "Browse the active sample library, see the exact selector each entry expects, and register any extra external loads in the same place. For tiles, the important field is usually the sound tile's `value`."
                    }
                    div {
                        class: "asset-columns",
                        div {
                            class: "asset-pane",
                            header {
                                class: "init-section-head",
                                strong { "Sample library" }
                                span { class: "role-chip", "{snapshot.sample_library.source_label}" }
                            }
                            if let Some(source_path) = snapshot.sample_library.source_path.clone() {
                                p {
                                    class: "init-helper asset-path",
                                    code { class: "init-code", "{source_path}" }
                                }
                            }
                            if let Some(load_error) = snapshot.sample_library.error.clone() {
                                p {
                                    class: "init-helper asset-warning",
                                    "{load_error}"
                                }
                            }
                            if snapshot.sample_library.available {
                                div {
                                    class: "init-inline",
                                    input {
                                        id: "init-sample-library-query",
                                        r#type: "text",
                                        value: snapshot.sample_library_query_input.clone(),
                                        placeholder: "Filter by name, folder, or tag",
                                        oninput: move |event| {
                                            state.write().sample_library_query_input = event.value();
                                        }
                                    }
                                }
                                if sample_library_total > visible_sample_library.len() {
                                    span {
                                        class: "init-helper",
                                        "Showing {visible_sample_library.len()} of {sample_library_total} entries"
                                    }
                                }
                                if sample_library_cards.is_empty() {
                                    p {
                                        class: "init-helper",
                                        "No samples match the current filter."
                                    }
                                } else {
                                    div {
                                        id: "init-sample-library",
                                        class: "init-list",
                                        {sample_library_cards.into_iter()}
                                    }
                                }
                                if hidden_sample_library_count > 0 {
                                    p {
                                        class: "init-helper",
                                        "{hidden_sample_library_count} more matches are hidden. Narrow the filter to inspect them."
                                    }
                                }
                            } else if snapshot.sample_library.error.is_none() {
                                p {
                                    class: "init-helper",
                                    "No sample library is available yet."
                                }
                            }
                        }
                        div {
                            class: "asset-pane",
                            header {
                                class: "init-section-head",
                                strong { "External loads" }
                                span { class: "role-chip", "Project manifest" }
                            }
                            p {
                                class: "init-helper",
                                "Use this when a sample should be fetched or aliased into the project. The saved id, plus any alias keys you define, can be typed directly into the sound tile's `value`."
                            }
                            div {
                                class: "init-form-grid",
                                label { class: "field", r#for: "init-sample-id", "Sample ID" }
                                input {
                                    id: "init-sample-id",
                                    r#type: "text",
                                    value: snapshot.sample_id_input.clone(),
                                    placeholder: "amen",
                                    oninput: move |event| state.write().sample_id_input = event.value()
                                }
                                label { class: "field", r#for: "init-sample-source", "External source" }
                                input {
                                    id: "init-sample-source",
                                    r#type: "text",
                                    value: snapshot.sample_source_input.clone(),
                                    placeholder: "https://cdn.example.com/drums/amen.wav",
                                    oninput: move |event| state.write().sample_source_input = event.value()
                                }
                                label { class: "field", r#for: "init-sample-aliases", "Aliases (JSON object)" }
                                textarea {
                                    id: "init-sample-aliases",
                                    class: "init-textarea",
                                    spellcheck: "false",
                                    value: snapshot.sample_aliases_input.clone(),
                                    placeholder: SAMPLE_ALIAS_PLACEHOLDER,
                                    oninput: move |event| state.write().sample_aliases_input = event.value()
                                }
                            }
                            div {
                                class: "init-actions",
                                button {
                                    id: "init-sample-save",
                                    r#type: "button",
                                    onclick: move |_| {
                                        spawn(async move {
                                            upsert_sample_load(state).await;
                                        });
                                    },
                                    "Save load"
                                }
                            }
                            if external_sample_cards.is_empty() {
                                p {
                                    class: "init-helper",
                                    "No external sources added yet."
                                }
                            } else {
                                div {
                                    class: "init-list",
                                    {external_sample_cards.into_iter()}
                                }
                            }
                        }
                    }
                }
                section {
                    class: "init-section",
                    header {
                        class: "init-section-head",
                        strong { "Patterns" }
                        span { id: "init-trick-summary", class: "node-chip", "{snapshot.init_stage.tricks.len()}" }
                    }
                    div {
                        class: "init-inline",
                        input {
                            id: "init-trick-name",
                            r#type: "text",
                            value: snapshot.trick_name_input.clone(),
                            placeholder: "melodia",
                            oninput: move |event| state.write().trick_name_input = event.value()
                        }
                        button {
                            id: "init-trick-create",
                            r#type: "button",
                            onclick: move |_| {
                                spawn(async move {
                                    create_trick(state).await;
                                });
                            },
                            "Create Pattern"
                        }
                    }
                    div {
                        id: "init-trick-list",
                        class: "init-list",
                        {trick_cards.into_iter()}
                    }
                }
            }
        }
    }
}
