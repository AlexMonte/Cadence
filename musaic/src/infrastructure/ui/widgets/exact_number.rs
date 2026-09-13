//! Exact numeric text entry, emitting the same typed value-change event as a slider.
use crate::infrastructure::ui::theme::MusaicUiTheme;
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use bevy_ui_widgets::ValueChange;
use tessera::prelude::Rational;

#[derive(Component)]
pub(crate) struct ExactNumber {
    pub value: Rational,
    draft: String,
    selected: bool,
}
impl ExactNumber {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn draft_value(&self) -> Result<Rational, &'static str> {
        parse(&self.draft)
    }
}
#[derive(Component)]
struct NumberHitArea(Entity);

fn label(value: Rational) -> String {
    if value.denominator == 1 {
        value.numerator.to_string()
    } else {
        format!("{}/{}", value.numerator, value.denominator)
    }
}

/// The slider's displayed value is the text field itself, with the same exact
/// parser and domain-specific observer used by standalone number controls.
pub(crate) fn spawn_inline<B: Bundle>(
    parent: &mut ChildSpawnerCommands<'_>,
    value: Rational,
    binding: B,
) -> Entity {
    let entity = spawn(parent, value, binding);
    parent.commands().entity(entity).insert(Node {
        position_type: PositionType::Absolute,
        right: px(0),
        top: px(-2),
        min_width: px(44),
        min_height: px(22),
        max_width: percent(100),
        padding: UiRect::axes(px(4), px(2)),
        border: UiRect::bottom(px(1)),
        ..default()
    });
    entity
}

pub(crate) fn spawn<B: Bundle>(
    parent: &mut ChildSpawnerCommands<'_>,
    value: Rational,
    binding: B,
) -> Entity {
    let theme = MusaicUiTheme::default_dark();
    let field = parent
        .spawn((
            Node {
                min_height: px(28),
                padding: UiRect::axes(px(7), px(4)),
                border: UiRect::all(px(1)),
                ..default()
            },
            TabIndex(0),
            Text::new(label(value)),
            TextFont {
                font_size: 12.,
                ..default()
            },
            TextColor(theme.chrome.text_main),
            BackgroundColor(theme.chrome.panel_bg),
            BorderColor::all(theme.chrome.button_border),
            ExactNumber {
                value,
                draft: label(value),
                selected: false,
            },
            binding,
        ))
        .observe(focus)
        .observe(type_number)
        .id();
    // Bevy 0.18 text picking only covers text runs, not the surrounding Node.
    // A real box keeps padding and empty drafts clickable as well.
    parent.commands().entity(field).with_children(|input| {
        input
            .spawn((
                NumberHitArea(field),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0),
                    top: px(0),
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                Pickable::default(),
            ))
            .observe(focus);
    });
    field
}
fn focus(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    targets: Query<&NumberHitArea>,
    mut fields: Query<(&mut ExactNumber, &mut Text)>,
) {
    let target = targets
        .get(event.entity)
        .map_or(event.entity, |area| area.0);
    if event.button != PointerButton::Primary {
        return;
    }
    if let Ok((mut field, mut text)) = fields.get_mut(target) {
        field.selected = true;
        text.0 = format!("[{}]", field.draft);
        focus.set(target);
        event.propagate(false);
    }
}
fn type_number(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut ExactNumber, &mut Text)>,
    mut commands: Commands,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut field, mut text)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    let key = event.input.key_code;
    keyboard.clear_just_pressed(key);
    let control = [
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ]
    .into_iter()
    .any(|key| keyboard.pressed(key));
    match key {
        KeyCode::Enter | KeyCode::NumpadEnter => match parse(&field.draft) {
            Ok(value) => {
                commands.trigger(ValueChange {
                    source: event.focused_entity,
                    value,
                });
                field.selected = false;
                focus.clear();
            }
            Err(error) => {
                text.0 = format!("{} · {error}", field.draft);
                event.propagate(false);
                return;
            }
        },
        KeyCode::Escape => {
            field.draft = label(field.value);
            field.selected = false;
            focus.clear();
        }
        KeyCode::KeyA if control => field.selected = true,
        KeyCode::Backspace | KeyCode::Delete => {
            if field.selected {
                field.draft.clear();
            } else {
                field.draft.pop();
            }
            field.selected = false;
        }
        _ if !control => {
            let value = event
                .input
                .text
                .as_deref()
                .or_else(|| match &event.input.logical_key {
                    Key::Character(value) => Some(value.as_str()),
                    _ => None,
                });
            if let Some(value) = value {
                if field.selected {
                    field.draft.clear();
                    field.selected = false;
                }
                for c in value.chars().filter(|c| !c.is_control()) {
                    if field.draft.len() + c.len_utf8() <= 64 {
                        field.draft.push(c);
                    }
                }
            }
        }
        _ => {}
    }
    text.0 = format!(
        "{}{}",
        field.draft,
        if focus.0 == Some(event.focused_entity) {
            " ▏"
        } else {
            ""
        }
    );
    event.propagate(false);
}
pub(crate) fn parse(text: &str) -> Result<Rational, &'static str> {
    const HELP: &str = "Use a number or fraction, such as -0.5 or 1/3";
    let text = text.trim();
    let (n, d) = if let Some((a, b)) = text.split_once('/') {
        (
            a.trim().parse::<i64>().map_err(|_| HELP)?,
            b.trim().parse::<i64>().map_err(|_| HELP)?,
        )
    } else if let Some((whole, fraction)) = text.split_once('.') {
        if fraction.is_empty()
            || fraction.len() > 6
            || !fraction.bytes().all(|c| c.is_ascii_digit())
        {
            return Err(HELP);
        }
        let negative = whole.starts_with('-');
        let whole = whole.strip_prefix('-').unwrap_or(whole);
        let whole = if whole.is_empty() {
            0
        } else {
            whole.parse::<i64>().map_err(|_| HELP)?
        };
        if whole < 0 {
            return Err(HELP);
        }
        let d = 10i64.pow(fraction.len() as u32);
        let n = whole
            .checked_mul(d)
            .and_then(|v| v.checked_add(fraction.parse::<i64>().ok()?))
            .ok_or("Number is too large")?;
        (if negative { -n } else { n }, d)
    } else {
        (text.parse::<i64>().map_err(|_| HELP)?, 1)
    };
    if !(1..=1_000_000).contains(&d) || n.unsigned_abs() > 1_000_000_000_000 {
        return Err("Number is outside the supported exact range");
    }
    Ok(Rational::new(n, d))
}
pub(crate) fn update(field: &mut ExactNumber, text: &mut Text, value: Rational, focused: bool) {
    field.value = value;
    if !focused {
        field.draft = label(value);
        text.0.clone_from(&field.draft);
    }
}
pub(crate) fn reject(text: &mut Text, focus: &mut InputFocus, entity: Entity, message: &str) {
    text.0 = format!("{message} · Enter a valid value");
    focus.set(entity);
}

