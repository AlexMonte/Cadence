use dioxus::prelude::*;

use crate::infrastructure::ui::AppShell;

#[allow(non_snake_case)]
pub fn App() -> Element {
    rsx! {
        AppShell {}
    }
}
