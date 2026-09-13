//! Multi-value effect tiles edit the live owned group, never a stale UI copy.
use crate::{
    application::{
        command::{EditorCommand, EditorCommandBus},
        session::MusaicProject,
    },
    domain::document::{AtomValue, DocumentNodeKind},
    infrastructure::ui::{theme::MusaicUiTheme, widgets::slider},
};
use bevy::prelude::*;
use bevy_feathers::controls::SliderProps;
use bevy_ui_widgets::{SliderPrecision, SliderStep, SliderValue, ValueChange};
use tessera::prelude::{AtomModifier, NodeId, Rational};

#[derive(Clone, Copy)]
enum Operand {
    Amount,
    Time,
    Feedback,
    Damping,
    Decay,
    Threshold,
    Ratio,
    Knee,
    Attack,
    Release,
}
#[derive(Component)]
pub(crate) struct EffectOperand {
    node: NodeId,
    operand: Operand,
}
impl Operand {
    fn read(self, modifier: &AtomModifier) -> Option<Rational> {
        use AtomModifier as M;
        use Operand as P;
        Some(match (modifier, self) {
            (M::Delay(p), P::Amount) => p.amount,
            (M::Delay(p), P::Time) => p.time,
            (M::Delay(p), P::Feedback) => p.feedback,
            (M::Delay(p), P::Damping) => p.damping,
            (M::Reverb(p), P::Amount) => p.amount,
            (M::Reverb(p), P::Decay) => p.decay,
            (M::Reverb(p), P::Damping) => p.damping,
            (M::Compressor(p), P::Threshold) => p.threshold,
            (M::Compressor(p), P::Ratio) => p.ratio,
            (M::Compressor(p), P::Knee) => p.knee_db,
            (M::Compressor(p), P::Attack) => p.attack,
            (M::Compressor(p), P::Release) => p.release,
            _ => return None,
        })
    }
    fn write(self, modifier: &mut AtomModifier, value: Rational) -> bool {
        use AtomModifier as M;
        use Operand as P;
        let target = match (modifier, self) {
            (M::Delay(p), P::Amount) => &mut p.amount,
            (M::Delay(p), P::Time) => &mut p.time,
            (M::Delay(p), P::Feedback) => &mut p.feedback,
            (M::Delay(p), P::Damping) => &mut p.damping,
            (M::Reverb(p), P::Amount) => &mut p.amount,
            (M::Reverb(p), P::Decay) => &mut p.decay,
            (M::Reverb(p), P::Damping) => &mut p.damping,
            (M::Compressor(p), P::Threshold) => &mut p.threshold,
            (M::Compressor(p), P::Ratio) => &mut p.ratio,
            (M::Compressor(p), P::Knee) => &mut p.knee_db,
            (M::Compressor(p), P::Attack) => &mut p.attack,
            (M::Compressor(p), P::Release) => &mut p.release,
            _ => return false,
        };
        *target = value;
        true
    }
}
fn scalar(value: Rational) -> f32 {
    value.numerator as f32 / value.denominator as f32
}
pub(super) fn spawn(parent: &mut ChildSpawnerCommands<'_>, node: &NodeId, modifier: &AtomModifier) {
    use Operand as P;
    let operands: &[(P, &str, f32, f32, f32)] = match modifier {
        AtomModifier::Delay(_) => &[
            (P::Amount, "Delay amount", 0., 1., 0.01),
            (P::Time, "Delay time · seconds", 0.001, 2., 0.001),
            (P::Feedback, "Feedback", 0., 1., 0.01),
            (P::Damping, "Damping", 0., 1., 0.01),
        ],
        AtomModifier::Reverb(_) => &[
            (P::Amount, "Reverb amount", 0., 1., 0.01),
            (P::Decay, "Decay · seconds", 0.001, 60., 0.01),
            (P::Damping, "Damping", 0., 1., 0.01),
        ],
        AtomModifier::Compressor(_) => &[
            (P::Threshold, "Threshold", 0., 1., 0.01),
            (P::Ratio, "Compression ratio", 1., 20., 0.1),
            (P::Knee, "Knee · dB", 0., 40., 0.1),
            (P::Attack, "Attack · seconds", 0.001, 60., 0.001),
            (P::Release, "Release · seconds", 0.001, 60., 0.001),
        ],
        _ => return,
    };
    for (operand, label, min, max, step) in operands {
        let theme = MusaicUiTheme::default_dark();
        parent.spawn((
            Text::new(*label),
            TextFont {
                font_size: 12.,
                ..default()
            },
            TextColor(theme.chrome.text_main),
        ));
        parent
            .spawn(slider(
                SliderProps {
                    value: scalar(operand.read(modifier).unwrap()),
                    min: *min,
                    max: *max,
                },
                (
                    crate::infrastructure::ui::widgets::ExactSlider,
                    EffectOperand {
                        node: node.clone(),
                        operand: *operand,
                    },
                    SliderStep(*step),
                    SliderPrecision(3),
                ),
            ))
            .with_children(|slider| {
                let field = crate::infrastructure::ui::widgets::exact_number::spawn_inline(
                    slider,
                    operand.read(modifier).unwrap(),
                    EffectOperand {
                        node: node.clone(),
                        operand: *operand,
                    },
                );
                slider.commands().entity(field).observe(exact_change);
            })
            .observe(change)
            .observe(start)
            .observe(end);
    }
}
fn change(
    event: On<ValueChange<f32>>,
    fields: Query<&EffectOperand>,
    project: Res<MusaicProject>,
    mut commands: Commands,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(field) = fields.get(event.source) else {
        return;
    };
    let Some(DocumentNodeKind::Atom(atom)) = project
        .document
        .graph
        .node(&field.node)
        .map(|node| &node.kind)
    else {
        return;
    };
    let AtomValue::Modifier(mut modifier) = atom.atom.clone() else {
        return;
    };
    if !event.value.is_finite() {
        return;
    }
    let value = Rational::new(
        (f64::from(event.value) * 1_000_000.).round() as i64,
        1_000_000,
    );
    if !field.operand.write(&mut modifier, value) {
        return;
    }
    let Some(owned) = modifier.parameter_value() else {
        return;
    };
    if modifier.with_parameter_value(owned).is_err() {
        return;
    }
    commands
        .entity(event.source)
        .insert(SliderValue(event.value));
    bus.write(EditorCommandBus(EditorCommand::SetAtomValue {
        node: field.node.clone(),
        value: AtomValue::Modifier(modifier),
    }));
}
fn start(
    event: On<Pointer<DragStart>>,
    fields: Query<&EffectOperand>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    if let Ok(field) = fields.get(event.entity) {
        bus.write(EditorCommandBus(EditorCommand::BeginAtomEdit {
            node: field.node.clone(),
        }));
    }
}
fn end(_: On<Pointer<DragEnd>>, mut bus: MessageWriter<EditorCommandBus>) {
    bus.write(EditorCommandBus(EditorCommand::EndAtomEdit));
}
pub(crate) fn sync(
    project: Res<MusaicProject>,
    fields: Query<(Entity, &EffectOperand, &SliderValue)>,
    mut commands: Commands,
) {
    if !project.is_changed() {
        return;
    }
    for (entity, field, slider) in &fields {
        if let Some(DocumentNodeKind::Atom(atom)) = project
            .document
            .graph
            .node(&field.node)
            .map(|node| &node.kind)
        {
            if let AtomValue::Modifier(modifier) = &atom.atom {
                if let Some(value) = field.operand.read(modifier) {
                    let value = scalar(value);
                    if slider.0 != value {
                        commands.entity(entity).insert(SliderValue(value));
                    }
                }
            }
        }
    }
}

