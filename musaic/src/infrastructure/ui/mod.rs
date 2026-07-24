//! Editor UI: shell chrome, inspector panel stack, and 3D viewports.
//!
//! Structure:
//! - [`theme`] — design tokens + Feathers mapping + shared motion
//! - [`widgets`] — button, panel, dialog primitives
//! - [`screens`] — MainMenu, Loading, UnsavedDialog
//! - [`shell`] — root layout, in-editor top chrome, breadcrumbs, rebuild driver
//! - [`inspector`] — right-rail panel stack projected from `InspectorLayout`
//! - [`board`] / [`controls`] — 3D board and tile palette viewports

use bevy::{prelude::*, state::condition::in_state};

use crate::infrastructure::app::MusaicSet;

pub mod asset_hydration;
pub mod board;
mod board_camera_nav;
pub mod board_geometry;
pub mod camera_rig;
pub mod controls;
mod diagnostics_panel;
pub mod inspector;
pub mod minimap;
pub mod minimap_view;
pub mod musaic_tile;
pub mod placement_preview;
pub mod port_glyphs;
pub mod render_layers;
pub mod screens;
pub mod shell;
pub mod theme;
pub mod tile_icons;
pub mod tile_mesh;
pub mod tile_surface;
pub mod tile_visual;
pub mod timeline_view;
pub mod transform_tile;
pub mod ui_sprites;
pub mod widgets;

/// Backward-compatible re-exports (ArtistPalette folded into theme).
pub use theme::{ArtistPalette, atom_value_color};

// Shared UI items re-exported for sibling modules (`super::…` imports).
pub(crate) use inspector::panels::drawer::{
    DrawerTileSource, on_drawer_tile_press, on_drawer_tile_release,
};
pub(crate) use shell::layout::{
    on_minimap_edge_drag, on_minimap_edge_drag_end, on_minimap_edge_drag_start,
    on_timeline_edge_drag, on_timeline_edge_drag_end, on_timeline_edge_drag_start,
};
pub(crate) use shell::menu::{InspectorButtonAction, on_inspector_button_activated};

use board::Board3dPlugin;
use board_camera_nav::BoardCameraNavPlugin;
use controls::MusaicUiControlsPlugin;
use theme::{InspectorPanelHost, InspectorTransitionQueue, drive_inspector_slides};

pub struct MusaicUiPlugin;

impl Plugin for MusaicUiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            asset_hydration::EditorSpriteHydrationPlugin,
            Board3dPlugin,
            camera_rig::CameraRigPlugin,
            tile_surface::TileSurfacePlugin,
            BoardCameraNavPlugin,
            MusaicUiControlsPlugin,
            screens::MainMenuPlugin,
            screens::LoadingUiPlugin,
            crate::infrastructure::diagnostics::HierarchyAuditPlugin,
        ))
        .init_resource::<InspectorPanelHost>()
        .init_resource::<InspectorTransitionQueue>()
        .init_resource::<minimap_view::MinimapContentCache>()
        .init_resource::<timeline_view::TimelineContentCache>()
        .init_resource::<timeline_view::TimelinePanState>()
        .add_systems(
            OnEnter(crate::infrastructure::app::AppState::Editor),
            diagnostics_panel::spawn_diagnostics_banner,
        )
        .add_systems(
            OnExit(crate::infrastructure::app::AppState::Editor),
            shell::layout::teardown_editor_ui,
        )
        .add_observer(on_drawer_tile_press)
        .add_observer(on_drawer_tile_release)
        .add_observer(inspector::on_view_settings_cycle_activated)
        .add_systems(
            Update,
            (
                (
                    shell::layout::sync_shell_panel_sizes,
                    shell::rebuild::rebuild_ui,
                    inspector::process_inspector_transitions,
                    drive_inspector_slides,
                    minimap_view::sync_minimap_content,
                    timeline_view::sync_timeline_content,
                )
                    .chain(),
                (
                    timeline_view::sync_timeline_pan_offset,
                    timeline_view::sync_timeline_playhead,
                    diagnostics_panel::sync_diagnostics_banner,
                    controls::sync_ui_tile_palette_scene,
                    controls::frame_ui_tile_palette_camera,
                    shell::menu::sync_menu_transport_label,
                )
                    .chain(),
            )
                .chain()
                .in_set(MusaicSet::RenderUi)
                .run_if(in_state(crate::infrastructure::app::AppState::Editor)),
        );
    }
}
