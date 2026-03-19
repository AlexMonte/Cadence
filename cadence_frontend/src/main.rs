use cadence_frontend::frontend::AppShell;
use dioxus::prelude::*;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    rsx! {
        AppShell {}

    }
}
