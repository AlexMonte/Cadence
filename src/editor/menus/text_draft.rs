//! Text-draft menu for editing, validating, and committing runtime code snippets.

use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;

use crate::core::ScopeId;
use crate::editor::AppState;
use crate::editor::menus::{Menu, MenuAssets};
use crate::editor::ui::{
    FontAssets, UiStyles, buttons::button_base, label_with_style, styles::ButtonImages, ui_root,
};
use crate::runtime::protocol::{RuntimeEvent, RuntimeIntent};
use crate::runtime::{RuntimeHostConfig, generate_scope_code};

const MAX_PREVIEW_LINES: usize = 30;
const MAX_PREVIEW_LINE_CHARS: usize = 120;
const TEXT_DRAFT_VALIDATE_DEBOUNCE_SECS: f32 = 0.35;

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<TextDraftState>()
        .add_message::<TextDraftCommitted>()
        .add_systems(OnEnter(Menu::TextDraft), spawn_text_draft_menu)
        .add_systems(
            Update,
            (
                close_text_draft_on_escape,
                apply_text_draft_shortcuts,
                sync_draft_scope_context,
                capture_text_draft_input,
                debounce_runtime_validation_requests,
                apply_runtime_validation_events,
                sync_text_draft_menu,
            )
                .chain()
                .run_if(in_state(Menu::TextDraft)),
        );
}

#[derive(Resource, Debug, Clone)]
pub struct TextDraftState {
    pub source_scope: Option<ScopeId>,
    pub draft: String,
    pub committed: String,
    pub draft_rev: u64,
    pub committed_rev: u64,
    pub dirty: bool,
    pub last_validation_error: Option<String>,
    pub last_validation_ok: bool,
    pub validation_request_seq: u64,
    pub pending_validation_request: Option<u64>,
    pub pending_validation_draft_rev: Option<u64>,
    pub pending_validation_scope: Option<ScopeId>,
    pub pending_commit_code: Option<String>,
    pub validation_debounce_armed: bool,
    pub validation_timer: Timer,
}

impl Default for TextDraftState {
    fn default() -> Self {
        Self {
            source_scope: None,
            draft: String::new(),
            committed: String::new(),
            draft_rev: 0,
            committed_rev: 0,
            dirty: false,
            last_validation_error: None,
            last_validation_ok: false,
            validation_request_seq: 0,
            pending_validation_request: None,
            pending_validation_draft_rev: None,
            pending_validation_scope: None,
            pending_commit_code: None,
            validation_debounce_armed: false,
            validation_timer: new_validation_timer(),
        }
    }
}

impl TextDraftState {
    fn clear_pending_validation(&mut self) {
        self.pending_validation_request = None;
        self.pending_validation_draft_rev = None;
        self.pending_validation_scope = None;
        self.pending_commit_code = None;
    }

    fn reset_validation_debounce(&mut self, armed: bool) {
        self.validation_debounce_armed = armed;
        self.validation_timer = new_validation_timer();
    }
}

#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct TextDraftCommitted {
    pub scope: ScopeId,
    pub code: String,
    pub draft_rev: u64,
}

#[derive(Component)]
struct TextDraftStatusLabel;

#[derive(Component)]
struct TextDraftErrorLabel;

#[derive(Component)]
struct TextDraftBodyLabel;

#[derive(Component)]
struct PullScopeButton;

#[derive(Component)]
struct RevertDraftButton;

#[derive(Component)]
struct ValidateCommitButton;

#[derive(Component)]
struct CloseTextDraftButton;

