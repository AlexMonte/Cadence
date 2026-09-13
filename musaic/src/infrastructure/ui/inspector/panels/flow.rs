//! The named endpoints of a flow node each own their side assignment.
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus, editing::TileEdit, flow::FlowEdit},
        session::MusaicProject,
    },
    domain::flow,
    infrastructure::ui::{theme::MusaicUiTheme, widgets::musaic_button},
};
use bevy::prelude::*;
use bevy_feathers::theme::ThemedText;
use bevy_ui_widgets::{Activate, ValueChange};
use tessera::prelude::{
    FlowControlNode, FlowControlPolicy, InputEndpoint, NodeId, NodeSpatialBindings, OutputEndpoint,
    Rational, SpatialSide,
};

#[derive(Component)]
struct FlowButton {
    node: NodeId,
    edit: FlowEdit,
}
#[derive(Component)]
struct SplitOctave(NodeId);
fn label(parent: &mut ChildSpawnerCommands<'_>, text: &str) {
    parent.spawn((
        Node {
            max_width: percent(100),
            min_width: px(0),
            ..default()
        },
        Text::new(text),
        TextFont {
            font_size: 12.,
            ..default()
        },
        TextColor(MusaicUiTheme::default_dark().chrome.text_main),
        ThemedText,
    ));
}
fn row() -> Node {
    Node {
        width: percent(100),
        flex_wrap: FlexWrap::Wrap,
        column_gap: px(3),
        row_gap: px(3),
        ..default()
    }
}
fn button(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    edit: FlowEdit,
    text: &str,
    selected: bool,
) {
    let theme = MusaicUiTheme::default_dark();
    parent
        .spawn(musaic_button(
            Node {
                min_height: px(25),
                padding: UiRect::axes(px(6), px(3)),
                max_width: percent(100),
                ..default()
            },
            (
                FlowButton {
                    node: node.clone(),
                    edit,
                },
                BackgroundColor(if selected {
                    theme.chrome.button_border
                } else {
                    theme.chrome.button_bg
                }),
            ),
            text,
        ))
        .observe(activate);
}
fn activate(
    event: On<Activate>,
    buttons: Query<&FlowButton>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if let Ok(button) = buttons.get(event.entity) {
        bus.write(EditorCommandBus(EditorCommand::EditTiles(TileEdit::Flow {
            node: button.node.clone(),
            edit: button.edit.clone(),
        })));
    }
}
const SIDES: [(SpatialSide, &str); 5] = [
    (SpatialSide::West, "W"),
    (SpatialSide::North, "N"),
    (SpatialSide::East, "E"),
    (SpatialSide::South, "S"),
    (SpatialSide::Off, "Off"),
];

