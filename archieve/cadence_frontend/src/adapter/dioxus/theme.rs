pub(crate) const APP_FRAME: &str = "app-shell min-h-dvh overflow-hidden bg-shell text-app";
pub(crate) const BACKEND_BANNER: &str = "backend-banner mx-3 mt-3 rounded-xl border border-panel bg-panel px-3 py-2 text-sm text-soft shadow-panel";
pub(crate) const STATUSBAR: &str =
    "statusbar border-t border-panel bg-panel px-3 py-2 text-xs text-muted";
pub(crate) const TOPBAR: &str =
    "topbar border-b border-panel bg-panel-strong px-3 py-3 text-app shadow-panel";
pub(crate) const TOPBAR_GROUP: &str = "topbar-group min-w-0 gap-2";
pub(crate) const TOPBAR_SECTION_LABEL: &str =
    "topbar-section-label text-xs uppercase tracking-section text-muted";
pub(crate) const TOPBAR_FIELD: &str = "topbar-field text-xs text-soft";
pub(crate) const TOPBAR_PROJECT_FIELD: &str = "topbar-field topbar-field-project text-xs text-soft";
pub(crate) const STATUS_TOAST: &str =
    "status-toast rounded-2xl border border-panel bg-panel px-3 py-3 shadow-panel";

const TOOLBAR_BUTTON_BASE: &str =
    "rounded-xl border border-control bg-control px-3 py-2 text-soft shadow-panel";
const ACTIVITY_DRAWER_BASE: &str =
    "activity-drawer border-t border-panel bg-panel-strong shadow-panel";
const ACTIVITY_TAB_BASE: &str =
    "activity-tab rounded-xl border border-control bg-control px-3 py-2 text-soft";

pub(crate) fn status_source(state_class: &str) -> String {
    format!(
        "status-source rounded-xl border border-panel bg-panel px-2 py-2 text-xs text-soft {}",
        state_class
    )
}

pub(crate) fn topbar_section(section_class: &str) -> String {
    format!(
        "topbar-section {} rounded-2xl border border-panel bg-panel px-3 py-2 shadow-panel",
        section_class
    )
}

pub(crate) fn toolbar_button(is_active: bool) -> &'static str {
    if is_active {
        "rounded-xl border border-control bg-control px-3 py-2 text-soft shadow-panel is-active"
    } else {
        TOOLBAR_BUTTON_BASE
    }
}

pub(crate) fn mode_chip(is_visible: bool) -> &'static str {
    if is_visible {
        "mode-chip rounded-xl border border-panel bg-panel px-2 py-2 text-soft"
    } else {
        "mode-chip rounded-xl border border-panel bg-panel px-2 py-2 text-soft is-hidden"
    }
}

pub(crate) fn activity_drawer(is_open: bool) -> &'static str {
    if is_open {
        "activity-drawer border-t border-panel bg-panel-strong shadow-panel is-open"
    } else {
        ACTIVITY_DRAWER_BASE
    }
}

pub(crate) fn activity_tab(is_active: bool) -> &'static str {
    if is_active {
        "activity-tab rounded-xl border border-control bg-control px-3 py-2 text-soft is-active"
    } else {
        ACTIVITY_TAB_BASE
    }
}
