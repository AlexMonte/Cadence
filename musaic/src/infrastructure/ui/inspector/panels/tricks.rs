//! Focused endpoint naming; Enter commits one atomic rename through history.
use crate::{
    application::command::{EditorCommand, EditorCommandBus, editing::TileEdit},
    infrastructure::ui::theme::MusaicUiTheme,
};
use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    input_focus::{FocusedInput, InputFocus, tab_navigation::TabIndex},
    prelude::*,
};
use bevy_feathers::theme::ThemedText;
use tessera::prelude::NodeId;
#[derive(Component)]
struct MemberName {
    node: NodeId,
    input: Option<NodeId>,
    output: bool,
    trick: Option<u64>,
    original: String,
    draft: String,
    selected: bool,
}
fn field(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    input: Option<NodeId>,
    output: bool,
    trick: Option<u64>,
    name: &str,
) {
    parent
        .spawn((
            Node {
                width: percent(100),
                min_height: px(30),
                padding: UiRect::all(px(6)),
                border: UiRect::all(px(1)),
                ..default()
            },
            BackgroundColor(MusaicUiTheme::default_dark().chrome.panel_bg),
            BorderColor::all(MusaicUiTheme::default_dark().chrome.button_border),
            TabIndex(0),
            Interaction::default(),
            Text::new(format!("Name: {name}")),
            TextFont {
                font_size: 13.,
                ..default()
            },
            TextColor(MusaicUiTheme::default_dark().chrome.text_main),
            ThemedText,
            MemberName {
                node: node.clone(),
                input,
                output,
                trick,
                original: name.into(),
                draft: name.into(),
                selected: false,
            },
        ))
        .observe(focus)
        .observe(type_name);
}
pub(crate) fn spawn(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    paint: &crate::application::tricks::TileCodePaint,
) {
    use super::tile_presentation::inspector_label as label;
    if let Some(name) = &paint.output_name {
        label(parent, "Timeline name", 15., false);
        field(parent, node, None, true, None, name);
        label(
            parent,
            &format!("Blank uses the instrument name · {}", paint.automatic_name),
            12.,
            true,
        );
        label(
            parent,
            "Click to type · Enter saves · Escape cancels",
            12.,
            true,
        );
        return;
    }
    if let Some((id, name, source)) = &paint.definition {
        field(parent, node, None, false, Some(*id), name);
        label(parent, &format!("Linked trick: {name}"), 15., false);
        label(
            parent,
            "Source edits update every linked tile. Duplicating this tile keeps the link.",
            12.,
            true,
        );
        uses_button(parent, *id, name, paint.linked_count);
        parent
            .spawn(crate::infrastructure::ui::widgets::musaic_button(
                Node::default(),
                crate::infrastructure::ui::InspectorButtonAction(EditorCommand::EditTiles(
                    crate::application::command::editing::TileEdit::IndependentVariation,
                )),
                "Duplicate as independent variation",
            ))
            .observe(crate::infrastructure::ui::on_inspector_button_activated);
        parent
            .spawn(crate::infrastructure::ui::widgets::musaic_button(
                Node::default(),
                crate::infrastructure::ui::linked_uses::SourceButton(source.clone()),
                "Edit source tiles",
            ))
            .observe(crate::infrastructure::ui::linked_uses::source_button);
        return;
    }
    if let Some(name) = &paint.suggested_name {
        label(parent, "Save reusable trick", 15., false);
        label(
            parent,
            "Enter a name to save this pattern. Place the trick from the tile library.",
            12.,
            true,
        );
        field(parent, node, None, false, None, name);
        for (input, title) in &paint.inputs {
            label(parent, &format!("Function input: {title}"), 13., false);
            label(
                parent,
                "Enter a name to reuse this code with a different input pattern.",
                12.,
                true,
            );
            field(parent, node, Some(input.clone()), false, None, name);
        }
    }
}
fn focus(
    mut event: On<Pointer<Press>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut MemberName, &mut Text)>,
) {
    let Ok((mut field, mut text)) = fields.get_mut(event.entity) else {
        return;
    };
    field.selected = true;
    text.0 = format!("Name: [{}] · Enter to save", field.draft);
    focus.set(event.entity);
    event.propagate(false);
}
fn type_name(
    mut event: On<FocusedInput<KeyboardInput>>,
    mut keyboard: ResMut<ButtonInput<KeyCode>>,
    mut focus: ResMut<InputFocus>,
    mut fields: Query<(&mut MemberName, &mut Text)>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if event.input.state != ButtonState::Pressed || event.input.key_code == KeyCode::Tab {
        return;
    }
    let Ok((mut field, mut text)) = fields.get_mut(event.focused_entity) else {
        return;
    };
    keyboard.clear_just_pressed(event.input.key_code);
    let control = [
        KeyCode::SuperLeft,
        KeyCode::SuperRight,
        KeyCode::ControlLeft,
        KeyCode::ControlRight,
    ]
    .into_iter()
    .any(|key| keyboard.pressed(key));
    match event.input.key_code {
        KeyCode::Enter | KeyCode::NumpadEnter => {
            let edit = if let Some(id) = field.trick {
                TileEdit::RenameTrick {
                    id,
                    name: field.draft.clone(),
                }
            } else if field.output {
                TileEdit::RenameOutput {
                    node: field.node.clone(),
                    name: field.draft.clone(),
                }
            } else {
                TileEdit::DefineTrick {
                    node: field.node.clone(),
                    name: field.draft.clone(),
                    input: field.input.clone(),
                }
            };
            bus.write(EditorCommandBus(EditorCommand::EditTiles(edit)));
            field.selected = false;
            focus.clear();
        }
        KeyCode::Escape => {
            field.draft = field.original.clone();
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
            let value = event.input.text.as_deref().map(str::to_owned).or_else(|| {
                match &event.input.logical_key {
                    Key::Character(text) => Some(text.to_string()),
                    Key::Space => Some(" ".into()),
                    _ => None,
                }
            });
            if let Some(value) = value.filter(|value| !value.chars().any(char::is_control)) {
                let capacity = if field.selected { 0 } else { field.draft.len() };
                if capacity + value.len() <= 128 {
                    if field.selected {
                        field.draft.clear();
                    }
                    field.draft.push_str(&value);
                    field.selected = false;
                }
            }
        }
        _ => {}
    }
    text.0 = if field.selected {
        format!("Name: [{}]", field.draft)
    } else {
        format!("Name: {}", field.draft)
    };
    event.propagate(false);
}

fn uses_button(parent: &mut ChildSpawnerCommands<'_>, id: u64, name: &str, count: usize) {
    parent
        .spawn(crate::infrastructure::ui::widgets::musaic_button(
            Node {
                min_height: px(30),
                padding: UiRect::all(px(5)),
                ..default()
            },
            crate::infrastructure::ui::linked_uses::UsesButton(id),
            format!("Find linked uses · {name} ({count})"),
        ))
        .observe(crate::infrastructure::ui::linked_uses::open_button);
}

pub(crate) fn spawn_shared(
    parent: &mut ChildSpawnerCommands<'_>,
    paint: &crate::application::tricks::TileCodePaint,
) {
    use super::tile_presentation::inspector_label as label;
    for (id, name, count) in &paint.linked_sources {
        label(parent, &format!("Shared source for {name}"), 15., false);
        label(
            parent,
            "Editing these source tiles updates every linked use.",
            12.,
            true,
        );
        uses_button(parent, *id, name, *count);
    }
}
