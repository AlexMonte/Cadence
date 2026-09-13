//! Stable playback overlays. Musical time controls motion; wall time is unused.
use bevy::{picking::prelude::Pickable, prelude::*};
use tessera::prelude::NodeId;

use super::{components::Board3dTile, scene_reconcile::sync_board_3d_scene};
use crate::application::pipeline::{
    runtime::activity::PlaybackActivity, scene_sync::VisibleBoardState,
};

#[derive(Resource)]
struct ActivityMaterials {
    glow: Handle<StandardMaterial>,
    packet: Handle<StandardMaterial>,
    horizontal: Handle<Mesh>,
    vertical: Handle<Mesh>,
    packet_mesh: Handle<Mesh>,
}
impl FromWorld for ActivityMaterials {
    fn from_world(world: &mut World) -> Self {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        let horizontal = meshes.add(Cuboid::new(0.94, 0.008, 0.025));
        let vertical = meshes.add(Cuboid::new(0.025, 0.008, 0.94));
        let packet_mesh = meshes.add(Cuboid::new(0.10, 0.012, 0.052));
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        let glow = materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.63, 0.46),
            unlit: true,
            ..default()
        });
        let packet = materials.add(StandardMaterial {
            base_color: Color::srgb(0.05, 0.72, 0.50),
            unlit: true,
            ..default()
        });
        Self {
            glow,
            packet,
            horizontal,
            vertical,
            packet_mesh,
        }
    }
}

#[derive(Component)]
struct TilePlaybackGlow {
    node: NodeId,
}

