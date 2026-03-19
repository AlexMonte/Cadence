use dioxus::prelude::*;

use super::native_editor::NativeEditorApp;

const FAVICON: Asset = asset!("/assets/favicon.ico");
const MAIN_CSS: Asset = asset!("/assets/app.css");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

pub fn app_route() -> &'static str {
    "/editor"
}

#[allow(non_snake_case)]
pub fn AppShell() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Link { rel: "stylesheet", href: MAIN_CSS }
        document::Link { rel: "stylesheet", href: TAILWIND_CSS }
        NativeEditorApp {}
    }
}