fn spawn_text_draft_menu(
    mut commands: Commands,
    assets: Res<MenuAssets>,
    styles: Res<UiStyles>,
    fonts: Res<FontAssets>,
    app_state: Res<AppState>,
    mut draft_state: ResMut<TextDraftState>,
) {
    pull_scope_code_into_draft(&app_state, draft_state.as_mut(), true);

    let mut mono_text = styles.text.label.clone();
    mono_text.font = fonts.default_font_mono.clone();
    mono_text.font_size = 10.0;

    let mut action_button_style = styles
        .buttons
        .normal
        .clone()
        .with_size(Vec2::new(150.0, 28.0));
    action_button_style.text.font_size = 14.0;
    let action_images = ButtonImages::from(assets.as_ref());

    commands.spawn((
        ui_root("Text Draft Menu"),
        GlobalZIndex(60),
        DespawnOnExit(Menu::TextDraft),
        children![(
            Name::new("Text Draft Panel"),
            Node {
                width: px(1040.0),
                height: px(640.0),
                flex_direction: FlexDirection::Column,
                row_gap: px(8.0),
                padding: UiRect::all(px(10.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.08, 0.1, 0.14, 0.96)),
            children![
                (
                    label_with_style("Text Draft (Phase 4 Shell)", &styles.text.header),
                ),
                (
                    Name::new("Text Draft Actions"),
                    Node {
                        column_gap: px(6.0),
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    children![
                        button_base(
                            "Pull Scope",
                            pull_scope_code_on_click,
                            (action_button_style.node(), PullScopeButton),
                            Some(action_images.clone()),
                            action_button_style.clone()
                        ),
                        button_base(
                            "Revert Draft",
                            revert_draft_on_click,
                            (action_button_style.node(), RevertDraftButton),
                            Some(action_images.clone()),
                            action_button_style.clone()
                        ),
                        button_base(
                            "Validate + Commit",
                            validate_and_commit_on_click,
                            (action_button_style.node(), ValidateCommitButton),
                            Some(action_images.clone()),
                            action_button_style.clone()
                        ),
                        button_base(
                            "Close",
                            close_text_draft_on_click,
                            (action_button_style.node(), CloseTextDraftButton),
                            Some(action_images.clone()),
                            action_button_style
                        ),
                    ],
                ),
                (TextDraftStatusLabel, label_with_style("", &mono_text)),
                (TextDraftErrorLabel, label_with_style("", &mono_text)),
                (
                    Name::new("Text Draft Preview Frame"),
                    Node {
                        width: percent(100.0),
                        flex_grow: 1.0,
                        padding: UiRect::all(px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.03, 0.05, 0.08, 0.98)),
                    children![(TextDraftBodyLabel, label_with_style("", &mono_text),)],
                ),
                (
                    label_with_style(
                        "Type directly in this panel. Esc closes, Cmd/Ctrl+Enter validates and commits.",
                        &mono_text
                    ),
                ),
            ],
        )],
    ));
}

fn pull_scope_code_on_click(
    click: On<Pointer<Click>>,
    buttons: Query<(), With<PullScopeButton>>,
    app_state: Res<AppState>,
    mut draft_state: ResMut<TextDraftState>,
) {
    if !button_target_click_hit(&click, &buttons) {
        return;
    }
    pull_scope_code_into_draft(&app_state, draft_state.as_mut(), true);
}

fn revert_draft_on_click(
    click: On<Pointer<Click>>,
    buttons: Query<(), With<RevertDraftButton>>,
    mut draft_state: ResMut<TextDraftState>,
) {
    if !button_target_click_hit(&click, &buttons) {
        return;
    }
    if draft_state.draft != draft_state.committed {
        draft_state.draft = draft_state.committed.clone();
        draft_state.draft_rev += 1;
    }
    draft_state.dirty = false;
    draft_state.last_validation_error = None;
    draft_state.last_validation_ok = true;
    draft_state.clear_pending_validation();
    draft_state.reset_validation_debounce(false);
}

fn validate_and_commit_on_click(
    click: On<Pointer<Click>>,
    buttons: Query<(), With<ValidateCommitButton>>,
    app_state: Res<AppState>,
    runtime_host_config: Option<Res<RuntimeHostConfig>>,
    mut draft_state: ResMut<TextDraftState>,
    mut runtime_intents: MessageWriter<RuntimeIntent>,
    mut committed: MessageWriter<TextDraftCommitted>,
) {
    if !button_target_click_hit(&click, &buttons) {
        return;
    }
    request_or_fallback_validate_and_commit(
        &app_state,
        runtime_host_config.as_deref(),
        draft_state.as_mut(),
        &mut runtime_intents,
        &mut committed,
    );
}

fn close_text_draft_on_click(
    click: On<Pointer<Click>>,
    buttons: Query<(), With<CloseTextDraftButton>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    if !button_target_click_hit(&click, &buttons) {
        return;
    }
    next_menu.set(Menu::None);
}

fn close_text_draft_on_escape(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut next_menu: ResMut<NextState<Menu>>,
) {
    if keyboard.just_pressed(KeyCode::Escape) {
        next_menu.set(Menu::None);
    }
}

fn apply_text_draft_shortcuts(
    keyboard: Res<ButtonInput<KeyCode>>,
    app_state: Res<AppState>,
    runtime_host_config: Option<Res<RuntimeHostConfig>>,
    mut draft_state: ResMut<TextDraftState>,
    mut runtime_intents: MessageWriter<RuntimeIntent>,
    mut committed: MessageWriter<TextDraftCommitted>,
) {
    if !primary_modifier_pressed(&keyboard) {
        return;
    }

    if keyboard.just_pressed(KeyCode::KeyR) {
        pull_scope_code_into_draft(&app_state, draft_state.as_mut(), true);
    }
    if keyboard.just_pressed(KeyCode::Enter) || keyboard.just_pressed(KeyCode::NumpadEnter) {
        request_or_fallback_validate_and_commit(
            &app_state,
            runtime_host_config.as_deref(),
            draft_state.as_mut(),
            &mut runtime_intents,
            &mut committed,
        );
    }
}

fn capture_text_draft_input(
    mut key_events: MessageReader<KeyboardInput>,
    keyboard: Res<ButtonInput<KeyCode>>,
    mut draft_state: ResMut<TextDraftState>,
) {
    if primary_modifier_pressed(&keyboard) {
        return;
    }

    let mut changed = false;

    for event in key_events.read() {
        if event.state != ButtonState::Pressed {
            continue;
        }

        match event.key_code {
            KeyCode::Backspace => {
                if draft_state.draft.pop().is_some() {
                    changed = true;
                }
            }
            KeyCode::Enter | KeyCode::NumpadEnter => {
                draft_state.draft.push('\n');
                changed = true;
            }
            KeyCode::Tab => {
                draft_state.draft.push_str("  ");
                changed = true;
            }
            _ => {
                let Some(text) = event.text.as_deref() else {
                    continue;
                };
                for ch in text.chars() {
                    if ch.is_control() {
                        continue;
                    }
                    draft_state.draft.push(ch);
                    changed = true;
                }
            }
        }
    }

    if changed {
        draft_state.draft_rev += 1;
        draft_state.dirty = draft_state.draft != draft_state.committed;
        draft_state.last_validation_error = None;
        draft_state.last_validation_ok = false;
        draft_state.clear_pending_validation();
        draft_state.reset_validation_debounce(true);
    }
}

fn sync_draft_scope_context(app_state: Res<AppState>, mut draft_state: ResMut<TextDraftState>) {
    let current_scope = app_state.current_scope;
    match draft_state.source_scope {
        None => {
            pull_scope_code_into_draft(&app_state, draft_state.as_mut(), true);
        }
        Some(source_scope) if source_scope == current_scope => {}
        Some(_) if !draft_state.dirty => {
            pull_scope_code_into_draft(&app_state, draft_state.as_mut(), true);
        }
        Some(source_scope) => {
            draft_state.last_validation_ok = false;
            let pinned_message = format!(
                "draft pinned to scope {} while current scope is {} (Pull Scope to switch)",
                short_scope(source_scope),
                short_scope(current_scope)
            );
            draft_state.last_validation_error = Some(pinned_message);
        }
    }
}

fn debounce_runtime_validation_requests(
    time: Res<Time>,
    app_state: Res<AppState>,
    runtime_host_config: Option<Res<RuntimeHostConfig>>,
    mut draft_state: ResMut<TextDraftState>,
    mut runtime_intents: MessageWriter<RuntimeIntent>,
) {
    let host_validation_enabled = runtime_host_config
        .as_deref()
        .is_some_and(|config| config.enabled);
    if !host_validation_enabled {
        return;
    }
    if draft_state.pending_validation_request.is_some() || !draft_state.validation_debounce_armed {
        return;
    }

    draft_state.validation_timer.tick(time.delta());
    if !draft_state.validation_timer.just_finished() {
        return;
    }

    if draft_state.draft.trim().is_empty() {
        draft_state.last_validation_ok = false;
        draft_state.last_validation_error = Some("draft is empty".to_string());
        draft_state.validation_debounce_armed = false;
        return;
    }

    request_runtime_validation(&app_state, draft_state.as_mut(), &mut runtime_intents, None);
}

fn apply_runtime_validation_events(
    mut runtime_events: MessageReader<RuntimeEvent>,
    app_state: Res<AppState>,
    mut draft_state: ResMut<TextDraftState>,
    mut committed: MessageWriter<TextDraftCommitted>,
) {
    let Some(pending_request) = draft_state.pending_validation_request else {
        return;
    };

    for event in runtime_events.read() {
        match event {
            RuntimeEvent::Validation {
                request_id,
                ok,
                message,
                line,
                column,
                ..
            } if *request_id == pending_request => {
                if *ok {
                    draft_state.last_validation_ok = true;
                    draft_state.last_validation_error = None;
                    if let Some(code) = draft_state.pending_commit_code.clone() {
                        let commit_scope =
                            draft_state.source_scope.unwrap_or(app_state.current_scope);
                        draft_state.committed = code.clone();
                        draft_state.committed_rev = draft_state
                            .pending_validation_draft_rev
                            .unwrap_or(draft_state.draft_rev);
                        draft_state.dirty = draft_state.draft != draft_state.committed;
                        committed.write(TextDraftCommitted {
                            scope: commit_scope,
                            code,
                            draft_rev: draft_state.committed_rev,
                        });
                    }
                } else {
                    draft_state.last_validation_ok = false;
                    draft_state.last_validation_error =
                        Some(format_runtime_validation_error(message, *line, *column));
                }
                draft_state.clear_pending_validation();
            }
            RuntimeEvent::Error { message, .. } => {
                draft_state.last_validation_ok = false;
                draft_state.last_validation_error =
                    Some(format!("runtime validation failed: {message}"));
                draft_state.clear_pending_validation();
            }
            _ => {}
        }
    }
}

fn sync_text_draft_menu(
    draft_state: Res<TextDraftState>,
    app_state: Res<AppState>,
    mut labels: ParamSet<(
        Query<&mut Text, With<TextDraftStatusLabel>>,
        Query<&mut Text, With<TextDraftErrorLabel>>,
        Query<&mut Text, With<TextDraftBodyLabel>>,
    )>,
) {
    let source_scope = draft_state.source_scope.unwrap_or(app_state.current_scope);
    let source_short = short_scope(source_scope);
    let current_short = short_scope(app_state.current_scope);
    let scope_status = if source_scope == app_state.current_scope {
        format!("scope:{source_short}")
    } else {
        format!("scope:{source_short}->{}(pinned)", current_short)
    };

    if let Ok(mut status_label) = labels.p0().single_mut() {
        let validation = if draft_state.last_validation_ok {
            "ok"
        } else if draft_state.pending_validation_request.is_some() {
            "validating"
        } else if draft_state.validation_debounce_armed {
            "typing"
        } else {
            "pending"
        };
        status_label.0 = format!(
            "{scope_status} | draft_rev:{} | committed_rev:{} | dirty:{} | validation:{validation}",
            draft_state.draft_rev, draft_state.committed_rev, draft_state.dirty,
        );
    }

    if let Ok(mut error_label) = labels.p1().single_mut() {
        error_label.0 = draft_state
            .last_validation_error
            .clone()
            .unwrap_or_else(|| "validation: no error".to_string());
    }

    if let Ok(mut body_label) = labels.p2().single_mut() {
        body_label.0 = render_draft_preview(&draft_state.draft);
    }
}

fn validate_and_commit_draft(
    app_state: &AppState,
    draft_state: &mut TextDraftState,
    committed: &mut MessageWriter<TextDraftCommitted>,
) {
    match validate_text_draft(&draft_state.draft) {
        Ok(()) => {
            draft_state.committed = draft_state.draft.clone();
            draft_state.committed_rev = draft_state.draft_rev;
            draft_state.dirty = false;
            draft_state.last_validation_ok = true;
            draft_state.last_validation_error = None;
            let commit_scope = draft_state.source_scope.unwrap_or(app_state.current_scope);
            committed.write(TextDraftCommitted {
                scope: commit_scope,
                code: draft_state.committed.clone(),
                draft_rev: draft_state.committed_rev,
            });
        }
        Err(error) => {
            draft_state.last_validation_ok = false;
            draft_state.last_validation_error = Some(error);
        }
    }
}

fn request_or_fallback_validate_and_commit(
    app_state: &AppState,
    runtime_host_config: Option<&RuntimeHostConfig>,
    draft_state: &mut TextDraftState,
    runtime_intents: &mut MessageWriter<RuntimeIntent>,
    committed: &mut MessageWriter<TextDraftCommitted>,
) {
    let host_validation_enabled = runtime_host_config.is_some_and(|config| config.enabled);
    if host_validation_enabled {
        request_runtime_validation(
            app_state,
            draft_state,
            runtime_intents,
            Some(draft_state.draft.clone()),
        );
    } else {
        validate_and_commit_draft(app_state, draft_state, committed);
    }
}

fn request_runtime_validation(
    app_state: &AppState,
    draft_state: &mut TextDraftState,
    runtime_intents: &mut MessageWriter<RuntimeIntent>,
    pending_commit_code: Option<String>,
) {
    let request_id = draft_state.validation_request_seq.saturating_add(1);
    draft_state.validation_request_seq = request_id;
    draft_state.pending_validation_request = Some(request_id);
    draft_state.pending_validation_draft_rev = Some(draft_state.draft_rev);
    draft_state.pending_validation_scope = Some(app_state.current_scope);
    draft_state.pending_commit_code = pending_commit_code;
    draft_state.last_validation_ok = false;
    draft_state.last_validation_error = None;
    draft_state.reset_validation_debounce(false);
    runtime_intents.write(RuntimeIntent::validate(
        request_id,
        draft_state.draft.clone(),
    ));
}

fn format_runtime_validation_error(
    message: &Option<String>,
    line: Option<u32>,
    column: Option<u32>,
) -> String {
    let message = message
        .clone()
        .unwrap_or_else(|| "runtime parser error".to_string());
    match (line, column) {
        (Some(line), Some(column)) => format!(
            "validation failed at line {}, column {}: {}",
            line,
            column.saturating_add(1),
            message
        ),
        (Some(line), None) => format!("validation failed at line {}: {}", line, message),
        _ => format!("validation failed: {}", message),
    }
}

fn pull_scope_code_into_draft(app_state: &AppState, draft_state: &mut TextDraftState, reset: bool) {
    match generate_scope_code(&app_state.project, app_state.current_scope) {
        Ok(code) => {
            if draft_state.draft != code {
                draft_state.draft_rev += 1;
            }
            draft_state.source_scope = Some(app_state.current_scope);
            draft_state.draft = code.clone();
            if reset || draft_state.committed.is_empty() || !draft_state.dirty {
                draft_state.committed = code;
                draft_state.committed_rev = draft_state.draft_rev;
                draft_state.dirty = false;
            } else {
                draft_state.dirty = draft_state.draft != draft_state.committed;
            }
            draft_state.last_validation_error = None;
            draft_state.last_validation_ok = true;
            draft_state.clear_pending_validation();
            draft_state.reset_validation_debounce(false);
        }
        Err(error) => {
            draft_state.source_scope = Some(app_state.current_scope);
            draft_state.last_validation_ok = false;
            draft_state.last_validation_error = Some(format!("scope codegen failed: {error}"));
            draft_state.clear_pending_validation();
            draft_state.reset_validation_debounce(false);
        }
    }
}

fn new_validation_timer() -> Timer {
    Timer::from_seconds(TEXT_DRAFT_VALIDATE_DEBOUNCE_SECS, TimerMode::Once)
}

fn short_scope(scope: ScopeId) -> String {
    scope.to_string().chars().take(8).collect()
}

fn render_draft_preview(draft: &str) -> String {
    if draft.trim().is_empty() {
        return "<empty draft>".to_string();
    }

    let mut out = String::new();
    for (idx, line) in draft.lines().take(MAX_PREVIEW_LINES).enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let mut visible = line.to_string();
        if visible.chars().count() > MAX_PREVIEW_LINE_CHARS {
            visible = visible.chars().take(MAX_PREVIEW_LINE_CHARS).collect();
            visible.push_str("...");
        }
        out.push_str(&format!("{:>3}| {}", idx + 1, visible));
    }
    if draft.lines().count() > MAX_PREVIEW_LINES {
        out.push_str("\n...");
    }
    out
}

fn button_target_click_hit(
    click: &On<Pointer<Click>>,
    buttons: &Query<(), impl bevy::ecs::query::QueryFilter>,
) -> bool {
    buttons.get(click.entity).is_ok()
        || buttons.get(click.original_event_target()).is_ok()
        || buttons.get(click.event_target()).is_ok()
}

fn primary_modifier_pressed(keyboard: &ButtonInput<KeyCode>) -> bool {
    keyboard.pressed(KeyCode::ControlLeft)
        || keyboard.pressed(KeyCode::ControlRight)
        || keyboard.pressed(KeyCode::SuperLeft)
        || keyboard.pressed(KeyCode::SuperRight)
}

fn validate_text_draft(draft: &str) -> Result<(), String> {
    let trimmed = draft.trim();
    if trimmed.is_empty() {
        return Err("draft is empty".to_string());
    }
    if !trimmed.contains("main") {
        return Err("draft must include a main expression".to_string());
    }

    let mut stack: Vec<char> = Vec::new();
    let mut in_string = false;
    let mut escape = false;

    for ch in trimmed.chars() {
        if in_string {
            if escape {
                escape = false;
                continue;
            }
            match ch {
                '\\' => escape = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '(' | '[' | '{' => stack.push(ch),
            ')' | ']' | '}' => {
                let Some(open) = stack.pop() else {
                    return Err(format!("unbalanced closing delimiter '{ch}'"));
                };
                if !matches!((open, ch), ('(', ')') | ('[', ']') | ('{', '}')) {
                    return Err(format!("mismatched delimiters '{open}' and '{ch}'"));
                }
            }
            _ => {}
        }
    }

    if in_string {
        return Err("unterminated string literal".to_string());
    }
    if let Some(open) = stack.last() {
        return Err(format!("unclosed delimiter '{open}'"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_text_draft;

    #[test]
    fn validates_basic_main_script() {
        let code = "let main = s(\"bd\").struct(\"<[x _ _ x]>\");\nmain\n";
        assert!(validate_text_draft(code).is_ok());
    }

    #[test]
    fn rejects_missing_main() {
        let code = "let beat = s(\"bd\")";
        let err = validate_text_draft(code).expect_err("missing main should fail");
        assert!(err.contains("main"));
    }

    #[test]
    fn rejects_unbalanced_delimiters() {
        let code = "let main = s(\"bd\").struct(\"<[x _ _ x]>\");\nmain)";
        let err = validate_text_draft(code).expect_err("unbalanced delimiters should fail");
        assert!(err.contains("unbalanced") || err.contains("mismatched"));
    }
}
