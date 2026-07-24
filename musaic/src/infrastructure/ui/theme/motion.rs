//! Shared UI motion tokens and inspector slide helpers.

use bevy::prelude::*;

use crate::application::editor::InspectorLayout;

/// Duration / distance tokens for panel motion.
#[derive(Debug, Clone, Copy)]
pub struct MotionTokens {
    pub panel_slide_secs: f32,
    pub panel_slide_distance_fallback: f32,
}

impl MotionTokens {
    pub const fn default_dark() -> Self {
        Self {
            panel_slide_secs: 0.18,
            panel_slide_distance_fallback: 320.0,
        }
    }
}

pub const INSPECTOR_SLIDE_DURATION: f32 = MotionTokens::default_dark().panel_slide_secs;
pub const INSPECTOR_SLIDE_DISTANCE_FALLBACK: f32 =
    MotionTokens::default_dark().panel_slide_distance_fallback;

#[derive(Component)]
pub(crate) struct UiInspectorViewport;

#[derive(Component)]
pub(crate) struct UiInspectorHeader;

#[derive(Component)]
pub(crate) struct UiInspectorBody;

#[derive(Component, Debug)]
pub(crate) struct InspectorSlideLayer {
    pub direction: SlideDirection,
    pub elapsed: f32,
    pub distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SlideDirection {
    Enter,
    Exit,
}

#[derive(Resource, Default)]
pub(crate) struct InspectorPanelHost {
    pub viewport: Option<Entity>,
    pub header: Option<Entity>,
    pub active_body: Option<Entity>,
    pub displayed_layout: Option<InspectorLayout>,
}

#[derive(Resource, Default)]
pub(crate) struct InspectorTransitionQueue {
    pub pending: Option<PendingInspectorTransition>,
}

#[derive(Debug, Clone)]
pub(crate) struct PendingInspectorTransition {
    pub layout: InspectorLayout,
    pub animate: bool,
}

pub(crate) fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t;
    1.0 - inv * inv * inv
}

pub(crate) fn slide_translation_x(direction: SlideDirection, distance: f32, eased: f32) -> f32 {
    match direction {
        SlideDirection::Enter => (-distance) + distance * eased,
        SlideDirection::Exit => distance * eased,
    }
}

pub(crate) fn finish_active_slides(
    commands: &mut Commands,
    slides: &Query<(Entity, &InspectorSlideLayer)>,
    host: &mut InspectorPanelHost,
) {
    let mut incoming: Option<Entity> = None;
    let mut outgoing: Vec<Entity> = Vec::new();

    for (entity, slide) in slides.iter() {
        match slide.direction {
            SlideDirection::Enter => incoming = Some(entity),
            SlideDirection::Exit => outgoing.push(entity),
        }
    }

    for entity in outgoing {
        commands.entity(entity).despawn();
    }

    if let Some(entity) = incoming {
        commands
            .entity(entity)
            .remove::<InspectorSlideLayer>()
            .insert(UiTransform::from_translation(Val2::ZERO));
        host.active_body = Some(entity);
    }
}

pub(crate) fn drive_inspector_slides(
    time: Res<Time>,
    mut commands: Commands,
    mut host: ResMut<InspectorPanelHost>,
    mut slides: Query<(Entity, &mut InspectorSlideLayer, &mut UiTransform)>,
    theme: Res<super::MusaicUiTheme>,
) {
    if slides.is_empty() {
        return;
    }

    let dt = time.delta_secs();
    let duration = theme.motion.panel_slide_secs;
    let mut finished_exits = Vec::new();
    let mut finished_enters = Vec::new();

    for (entity, mut slide, mut transform) in &mut slides {
        slide.elapsed += dt;
        let t = (slide.elapsed / duration).min(1.0);
        let eased = ease_out_cubic(t);
        let x = slide_translation_x(slide.direction, slide.distance, eased);
        transform.translation = Val2::px(x, 0.0);

        if t >= 1.0 {
            match slide.direction {
                SlideDirection::Exit => finished_exits.push(entity),
                SlideDirection::Enter => finished_enters.push(entity),
            }
        }
    }

    for entity in finished_exits {
        if host.active_body == Some(entity) {
            host.active_body = None;
        }
        commands.entity(entity).despawn();
    }

    for entity in finished_enters {
        commands
            .entity(entity)
            .remove::<InspectorSlideLayer>()
            .insert(UiTransform::from_translation(Val2::ZERO));
        host.active_body = Some(entity);
    }
}
