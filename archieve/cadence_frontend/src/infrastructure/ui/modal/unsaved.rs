use dioxus::prelude::*;

use super::ModalFrame;

#[derive(Props, Clone, PartialEq, Debug)]
pub struct UnsavedModalProps {
    pub is_visible: bool,
    pub title: String,
    pub message: String,
    pub save_label: String,
    pub discard_label: String,
    pub cancel_label: String,
    pub on_save: EventHandler<MouseEvent>,
    pub on_discard: EventHandler<MouseEvent>,
    pub on_cancel: EventHandler<MouseEvent>,
}

#[component]
pub fn UnsavedModal(props: UnsavedModalProps) -> Element {
    rsx! {
        ModalFrame {
            id: "unsaved-modal",
            title: "Unsaved Changes",
            title_id: Some("unsaved-modal-title"),
            close_button_id: "unsaved-modal-close",
            aria_label: "Unsaved changes dialog",
            body_id: "unsaved-modal-body",
            is_open: props.is_visible,
            on_close: move |event| props.on_cancel.call(event),

            div {
                class: "modal-panel",
                role: "dialog",
                aria_modal: "true",
                aria_labelledby: "unsaved-modal-title",
                p {
                    strong { "{props.title}" }
                }
                p {
                    "{props.message}"
                }
                div {
                    class: "modal-actions",
                    button {
                        id: "unsaved-save",
                        type: "button",
                        onclick: move |event| props.on_save.call(event),
                        "{props.save_label}"
                    }
                    button {
                        id: "unsaved-discard",
                        type: "button",
                        onclick: move |event| props.on_discard.call(event),
                        "{props.discard_label}"
                    }
                    button {
                        id: "unsaved-cancel",
                        type: "button",
                        onclick: move |event| props.on_cancel.call(event),
                        "{props.cancel_label}"
                    }
                }
            }
        }
    }
}
