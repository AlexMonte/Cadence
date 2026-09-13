//! Thin track and a visible thumb around Bevy's existing accessible slider.
//! Value changes still pass through the editor's validation and undo observers.

use bevy::{input_focus::tab_navigation::TabIndex, picking::prelude::*, prelude::*};
use bevy_feathers::controls::SliderProps;
use bevy_ui_widgets::{Slider, SliderPrecision, SliderRange, SliderThumb, SliderValue, TrackClick};
use tessera::prelude::Rational;

use super::exact_number::{self, ExactNumber};

#[derive(Component)]
struct SliderValueLabel(Entity);
/// A caller supplies its own exact field and domain validation inside this slider.
#[derive(Component)]
pub(crate) struct ExactSlider;
#[derive(Component)]
struct SliderThumbVisual;

pub struct MusaicSliderPlugin;
impl Plugin for MusaicSliderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, spawn_slider_values);
        app.add_systems(
            PostUpdate,
            sync_slider_visuals.before(bevy::ui::UiSystems::Layout),
        );
    }
}

pub fn slider<B: Bundle>(props: SliderProps, overrides: B) -> impl Bundle {
    let theme = super::super::theme::MusaicUiTheme::default();
    (
        Node {
            height: px(38),
            min_width: px(0),
            flex_grow: 1.0,
            flex_shrink: 0.0,
            position_type: PositionType::Relative,
            ..default()
        },
        Slider {
            track_click: TrackClick::Snap,
        },
        SliderValue(props.value),
        SliderRange::new(props.min, props.max),
        TabIndex(0),
        overrides,
        children![
            (
                Node {
                    position_type: PositionType::Absolute,
                    left: px(8),
                    right: px(8),
                    top: px(26),
                    height: px(3),
                    border_radius: BorderRadius::all(px(2)),
                    ..default()
                },
                BackgroundColor(theme.chrome.border),
                Pickable::IGNORE,
            ),
            (
                SliderThumbVisual,
                SliderThumb,
                Node {
                    position_type: PositionType::Absolute,
                    width: px(16),
                    height: px(16),
                    top: px(19),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(8)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                BorderColor::all(theme.chrome.accent),
                Pickable::default(),
            ),
        ],
    )
}

fn spawn_slider_values(
    mut commands: Commands,
    sliders: Query<(Entity, &SliderValue), (Added<Slider>, Without<ExactSlider>)>,
) {
    for (entity, value) in &sliders {
        commands.entity(entity).with_children(|parent| {
            let field = exact_number::spawn_inline(
                parent,
                Rational::new((f64::from(value.0) * 1_000_000.0).round() as i64, 1_000_000),
                SliderValueLabel(entity),
            );
            parent.commands().entity(field).observe(change_typed_value);
        });
    }
}

fn change_typed_value(
    event: On<bevy_ui_widgets::ValueChange<Rational>>,
    bindings: Query<&SliderValueLabel>,
    sliders: Query<&SliderRange>,
    mut fields: Query<&mut Text, With<ExactNumber>>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut commands: Commands,
) {
    let Ok(binding) = bindings.get(event.source) else {
        return;
    };
    let Ok(range) = sliders.get(binding.0) else {
        return;
    };
    let value = event.value.numerator as f64 / event.value.denominator as f64;
    if !value.is_finite() || value < f64::from(range.start()) || value > f64::from(range.end()) {
        if let Ok(mut text) = fields.get_mut(event.source) {
            exact_number::reject(&mut text, &mut focus, event.source, "Outside slider range");
        }
    } else {
        commands.trigger(bevy_ui_widgets::ValueChange {
            source: binding.0,
            value: value as f32,
        });
    }
}

fn sync_slider_visuals(
    focus: Res<bevy::input_focus::InputFocus>,
    values: Query<(&SliderValue, &SliderRange, Option<&SliderPrecision>)>,
    mut thumbs: Query<(&ChildOf, &mut Node), With<SliderThumbVisual>>,
    mut labels: Query<(Entity, &SliderValueLabel, &mut ExactNumber, &mut Text)>,
) {
    for (parent, mut thumb) in &mut thumbs {
        let Ok((value, range, _)) = values.get(parent.parent()) else {
            continue;
        };
        let ratio = range.thumb_position(value.0).clamp(0.0, 1.0);
        let left = percent(ratio * 100.0);
        let margin = px(-16.0 * ratio);
        if thumb.left != left || thumb.margin.left != margin {
            thumb.left = left;
            thumb.margin.left = margin;
        }
    }
    for (entity, binding, mut field, mut label) in &mut labels {
        let Ok((value, _, precision)) = values.get(binding.0) else {
            continue;
        };
        let digits = precision.map_or(2, |p| p.0.clamp(0, 4)) as usize;
        let text = format!("{:.*}", digits, value.0);
        exact_number::update(
            &mut field,
            &mut label,
            Rational::new((f64::from(value.0) * 1_000_000.0).round() as i64, 1_000_000),
            focus.0 == Some(entity),
        );
        if focus.0 != Some(entity) && label.0 != text {
            label.0 = text;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    };
    use bevy::input_focus::InputFocus;

    #[derive(Resource, Default)]
    struct Accepted(Vec<(Entity, f32)>);

    fn accept(
        event: On<bevy_ui_widgets::ValueChange<f32>>,
        mut values: ResMut<Accepted>,
        mut commands: Commands,
    ) {
        values.0.push((event.source, event.value));
        commands
            .entity(event.source)
            .insert(SliderValue(event.value));
    }

    fn fixture() -> (App, Entity, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::input::InputPlugin,
            bevy::input_focus::InputDispatchPlugin,
            MusaicSliderPlugin,
        ))
        .init_resource::<Accepted>();
        app.configure_sets(
            PreUpdate,
            bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
        );
        let window = app.world_mut().spawn(bevy::window::PrimaryWindow).id();
        let slider = app
            .world_mut()
            .spawn(slider(
                SliderProps {
                    value: 2.0,
                    min: -16.0,
                    max: 16.0,
                },
                SliderPrecision(2),
            ))
            .observe(accept)
            .id();
        app.update();
        app.update();
        let field = app
            .world_mut()
            .query_filtered::<Entity, With<SliderValueLabel>>()
            .single(app.world())
            .unwrap();
        (app, window, slider, field)
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

    fn edit(app: &mut App, window: Entity, field: Entity, text: &str) {
        use bevy::camera::NormalizedRenderTarget;
        use bevy::picking::{
            backend::HitData,
            pointer::{Location, PointerButton, PointerId},
        };
        app.world_mut().trigger(Pointer::new(
            PointerId::Mouse,
            Location {
                target: NormalizedRenderTarget::Window(
                    bevy::window::WindowRef::Entity(window)
                        .normalize(None)
                        .unwrap(),
                ),
                position: Vec2::ZERO,
            },
            Press {
                button: PointerButton::Primary,
                hit: HitData::new(window, 0.0, None, None),
            },
            field,
        ));
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
        key(app, window, KeyCode::Digit1, Some(text));
        key(app, window, KeyCode::Enter, None);
    }

    #[test]
    fn clicking_the_value_types_into_the_existing_slider_binding() {
        let (mut app, window, slider, field) = fixture();
        edit(&mut app, window, field, "-1/2");
        assert_eq!(app.world().resource::<Accepted>().0, [(slider, -0.5)]);
        assert_eq!(app.world().get::<SliderValue>(slider).unwrap().0, -0.5);
        assert_eq!(app.world().resource::<InputFocus>().0, None);
        edit(&mut app, window, field, "7.125");
        assert_eq!(app.world().get::<SliderValue>(slider).unwrap().0, 7.125);
    }

    #[test]
    fn invalid_typed_values_stay_editable_and_escape_restores_the_value() {
        let (mut app, window, slider, field) = fixture();
        for draft in ["1/0", "NaN", "17"] {
            edit(&mut app, window, field, draft);
            assert!(app.world().resource::<Accepted>().0.is_empty());
            assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
            key(&mut app, window, KeyCode::Escape, None);
            assert_eq!(app.world().resource::<InputFocus>().0, None);
            assert_eq!(app.world().get::<SliderValue>(slider).unwrap().0, 2.0);
        }
    }
}