pub(super) fn spawn(
    parent: &mut ChildSpawnerCommands<'_>,
    node: &NodeId,
    control: &FlowControlNode,
    bindings: &NodeSpatialBindings,
) {
    label(parent, "Flow policy");
    parent.spawn(row()).with_children(|row| {
        for policy in flow::policies(control) {
            button(
                row,
                node,
                FlowEdit::Policy(policy.clone()),
                &flow::policy_label(&policy),
                control.policy == policy,
            );
        }
    });
    if let FlowControlPolicy::SplitByPitchRange { threshold_octave } = control.policy {
        label(parent, "Threshold octave (whole number)");
        let field = crate::infrastructure::ui::widgets::exact_number::spawn(
            parent,
            Rational::from_integer(threshold_octave),
            SplitOctave(node.clone()),
        );
        parent.commands().entity(field).observe(set_split_octave);
    }
    label(
        parent,
        "Each named port connects to adjacent tiles on its chosen side.",
    );
    for endpoint in flow::inputs(control) {
        let name = match &endpoint {
            InputEndpoint::Socket(p) => p.0.clone(),
            InputEndpoint::GroupMember { group, member } => format!("{} / {}", group.0, member.0),
        };
        label(parent, &format!("In · {name}"));
        if let InputEndpoint::GroupMember { group, member } = &endpoint {
            super::flow_member_name::spawn(parent, node, group, member, true);
        }
        let active = bindings
            .inputs
            .get(&endpoint)
            .copied()
            .unwrap_or(SpatialSide::Off);
        parent.spawn(row()).with_children(|row| {
            for (side, text) in SIDES {
                button(
                    row,
                    node,
                    FlowEdit::Input {
                        endpoint: endpoint.clone(),
                        side,
                    },
                    text,
                    side == active,
                );
            }
            if let InputEndpoint::GroupMember { group, member } = &endpoint {
                button(
                    row,
                    node,
                    FlowEdit::RemoveMember {
                        group: group.clone(),
                        member: member.clone(),
                        input: true,
                    },
                    "−",
                    false,
                );
            }
        });
    }
    for group in &control.signature.input_groups {
        button(
            parent,
            node,
            FlowEdit::AddMember {
                group: group.group.clone(),
                input: true,
            },
            &format!("+ {} input", group.group.0),
            false,
        );
    }
    for endpoint in flow::outputs(control) {
        let name = match &endpoint {
            OutputEndpoint::Socket(p) => p.0.clone(),
            OutputEndpoint::GroupMember { group, member } => format!("{} / {}", group.0, member.0),
        };
        label(parent, &format!("Out · {name}"));
        if let OutputEndpoint::GroupMember { group, member } = &endpoint {
            super::flow_member_name::spawn(parent, node, group, member, false);
        }
        let active = bindings
            .outputs
            .get(&endpoint)
            .copied()
            .unwrap_or(SpatialSide::Off);
        parent.spawn(row()).with_children(|row| {
            for (side, text) in SIDES {
                button(
                    row,
                    node,
                    FlowEdit::Output {
                        endpoint: endpoint.clone(),
                        side,
                    },
                    text,
                    side == active,
                );
            }
            if let OutputEndpoint::GroupMember { group, member } = &endpoint {
                button(
                    row,
                    node,
                    FlowEdit::RemoveMember {
                        group: group.clone(),
                        member: member.clone(),
                        input: false,
                    },
                    "−",
                    false,
                );
            }
        });
    }
    for group in &control.signature.output_groups {
        button(
            parent,
            node,
            FlowEdit::AddMember {
                group: group.group.clone(),
                input: false,
            },
            &format!("+ {} output", group.group.0),
            false,
        );
    }
}
fn set_split_octave(
    event: On<ValueChange<Rational>>,
    fields: Query<&SplitOctave>,
    project: Res<MusaicProject>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(field) = fields.get(event.source) else {
        return;
    };
    if event.value.denominator != 1 {
        return;
    }
    let Some(crate::domain::document::DocumentNodeKind::FlowControl(control)) =
        project.document.graph.node(&field.0).map(|n| &n.kind)
    else {
        return;
    };
    if !matches!(control.policy, FlowControlPolicy::SplitByPitchRange { .. }) {
        return;
    }
    bus.write(EditorCommandBus(EditorCommand::EditTiles(TileEdit::Flow {
        node: field.0.clone(),
        edit: FlowEdit::Policy(FlowControlPolicy::SplitByPitchRange {
            threshold_octave: event.value.numerator,
        }),
    })));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flow_inspector_buttons_send_exact_named_endpoint_and_policy_edits() {
        let mut app = App::new();
        app.add_message::<EditorCommandBus>();
        let control = FlowControlNode::new(tessera::prelude::FlowControlKind::Split);
        let bindings = tessera::prelude::default_spatial_bindings(
            &tessera::prelude::RootSurfaceNodeKind::FlowControl(control.clone()),
        );
        let mut queue = bevy::ecs::world::CommandQueue::default();
        let mut commands = Commands::new(&mut queue, app.world());
        commands
            .spawn_empty()
            .with_children(|parent| spawn(parent, &NodeId::new("split"), &control, &bindings));
        queue.apply(app.world_mut());
        app.update();
        let (entity, expected) = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &FlowButton)>();
            query.iter(world).find_map(|(entity,button)|matches!(&button.edit,FlowEdit::Output { endpoint: OutputEndpoint::GroupMember{member,..},side:SpatialSide::South } if member.0=="odd").then(||(entity,button.edit.clone()))).unwrap()
        };
        app.world_mut().trigger(Activate { entity });
        let messages = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(messages.len(), 1);
        assert_eq!(
            messages[0].0,
            EditorCommand::EditTiles(TileEdit::Flow {
                node: NodeId::new("split"),
                edit: expected
            })
        );
        let entity = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &FlowButton)>();
            query
                .iter(world)
                .find_map(|(e, b)| {
                    matches!(b.edit, FlowEdit::Policy(FlowControlPolicy::SplitCopyToAll))
                        .then_some(e)
                })
                .unwrap()
        };
        app.world_mut().trigger(Activate { entity });
        let messages = app
            .world_mut()
            .resource_mut::<Messages<EditorCommandBus>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(
            messages[0].0,
            EditorCommand::EditTiles(TileEdit::Flow {
                node: NodeId::new("split"),
                edit: FlowEdit::Policy(FlowControlPolicy::SplitCopyToAll)
            })
        );
    }
}
