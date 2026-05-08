use dioxus::prelude::*;

use crate::adapter::dioxus::assets::{APP_CSS, COMPONENTS_CSS, FAVICON, MAIN_CSS, TAILWIND_CSS};

use super::EditorSurface;

#[allow(non_snake_case)]
pub fn AppShell() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: APP_CSS}
        document::Link { rel: "stylesheet", href: COMPONENTS_CSS}
        EditorSurface {}
    }
}