/// Attached to a persistent connection by its owning scene reconcile.
#[derive(Component)]
pub(super) struct ConnectionPlaybackPath {
    pub from: NodeId,
    pub to: NodeId,
    pub length: f32,
    pub offset: f32,
    pub total: f32,
}
#[derive(Component)]
struct ConnectionPlaybackPacket {
    from: NodeId,
    to: NodeId,
    length: f32,
    offset: f32,
    total: f32,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<PlaybackActivity>()
        .init_resource::<ActivityMaterials>()
        .add_systems(
            Update,
            (attach_activity_overlays, present_activity)
                .chain()
                .after(sync_board_3d_scene)
                .after(crate::infrastructure::app::MusaicSet::Runtime)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
}

fn attach_activity_overlays(
    mut commands: Commands,
    assets: Res<ActivityMaterials>,
    visible: Res<VisibleBoardState>,
    tiles: Query<(Entity, &Board3dTile), Added<Board3dTile>>,
    paths: Query<(Entity, &ConnectionPlaybackPath), Added<ConnectionPlaybackPath>>,
) {
    for (entity, tile) in &tiles {
        let dimensions = visible
            .nodes
            .iter()
            .find(|node| node.node == tile.node)
            .map(|node| {
                let f = node
                    .tessera_footprint
                    .unwrap_or(tessera::prelude::TileFootprint::unit());
                Vec2::new(f.width as f32, f.height as f32)
            })
            .unwrap_or(Vec2::ONE);
        commands.entity(entity).with_children(|parent| {
            parent
                .spawn((
                    Name::new("Playing note outline"),
                    TilePlaybackGlow {
                        node: tile.node.clone(),
                    },
                    Transform::from_xyz(0.0, 0.102, 0.0),
                    Visibility::Hidden,
                    Pickable::IGNORE,
                ))
                .with_children(|outline| {
                    for (mesh, x, z) in [
                        (&assets.horizontal, 0.0, -0.47),
                        (&assets.horizontal, 0.0, 0.47),
                        (&assets.vertical, -0.47, 0.0),
                        (&assets.vertical, 0.47, 0.0),
                    ] {
                        outline.spawn((
                            Mesh3d(mesh.clone()),
                            MeshMaterial3d(assets.glow.clone()),
                            Transform::from_xyz(x * dimensions.x, 0.0, z * dimensions.y)
                                .with_scale(Vec3::new(dimensions.x, 1.0, dimensions.y)),
                            Visibility::Inherited,
                            Pickable::IGNORE,
                        ));
                    }
                });
        });
    }
    for (entity, path) in &paths {
        commands.entity(entity).with_children(|parent| {
            parent.spawn((
                Name::new("Playing connection flow"),
                ConnectionPlaybackPacket {
                    from: path.from.clone(),
                    to: path.to.clone(),
                    length: path.length,
                    offset: path.offset,
                    total: path.total,
                },
                Mesh3d(assets.packet_mesh.clone()),
                MeshMaterial3d(assets.packet.clone()),
                // Keep the small moving marker above compact faces; the wire
                // itself stays underneath and never covers the tile notation.
                Transform::from_xyz(0.0, 0.12, 0.0),
                Visibility::Hidden,
                Pickable::IGNORE,
            ));
        });
    }
}

/// Owned modifier pieces share one face but keep independent authored identity.
fn face_level(activity: &PlaybackActivity, visible: &VisibleBoardState, node: &NodeId) -> f32 {
    let direct = activity.nodes.get(node).copied().unwrap_or(0.0);
    visible
        .atom_compounds
        .iter()
        .find(|compound| compound.compound.members.first() == Some(node))
        .map(|compound| {
            compound
                .compound
                .members
                .iter()
                .filter_map(|member| activity.nodes.get(member).copied())
                .fold(direct, f32::max)
        })
        .unwrap_or(direct)
}

fn present_activity(
    activity: Res<PlaybackActivity>,
    preferences: Option<Res<crate::application::editor::preferences::EditorPreferences>>,
    visible: Res<VisibleBoardState>,
    mut outlines: Query<
        (&TilePlaybackGlow, &mut Visibility, &mut Transform),
        Without<ConnectionPlaybackPacket>,
    >,
    mut packets: Query<
        (&ConnectionPlaybackPacket, &mut Visibility, &mut Transform),
        Without<TilePlaybackGlow>,
    >,
) {
    let reduced_motion = preferences
        .as_ref()
        .is_some_and(|preferences| preferences.reduced_motion);
    for (glow, mut visibility, mut transform) in &mut outlines {
        let level = if activity.playing {
            face_level(&activity, &visible, &glow.node)
        } else {
            0.0
        };
        let next = if level > 0.001 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
        if level > 0.001 {
            // An onset opens the outline very slightly; held notes settle.
            let scale = if reduced_motion {
                Vec3::ONE
            } else {
                Vec3::new(1.0 + 0.014 * level, 1.0, 1.0 + 0.014 * level)
            };
            if transform.scale != scale {
                transform.scale = scale;
            }
        }
    }
    for (packet, mut visibility, mut transform) in &mut packets {
        let distance = if reduced_motion {
            0.5 * packet.total
        } else {
            (activity.cycle * 4.0).rem_euclid(1.0) as f32 * packet.total
        };
        let active = distance >= packet.offset
            && distance < packet.offset + packet.length
            && activity.playing
            && activity
                .connections
                .contains(&(packet.from.clone(), packet.to.clone()));
        let next = if active {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != next {
            *visibility = next;
        }
        if active {
            // The midpoint/yaw belongs to the connection entity. Local +X is
            // always source -> destination, including leftward/upward links.
            transform.translation.x = distance - packet.offset - 0.5 * packet.length;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            editor::{AtomCompoundSemantic, AtomCompoundView},
            pipeline::scene_sync::VisibleAtomCompound,
        },
        domain::{
            board::{BoardSlot, BoardSurfaceId},
            document::PlacementAddress,
        },
    };

    fn app() -> App {
        let mut app = App::new();
        app.insert_resource(Assets::<Mesh>::default())
            .insert_resource(Assets::<StandardMaterial>::default())
            .init_resource::<ActivityMaterials>()
            .init_resource::<PlaybackActivity>()
            .init_resource::<VisibleBoardState>()
            .add_systems(Update, (attach_activity_overlays, present_activity).chain());
        app
    }
    fn node(value: &str) -> NodeId {
        NodeId::new(value)
    }
    fn spawn_tile(app: &mut App, name: &str) -> Entity {
        app.world_mut()
            .spawn((
                Board3dTile {
                    surface: BoardSurfaceId(0),
                    node: node(name),
                    address: PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                },
                Transform::default(),
            ))
            .id()
    }

    #[test]
    fn a_sounding_hidden_owned_piece_highlights_its_compact_face() {
        let mut app = app();
        spawn_tile(&mut app, "face");
        app.world_mut()
            .resource_mut::<VisibleBoardState>()
            .atom_compounds
            .push(VisibleAtomCompound {
                slot: BoardSlot::new(0, 0),
                primary_atom: None,
                compound: AtomCompoundView {
                    members: vec![node("face"), node("sounding_note"), node("octave")],
                    display: "C4".into(),
                    semantic: AtomCompoundSemantic::Single,
                },
            });
        *app.world_mut().resource_mut::<PlaybackActivity>() = PlaybackActivity {
            playing: true,
            nodes: [(node("sounding_note"), 0.8)].into(),
            ..default()
        };
        app.update();
        let visibility = app
            .world_mut()
            .query_filtered::<&Visibility, With<TilePlaybackGlow>>()
            .single(app.world())
            .unwrap();
        assert_eq!(*visibility, Visibility::Inherited);
    }

    #[test]
    fn one_packet_follows_the_whole_bent_cable_across_segment_boundaries() {
        let mut app = app();
        for offset in [0.0, 1.0] {
            app.world_mut().spawn((
                ConnectionPlaybackPath {
                    from: node("a"),
                    to: node("b"),
                    length: 1.0,
                    offset,
                    total: 2.0,
                },
                Transform::default(),
            ));
        }
        *app.world_mut().resource_mut::<PlaybackActivity>() = PlaybackActivity {
            playing: true,
            cycle: 0.1,
            connections: [(node("a"), node("b"))].into(),
            ..default()
        };
        app.update();
        for (cycle, expected_offset) in [(0.1, 0.0), (0.15, 1.0), (0.3, 0.0)] {
            app.world_mut().resource_mut::<PlaybackActivity>().cycle = cycle;
            app.update();
            let visible: Vec<_> = app
                .world_mut()
                .query::<(&ConnectionPlaybackPacket, &Visibility)>()
                .iter(app.world())
                .filter(|(_, visibility)| **visibility == Visibility::Inherited)
                .map(|(packet, _)| packet.offset)
                .collect();
            assert_eq!(visible, vec![expected_offset]);
        }
    }

    #[test]
    fn playback_updates_reuse_entities_and_meshes_and_stop_clears_every_overlay() {
        let mut app = app();
        let tile = spawn_tile(&mut app, "note");
        app.world_mut().spawn((
            ConnectionPlaybackPath {
                from: node("note"),
                to: node("out"),
                length: 2.0,
                offset: 0.0,
                total: 2.0,
            },
            Transform::default(),
        ));
        *app.world_mut().resource_mut::<PlaybackActivity>() = PlaybackActivity {
            playing: true,
            cycle: 0.0625,
            nodes: [(node("note"), 1.0)].into(),
            connections: [(node("note"), node("out"))].into(),
        };
        app.update();
        let entities = app.world().entities().len();
        let meshes = app.world().resource::<Assets<Mesh>>().len();
        let mut query = app
            .world_mut()
            .query_filtered::<(Entity, &Transform, &Visibility), With<ConnectionPlaybackPacket>>();
        let (packet, first, visibility) = query.single(app.world()).unwrap();
        let first = *first;
        assert_eq!(*visibility, Visibility::Inherited);
        app.world_mut().resource_mut::<PlaybackActivity>().cycle = 0.1875;
        app.update();
        let (same, next, _) = query.single(app.world()).unwrap();
        assert_eq!(packet, same);
        assert_ne!(first.translation.x, next.translation.x);
        assert!(app.world().get::<Board3dTile>(tile).is_some());
        assert_eq!(entities, app.world().entities().len());
        assert_eq!(meshes, app.world().resource::<Assets<Mesh>>().len());
        // Repeated render updates at an unchanged audio clock cannot animate.
        let fixed = *next;
        app.update();
        assert_eq!(query.single(app.world()).unwrap().1, &fixed);
        app.insert_resource(crate::application::editor::preferences::EditorPreferences {
            reduced_motion: true,
            ..default()
        });
        app.update();
        let reduced = *query.single(app.world()).unwrap().1;
        app.world_mut().resource_mut::<PlaybackActivity>().cycle = 0.4;
        app.update();
        assert_eq!(query.single(app.world()).unwrap().1, &reduced);
        assert_eq!(
            *query.single(app.world()).unwrap().2,
            Visibility::Inherited,
            "reduced motion keeps the active route visible"
        );
        *app.world_mut().resource_mut::<PlaybackActivity>() = PlaybackActivity::default();
        app.update();
        assert_eq!(*query.single(app.world()).unwrap().2, Visibility::Hidden);
        assert!(
            app.world_mut()
                .query_filtered::<&Visibility, With<TilePlaybackGlow>>()
                .iter(app.world())
                .all(|visibility| *visibility == Visibility::Hidden)
        );
    }
}
