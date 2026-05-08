use dioxus::prelude::*;

pub mod compiled_modal;
pub mod tile_inspector;
pub mod unsaved;

#[derive(Clone, Debug, PartialEq)]
pub struct ModalHeaderAction {
    pub id: &'static str,
    pub label: &'static str,
    pub container_class: &'static str,
    pub button_class: &'static str,
}

#[derive(Props, Clone, PartialEq)]
pub struct ModalFrameProps {
    pub id: &'static str,
    pub title: &'static str,
    pub title_id: Option<&'static str>,
    pub aria_label: &'static str,
    pub close_button_id: &'static str,
    pub body_id: &'static str,
    pub is_open: bool,
    pub on_close: EventHandler<MouseEvent>,
    #[props(default)]
    pub header_action: Option<ModalHeaderAction>,
    #[props(default)]
    pub on_header_action: Option<EventHandler<MouseEvent>>,
    pub children: Element,
}

#[component]
pub fn ModalFrame(props: ModalFrameProps) -> Element {
    let modal_class = if props.is_open {
        "pixel-modal"
    } else {
        "pixel-modal is-hidden"
    };

    let header_action = props.header_action.clone();
    let on_header_action = props.on_header_action;

    rsx! {
        section {
            id: props.id,
            class: modal_class,
            "aria-label": props.aria_label,

            header {
                class: "pixel-modal-head",

                if let Some(title_id) = props.title_id {
                    span { id: title_id, "{props.title}" }
                } else {
                    span { "{props.title}" }
                }

                if let Some(action) = header_action {
                    div {
                        class: action.container_class,
                        button {
                            id: action.id,
                            class: action.button_class,
                            type: "button",
                            onclick: move |event| {
                                if let Some(handler) = &on_header_action {
                                    handler.call(event);
                                }
                            },
                            "{action.label}"
                        }
                    }
                }

                button {
                    id: props.close_button_id,
                    type: "button",
                    onclick: move |event| props.on_close.call(event),
                    "X"
                }
            }

            div {
                id: props.body_id,
                class: "pixel-modal-body",
                {props.children}
            }
        }
    }
}
