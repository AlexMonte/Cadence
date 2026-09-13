//! Screen-size detail switching owns only retained mesh handles, never authored state.
use super::Board3dCamera;
use crate::application::pipeline::scene_sync::{TileSurfaceContent, VisibleNodeKind};
use crate::infrastructure::ui::theme::MusaicUiTheme;
use crate::infrastructure::{
    app::AppState,
    ui::{
        board_camera_nav::UiBoardViewport,
        camera_rig::CameraRigSet,
        tile_surface::{self, compact_tile_mesh},
    },
};
use bevy::prelude::*;
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum BoardDetailLevel {
    #[default]
    Full,
    Label,
    Overview,
}
#[derive(Component)]
#[require(BoardDetailLevel)]
pub(super) struct DetailMeshes {
    full: Handle<Mesh>,
    label: Handle<Mesh>,
    overview: Handle<Mesh>,
    full_min: f32,
    label_min: f32,
}
impl DetailMeshes {
    fn next(&self, current: BoardDetailLevel, pixels: f32) -> BoardDetailLevel {
        use BoardDetailLevel::*;
        let full = self.full_min + if current == Full { 0.0 } else { 6.0 };
        let label = self.label_min + if current == Overview { 4.0 } else { 0.0 };
        if pixels >= full {
            Full
        } else if pixels >= label {
            Label
        } else {
            Overview
        }
    }
    fn handle(&self, level: BoardDetailLevel) -> Handle<Mesh> {
        match level {
            BoardDetailLevel::Full => &self.full,
            BoardDetailLevel::Label => &self.label,
            BoardDetailLevel::Overview => &self.overview,
        }
        .clone()
    }
    pub(super) fn port(full: Handle<Mesh>, coarse: Handle<Mesh>) -> Self {
        Self {
            full,
            label: coarse.clone(),
            overview: coarse,
            full_min: 64.0,
            label_min: 28.0,
        }
    }
}
pub(super) fn face(
    meshes: &mut Assets<Mesh>,
    kind: VisibleNodeKind,
    content: &TileSurfaceContent,
    size: Vec2,
    selected: bool,
    focused: bool,
    theme: &MusaicUiTheme,
) -> (Mesh3d, DetailMeshes) {
    let full = meshes.add(compact_tile_mesh(
        kind, content, size, selected, focused, theme,
    ));
    let label = meshes.add(tile_surface::simplified_tile_mesh(
        kind, content, size, selected, focused, theme, true,
    ));
    let overview = meshes.add(tile_surface::simplified_tile_mesh(
        kind, content, size, selected, focused, theme, false,
    ));
    let (full_min, label_min) = tile_surface::detail_thresholds(kind, content, size);
    (
        Mesh3d(full.clone()),
        DetailMeshes {
            full,
            label,
            overview,
            full_min,
            label_min,
        },
    )
}
#[derive(Component)]
pub(crate) struct DetailLegend;
pub(crate) const FULL_LEGEND: &str = "Stacked tiles     [ ]  container     ×  pattern speed";
pub(super) fn plugin(app: &mut App) {
    app.register_type::<BoardDetailLevel>().add_systems(
        Update,
        sync.after(CameraRigSet::Smooth)
            .after(super::scene_reconcile::sync_board_3d_scene)
            .after(super::placement_ghost::sync_drag_preview)
            .run_if(in_state(AppState::Editor)),
    );
}
/// The app's orthographic board projects the same plane scale at every tile.
/// UI logical pixels account for Retina and render-texture scaling once.
fn pixels_per_cell(transform: &Transform, projection: &Projection, size: Vec2) -> Option<f32> {
    let Projection::Orthographic(p) = projection else {
        return None;
    };
    let bevy::camera::ScalingMode::FixedVertical { viewport_height } = p.scaling_mode else {
        return None;
    };
    let height = viewport_height * p.scale;
    if !height.is_finite() || height <= 0.0 || !size.is_finite() || size.min_element() <= 0.0 {
        return None;
    }
    let projected =
        |axis: Vec3| Vec2::new(transform.right().dot(axis), transform.up().dot(axis)).length();
    Some(size.y / height * projected(Vec3::X).min(projected(Vec3::Z)))
}
#[derive(bevy::ecs::system::SystemParam)]
struct Faces<'w, 's> {
    all: Query<'w, 's, (Entity, &'static DetailMeshes)>,
    added: Query<'w, 's, (Entity, &'static DetailMeshes), Added<DetailMeshes>>,
    levels: Query<'w, 's, (&'static mut Mesh3d, &'static mut BoardDetailLevel)>,
}
fn sync(
    camera: Query<(&Transform, &Projection), With<Board3dCamera>>,
    viewport: Query<(Entity, &ComputedNode), With<UiBoardViewport>>,
    mut faces: Faces,
    mut removed: RemovedComponents<DetailMeshes>,
    mut legends: Query<&mut Text, With<DetailLegend>>,
    mut previous: Local<Option<(Entity, f32)>>,
) {
    let Faces { all, added, levels } = &mut faces;
    let Ok((transform, projection)) = camera.single() else {
        return;
    };
    let Ok((entity, node)) = viewport.single() else {
        return;
    };
    let Some(pixels) = pixels_per_cell(
        transform,
        projection,
        node.size() * node.inverse_scale_factor(),
    ) else {
        return;
    };
    let key = (entity, pixels);
    let changed = previous.as_ref() != Some(&key);
    *previous = Some(key);
    let removed = removed.read().count() > 0;
    if !changed && added.is_empty() && !removed && !legends.iter().any(|t| t.0.is_empty()) {
        return;
    }
    let mut switch = |(entity, meshes): (Entity, &DetailMeshes)| {
        if let Ok((mut mesh, mut level)) = levels.get_mut(entity) {
            let next = meshes.next(*level, pixels);
            if next != *level {
                mesh.0 = meshes.handle(next);
                *level = next;
            }
        }
    };
    if changed {
        for item in all.iter() {
            switch(item);
        }
    } else {
        for item in added.iter() {
            switch(item);
        }
    }
    let overview = levels
        .iter()
        .any(|(_, level)| *level == BoardDetailLevel::Overview);
    let simplified = levels
        .iter()
        .any(|(_, level)| *level == BoardDetailLevel::Label);
    let text = if overview {
        "Overview · select a tile for exact values · View → Fit selected tiles"
    } else if simplified {
        "Simplified tiles · select a tile for exact values · View → Fit selected tiles"
    } else {
        FULL_LEGEND
    };
    for mut legend in &mut legends {
        if legend.0 != text {
            legend.0 = text.into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::ScalingMode;
    fn projection(height: f32) -> Projection {
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: height,
            },
            ..OrthographicProjection::default_3d()
        })
    }
    fn overhead() -> Transform {
        Transform::from_xyz(0.0, 20.0, 0.0).looking_at(Vec3::ZERO, Vec3::NEG_Z)
    }
    #[test]
    fn screen_size_uses_logical_viewport_height_and_camera_plane_projection() {
        let pixels =
            pixels_per_cell(&overhead(), &projection(10.0), Vec2::new(800., 600.)).unwrap();
        assert!((pixels - 60.).abs() < 0.001);
        let pixels =
            pixels_per_cell(&overhead(), &projection(20.0), Vec2::new(800., 600.)).unwrap();
        assert!((pixels - 30.).abs() < 0.001);
        let tilted = Transform::from_xyz(0., 20., 20.).looking_at(Vec3::ZERO, Vec3::Y);
        let pixels = pixels_per_cell(&tilted, &projection(10.), Vec2::new(800., 600.)).unwrap();
        assert!((pixels - 60.0 / 2.0_f32.sqrt()).abs() < 0.001);
        assert!(pixels_per_cell(&overhead(), &projection(0.), Vec2::ONE).is_none());
    }
    #[test]
    fn zoom_reuses_meshes_and_entities_with_hysteresis_and_applies_to_new_faces() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<Assets<Mesh>>()
            .add_systems(Update, sync);
        let camera = app
            .world_mut()
            .spawn((Board3dCamera, overhead(), projection(5.)))
            .id();
        let viewport = app
            .world_mut()
            .spawn((
                UiBoardViewport,
                ComputedNode {
                    size: Vec2::new(1600., 1200.),
                    inverse_scale_factor: 0.5,
                    ..default()
                },
            ))
            .id();
        app.world_mut().spawn((DetailLegend, Text::new("")));
        let content = TileSurfaceContent::Compound {
            display: "C#4".into(),
            layers: 7,
            parts: vec![
                "C".into(),
                "#".into(),
                "4".into(),
                "@".into(),
                "2".into(),
                "×".into(),
                "3".into(),
            ],
        };
        let (mesh, details) = face(
            &mut app.world_mut().resource_mut::<Assets<Mesh>>(),
            VisibleNodeKind::Atom,
            &content,
            Vec2::ONE,
            true,
            false,
            &MusaicUiTheme::default(),
        );
        let expected = [details.full.id(), details.label.id(), details.overview.id()];
        let entity = app.world_mut().spawn((mesh, details)).id();
        let count = app.world().resource::<Assets<Mesh>>().len();
        for (height, level, index) in [
            (5., BoardDetailLevel::Full, 0),
            (12., BoardDetailLevel::Label, 1),
            (10., BoardDetailLevel::Label, 1),
            (8., BoardDetailLevel::Full, 0),
            (30., BoardDetailLevel::Overview, 2),
            (20., BoardDetailLevel::Overview, 2),
            (15., BoardDetailLevel::Label, 1),
            (5., BoardDetailLevel::Full, 0),
        ] {
            *app.world_mut().get_mut::<Projection>(camera).unwrap() = projection(height);
            app.update();
            assert_eq!(
                *app.world().get::<BoardDetailLevel>(entity).unwrap(),
                level,
                "height {height}"
            );
            assert_eq!(
                app.world().get::<Mesh3d>(entity).unwrap().0.id(),
                expected[index]
            );
            assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count);
        }
        *app.world_mut().get_mut::<Projection>(camera).unwrap() = projection(30.);
        app.update();
        let (mesh, details) = face(
            &mut app.world_mut().resource_mut::<Assets<Mesh>>(),
            VisibleNodeKind::Atom,
            &content,
            Vec2::ONE,
            false,
            false,
            &MusaicUiTheme::default(),
        );
        let new = app.world_mut().spawn((mesh, details)).id();
        app.update();
        assert_eq!(
            *app.world().get::<BoardDetailLevel>(new).unwrap(),
            BoardDetailLevel::Overview
        );
        app.world_mut()
            .get_mut::<ComputedNode>(viewport)
            .unwrap()
            .size
            .y = 6000.;
        app.update();
        assert_eq!(
            *app.world().get::<BoardDetailLevel>(new).unwrap(),
            BoardDetailLevel::Full
        );
        for _ in 0..3 {
            app.update();
        }
        assert_eq!(app.world().resource::<Assets<Mesh>>().len(), count + 3);
    }
    #[test]
    fn long_values_need_more_pixels_and_each_mesh_keeps_the_same_hit_rectangle() {
        use bevy::mesh::VertexAttributeValues;
        let short = TileSurfaceContent::Scalar {
            display: "2".into(),
        };
        let long = TileSurfaceContent::Scalar {
            display: "123456789012".into(),
        };
        assert!(
            tile_surface::detail_thresholds(VisibleNodeKind::Atom, &long, Vec2::ONE).1
                > tile_surface::detail_thresholds(VisibleNodeKind::Atom, &short, Vec2::ONE).1
        );
        let content = TileSurfaceContent::Container {
            kind: crate::domain::document::ContainerKind::Sequence,
            children: vec![],
            child_count: 123,
        };
        let mut meshes = Assets::default();
        let (_, details) = face(
            &mut meshes,
            VisibleNodeKind::Container,
            &content,
            Vec2::new(5., 1.),
            true,
            false,
            &MusaicUiTheme::default(),
        );
        for handle in [&details.full, &details.label, &details.overview] {
            let VertexAttributeValues::Float32x3(positions) = meshes
                .get(handle)
                .unwrap()
                .attribute(Mesh::ATTRIBUTE_POSITION)
                .unwrap()
            else {
                panic!("positions")
            };
            assert_eq!(
                &positions[..4],
                &[
                    [-2.5, 0.035, -0.5],
                    [2.5, 0.035, -0.5],
                    [2.5, 0.035, 0.5],
                    [-2.5, 0.035, 0.5]
                ]
            );
        }
    }
}
