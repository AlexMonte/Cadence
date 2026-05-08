use dioxus::prelude::*;

use crate::adapter::dioxus::EditorApp;

#[allow(non_snake_case)]
pub(crate) fn EditorSurface() -> Element {
    rsx! {
        EditorApp {}
    }
}