#[cfg(test)]
mod tests {
    use super::*;
    proptest::proptest! {
        #[test]
        fn signed_fraction_roundtrip(n in -1_000_000i64..=1_000_000,d in 1i64..=1_000_000){let r=Rational::new(n,d);proptest::prop_assert_eq!(parse(&label(r)),Ok(r));}
    }
    #[test]
    fn rejects_overflow_and_zero_denominator() {
        for input in [
            "1/0",
            "9223372036854775807.9",
            "-9223372036854775808",
            "NaN",
            "1/-2",
            "--1.3",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
        assert_eq!(parse("-0.125"), Ok(Rational::new(-1, 8)));
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    #[derive(Resource, Default)]
    struct Values(Vec<Rational>);
    fn receive(event: On<ValueChange<Rational>>, mut values: ResMut<Values>) {
        values.0.push(event.value);
    }
    fn fixture() -> (App, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
        ))
        .init_resource::<Values>();
        app.configure_sets(
            PreUpdate,
            bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
        );
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let field = app
            .world_mut()
            .spawn((
                ExactNumber {
                    value: Rational::one(),
                    draft: "1".into(),
                    selected: true,
                },
                Text::default(),
            ))
            .observe(type_number)
            .observe(receive)
            .id();
        app.world_mut().resource_mut::<InputFocus>().set(field);
        (app, window, field)
    }
    fn key(app: &mut App, window: Entity, code: KeyCode, text: Option<&str>) {
        app.world_mut().write_message(KeyboardInput {
            key_code: code,
            logical_key: text.map(|t| Key::Character(t.into())).unwrap_or(Key::Enter),
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window,
        });
        app.update();
    }
    #[test]
    fn exact_fraction_commits_and_typing_consumes_editor_shortcuts() {
        let (mut app, window, field) = fixture();
        key(&mut app, window, KeyCode::KeyD, Some("1/3"));
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyD)
        );
        key(&mut app, window, KeyCode::Enter, None);
        assert_eq!(app.world().resource::<Values>().0, [Rational::new(1, 3)]);
        assert_eq!(app.world().resource::<InputFocus>().0, None);
        assert!(!app.world().get::<ExactNumber>(field).unwrap().selected);
    }
    #[test]
    fn invalid_input_does_not_commit_and_escape_restores_original() {
        let (mut app, window, field) = fixture();
        key(&mut app, window, KeyCode::Digit1, Some("1/0"));
        key(&mut app, window, KeyCode::Enter, None);
        assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
        assert!(app.world().resource::<Values>().0.is_empty());
        key(&mut app, window, KeyCode::Escape, None);
        assert_eq!(app.world().get::<ExactNumber>(field).unwrap().draft, "1");
        assert_eq!(app.world().resource::<InputFocus>().0, None);
    }
}
