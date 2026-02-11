//! Editor state machine for project lifecycle and interaction readiness.

use bevy::prelude::*;

use crate::editor::menus::Menu;
use crate::editor::screens::editor::InEditor;

pub(super) fn plugin(app: &mut App) {
    app.add_sub_state::<ProjectSession>();
    app.add_computed_state::<EditorReady>();
    app.add_computed_state::<EditorInteractive>();
    app.add_systems(OnEnter(InEditor), open_project_hub_if_needed);
    app.add_systems(
        Update,
        open_project_hub_if_needed.run_if(in_state(ProjectSession::NoProject)),
    );
}

#[derive(SubStates, Clone, PartialEq, Eq, Hash, Debug, Default)]
#[source(InEditor = InEditor)]
pub enum ProjectSession {
    #[default]
    NoProject,
    Loading,
    Loaded,
    Error,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EditorReady;

impl ComputedStates for EditorReady {
    type SourceStates = (Option<InEditor>, Option<ProjectSession>);

    fn compute((in_editor, session): (Option<InEditor>, Option<ProjectSession>)) -> Option<Self> {
        (in_editor.is_some() && matches!(session, Some(ProjectSession::Loaded))).then_some(Self)
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct EditorInteractive;

impl ComputedStates for EditorInteractive {
    type SourceStates = (Option<EditorReady>, Menu);

    fn compute((ready, menu): (Option<EditorReady>, Menu)) -> Option<Self> {
        (ready.is_some() && menu == Menu::None).then_some(Self)
    }
}

fn open_project_hub_if_needed(menu: Res<State<Menu>>, mut next_menu: ResMut<NextState<Menu>>) {
    if *menu == Menu::None {
        next_menu.set(Menu::ProjectHub);
    }
}
