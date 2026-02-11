//! Node-level chrome controls such as hover affordances and close actions.

use bevy::picking::hover::Hovered;
use bevy::{camera::visibility::RenderLayers, prelude::*};

use crate::EditorSystemSet;
use crate::core::NodeId;
use crate::editor::AppState;
use crate::editor::camera::CANVAS_RENDER_LAYER;
use crate::editor::canvas::CanvasNode;
use crate::editor::gestures::pointer::LayoutChanged;
use crate::editor::semantics::SelectionState;
use crate::editor::state::EditorReady;
use crate::editor::ui::assets::{UiIconAssets, UiIconId};
use crate::editor::ui::interactions::InteractiveChild;

const CLOSE_BUTTON_SIZE: f32 = 18.0;
const CLOSE_BUTTON_PADDING: f32 = 10.0;
const CLOSE_BUTTON_Z: f32 = 0.25;

#[derive(Component)]
/// Marker for node-level chrome entities.
pub struct NodeChrome;

#[derive(Component, Clone, Copy, Debug)]
/// Click target that removes a specific canvas node.
pub struct NodeCloseButton {
    pub node_id: NodeId,
}

#[derive(Resource, Default)]
/// Tracks the node currently hovered by the pointer.
pub struct HoveredNode {
    pub node_id: Option<NodeId>,
}

pub(super) fn plugin(app: &mut App) {
    app.init_resource::<HoveredNode>();
    app.add_systems(
        Update,
        (
            spawn_node_close_buttons,
            refresh_hovered_node,
            sync_close_button_visibility,
        )
            .chain()
            .run_if(in_state(EditorReady))
            .in_set(EditorSystemSet::RenderSelection),
    );
}

fn spawn_node_close_buttons(
    mut commands: Commands,
    nodes: Query<(Entity, &CanvasNode), Added<CanvasNode>>,
    icon_assets: Res<UiIconAssets>,
) {
    for (entity, node) in &nodes {
        let mut sprite = Sprite::from_image(icon_assets.get(UiIconId::Close));
        sprite.custom_size = Some(Vec2::splat(CLOSE_BUTTON_SIZE));
        sprite.color = Color::srgb(1.0, 1.0, 1.0);

        let close_position = Vec3::new(
            node.width * 0.5 - CLOSE_BUTTON_PADDING,
            node.height * 0.5 - CLOSE_BUTTON_PADDING,
            CLOSE_BUTTON_Z,
        );

        commands.entity(entity).with_children(|parent| {
            parent
                .spawn((
                    Name::new("Node Close Button"),
                    NodeChrome,
                    NodeCloseButton {
                        node_id: node.node_id,
                    },
                    sprite,
                    Transform::from_translation(close_position),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                    Hovered(false),
                    InteractiveChild,
                    RenderLayers::layer(CANVAS_RENDER_LAYER as usize),
                ))
                .observe(remove_node_from_close_button);
        });
    }
}

fn refresh_hovered_node(
    mut hovered_node: ResMut<HoveredNode>,
    nodes: Query<(&CanvasNode, &Hovered)>,
) {
    hovered_node.node_id = nodes
        .iter()
        .find_map(|(node, hovered)| hovered.0.then_some(node.node_id));
}

fn sync_close_button_visibility(
    hovered_node: Res<HoveredNode>,
    selection: Res<SelectionState>,
    mut close_buttons: Query<
        (
            &NodeCloseButton,
            &mut Visibility,
            &mut Pickable,
            &mut Sprite,
            Option<&Hovered>,
        ),
        With<NodeChrome>,
    >,
) {
    for (close_button, mut visibility, mut pickable, mut sprite, hovered) in &mut close_buttons {
        let show = selection.is_selected(close_button.node_id)
            || hovered_node.node_id == Some(close_button.node_id);

        if show {
            *visibility = Visibility::Visible;
            *pickable = Pickable::default();
        } else {
            *visibility = Visibility::Hidden;
            *pickable = Pickable::IGNORE;
        }

        let hovered = hovered.is_some_and(|value| value.0);
        sprite.color = if hovered {
            Color::srgb(1.0, 0.65, 0.65)
        } else {
            Color::srgb(1.0, 1.0, 1.0)
        };
    }
}

fn remove_node_from_close_button(
    click: On<Pointer<Click>>,
    close_buttons: Query<&NodeCloseButton>,
    mut app_state: ResMut<AppState>,
    mut selection: ResMut<SelectionState>,
    mut hovered_node: ResMut<HoveredNode>,
    mut layout_changed: MessageWriter<LayoutChanged>,
) {
    let Ok(close_button) = close_buttons.get(click.entity) else {
        return;
    };

    let removed = app_state.project.remove_node(close_button.node_id);
    if !removed {
        return;
    }

    selection.selected.retain(|id| *id != close_button.node_id);
    if selection.primary == Some(close_button.node_id) {
        selection.primary = selection.selected.first().copied();
    }

    if hovered_node.node_id == Some(close_button.node_id) {
        hovered_node.node_id = None;
    }

    layout_changed.write(LayoutChanged {
        scope: Some(app_state.current_scope),
    });
}