fn exact_change(
    event: On<ValueChange<Rational>>,
    fields: Query<&EffectOperand>,
    project: Res<MusaicProject>,
    mut text: Query<&mut Text, With<crate::infrastructure::ui::widgets::exact_number::ExactNumber>>,
    mut focus: ResMut<bevy::input_focus::InputFocus>,
    mut bus: MessageWriter<EditorCommandBus>,
) {
    let Ok(field) = fields.get(event.source) else {
        return;
    };
    let Some(DocumentNodeKind::Atom(atom)) =
        project.document.graph.node(&field.node).map(|n| &n.kind)
    else {
        return;
    };
    let AtomValue::Modifier(mut modifier) = atom.atom.clone() else {
        return;
    };
    if !field.operand.write(&mut modifier, event.value) {
        return;
    }
    let Some(value) = modifier.parameter_value() else {
        return;
    };
    match modifier.with_parameter_value(value) {
        Ok(modifier) => {
            bus.write(EditorCommandBus(EditorCommand::SetAtomValue {
                node: field.node.clone(),
                value: AtomValue::Modifier(modifier),
            }));
        }
        Err(error) => {
            if let Ok(mut text) = text.get_mut(event.source) {
                crate::infrastructure::ui::widgets::exact_number::reject(
                    &mut text,
                    &mut focus,
                    event.source,
                    error,
                );
            }
        }
    }
}
pub(crate) fn sync_exact(
    project: Res<MusaicProject>,
    focus: Res<bevy::input_focus::InputFocus>,
    mut fields: Query<(
        Entity,
        &EffectOperand,
        &mut crate::infrastructure::ui::widgets::exact_number::ExactNumber,
        &mut Text,
    )>,
) {
    for (entity, binding, mut field, mut text) in &mut fields {
        if let Some(DocumentNodeKind::Atom(atom)) =
            project.document.graph.node(&binding.node).map(|n| &n.kind)
        {
            if let AtomValue::Modifier(modifier) = &atom.atom {
                if let Some(value) = binding.operand.read(modifier) {
                    crate::infrastructure::ui::widgets::exact_number::update(
                        &mut field,
                        &mut text,
                        value,
                        focus.0 == Some(entity),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{editor::EditorPlugin, history::CommandHistory, pipeline::PlaybackPlugin},
        domain::{
            board::BoardSlot,
            document::{ContainerKind, PlacementAddress, StackIndex, TileSpawnKind},
        },
        infrastructure::app::{AppState, TransportMode},
    };
    fn app() -> (App, NodeId, Entity, Entity) {
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>()
            .init_state::<TransportMode>()
            .add_plugins(MinimalPlugins)
            .add_plugins((EditorPlugin, PlaybackPlugin))
            .init_resource::<bevy::input_focus::InputFocus>();
        app.insert_state(AppState::Editor);
        let mut project = MusaicProject::new_empty();
        let root = project.document.root_surface;
        let container = project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                root,
                PlacementAddress::BoardSlot(BoardSlot::new(0, 0)),
                TileSpawnKind::Container {
                    kind: ContainerKind::Sequence,
                },
            )
            .unwrap();
        let surface = project
            .document
            .graph
            .container_surface(&container)
            .unwrap();
        let node = project
            .document
            .graph
            .insert_tile(
                &mut project.document.surfaces,
                surface,
                PlacementAddress::StackIndex(StackIndex(0)),
                TileSpawnKind::Atom {
                    atom: AtomValue::Modifier(AtomModifier::Delay(Default::default())),
                },
            )
            .unwrap();
        app.insert_resource(project);
        let amount = app
            .world_mut()
            .spawn((
                EffectOperand {
                    node: node.clone(),
                    operand: Operand::Amount,
                },
                SliderValue(0.25),
            ))
            .observe(change)
            .id();
        let time = app
            .world_mut()
            .spawn((
                EffectOperand {
                    node: node.clone(),
                    operand: Operand::Time,
                },
                SliderValue(0.25),
            ))
            .observe(change)
            .observe(exact_change)
            .id();
        (app, node, amount, time)
    }
    fn delay(app: &App, node: &NodeId) -> tessera::prelude::DelayParameters {
        let DocumentNodeKind::Atom(atom) = &app
            .world()
            .resource::<MusaicProject>()
            .document
            .graph
            .node(node)
            .unwrap()
            .kind
        else {
            panic!()
        };
        let AtomValue::Modifier(AtomModifier::Delay(value)) = atom.atom else {
            panic!()
        };
        value
    }
    #[test]
    fn editing_two_operands_retains_latest_values_and_one_drag_is_one_undo() {
        let (mut app, node, amount, time) = app();
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::BeginAtomEdit {
                node: node.clone(),
            }));
        app.update();
        for value in [0.4_f32, 0.6, 0.8] {
            app.world_mut().trigger(ValueChange {
                source: amount,
                value,
            });
            app.update();
        }
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::EndAtomEdit));
        app.update();
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 1);
        app.world_mut().trigger(ValueChange {
            source: time,
            value: Rational::new(1, 3),
        });
        app.update();
        assert_eq!(delay(&app, &node).amount, Rational::new(4, 5));
        assert_eq!(delay(&app, &node).time, Rational::new(1, 3));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::Undo));
        app.update();
        assert_eq!(delay(&app, &node).time, Rational::new(1, 4));
        assert_eq!(delay(&app, &node).amount, Rational::new(4, 5));
        app.world_mut()
            .write_message(EditorCommandBus(EditorCommand::Undo));
        app.update();
        assert_eq!(delay(&app, &node), Default::default());
    }
    #[test]
    fn invalid_effect_time_cannot_publish_or_add_history() {
        let (mut app, node, _, time) = app();
        let before = delay(&app, &node);
        app.world_mut().trigger(ValueChange {
            source: time,
            value: 3.0f32,
        });
        app.update();
        assert_eq!(delay(&app, &node), before);
        assert_eq!(app.world().resource::<CommandHistory>().undo_len(), 0);
    }
}
