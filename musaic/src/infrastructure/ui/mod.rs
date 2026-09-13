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
mod audio_meter;
pub mod board;
mod board_camera_nav;
pub mod board_geometry;
mod board_rulers;
pub mod camera_rig;
mod command_search;
mod connection_search;
pub mod controls;
mod diagnostics_panel;
mod first_loop;
pub mod inspector;
mod keyboard_help;
mod keyboard_regions;
mod linked_uses;
pub mod minimap;
pub mod minimap_view;
pub mod musaic_tile;
mod note_entry;
pub mod placement_preview;
pub mod port_glyphs;
pub mod render_layers;
pub mod screens;
pub mod shell;
mod shortcut_settings;
pub mod theme;
mod tile_glyphs;
pub mod tile_icons;
pub mod tile_mesh;
pub mod tile_surface;
pub mod tile_visual;
pub mod timeline_view;
pub mod transform_tile;
pub mod ui_sprites;
pub mod widgets;

// Shared UI items re-exported for sibling modules (`super::…` imports).
pub use inspector::panels::drawer::DrawerTileSource;
pub(crate) use inspector::panels::drawer::{on_drawer_tile_press, on_drawer_tile_release};
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
        #[cfg(not(target_arch = "wasm32"))]
        shell::export_dialog::register(app);
        shell::dropdown::register(app);
        app.add_plugins((
            board_rulers::plugin,
            keyboard_help::plugin,
            keyboard_regions::plugin,
            command_search::plugin,
            connection_search::plugin,
            note_entry::plugin,
            asset_hydration::EditorSpriteHydrationPlugin,
            Board3dPlugin,
            camera_rig::CameraRigPlugin,
            BoardCameraNavPlugin,
            MusaicUiControlsPlugin,
            screens::MainMenuPlugin,
            screens::LoadingUiPlugin,
        ))
        .add_plugins((
            first_loop::plugin,
            audio_meter::plugin,
            shortcut_settings::plugin,
            linked_uses::plugin,
            inspector::view_memory::plugin,
        ))
        .init_resource::<InspectorPanelHost>()
        .init_resource::<InspectorTransitionQueue>()
        .init_resource::<minimap_view::MinimapContentCache>()
        .init_resource::<timeline_view::TimelineContentCache>()
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
                    inspector::panels::tile_inspect::sync_atom_value_controls,
                    inspector::panels::tile_inspect::sync_exact_atoms,
                    inspector::panels::effect_tiles::sync,
                    inspector::panels::effect_tiles::sync_exact,
                    inspector::panels::modulation_tiles::sync,
                    inspector::panels::sound::sync_sound_controls,
                    inspector::panels::sample_options::sync_sample_options_controls,
                    inspector::panels::sample_options::sync_bank_tile_choices,
                    drive_inspector_slides,
                    minimap_view::sync_minimap_content,
                    timeline_view::sync_timeline_content,
                )
                    .chain(),
                (
                    timeline_view::sync_timeline_playhead,
                    diagnostics_panel::sync_diagnostics_banner,
                    diagnostics_panel::sync_playback_status,
                    controls::sync_ui_tile_palette_scene,
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
