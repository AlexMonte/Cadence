use super::*;
use crate::application::editor::{DrawerTilePressQueue, basic_tile_options};
use bevy::{
    camera::{ComputedCameraValues, NormalizedRenderTarget, RenderTargetInfo, Viewport},
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::MouseButtonInput,
    },
    input_focus::InputFocus,
    picking::{
        backend::HitData,
        pointer::{Location, PointerButton, PointerId},
    },
    window::{PrimaryWindow, WindowRef},
};

fn fixture() -> (App, Entity) {
    let (mut app, window) = fixture_unstarted();
    app.update();
    app.update();
    (app, window)
}

fn fixture_unstarted() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        bevy::input::InputPlugin,
        bevy::input_focus::InputDispatchPlugin,
    ))
    .init_resource::<TileLibrarySearch>()
    .init_resource::<crate::application::editor::preferences::EditorPreferences>()
    .init_resource::<crate::application::editor::interaction::keyboard_navigation::KeyboardNavigation>()
    .init_resource::<UiTilePaletteDisplay>()
    .init_resource::<DrawerTilePressQueue>()
    .init_resource::<EditorUiProjection>();
    app.add_message::<crate::application::command::EditorCommandBus>();
    app.configure_sets(
        PreUpdate,
        bevy::input_focus::InputFocusSystems::Dispatch.after(bevy::input::InputSystems),
    );
    app.world_mut()
        .resource_mut::<EditorUiProjection>()
        .palette
        .context = TileLibraryContextKind::ContainerBody;
    app.world_mut()
        .resource_mut::<EditorUiProjection>()
        .palette
        .options = basic_tile_options(TileLibraryContextKind::ContainerBody);
    app.add_systems(
        Update,
        (
            sync_ui_tile_palette_scene,
            super::super::library_search::sync,
        )
            .chain(),
    );
    // The same Bevy layout path as the native UI, with a synthetic render target
    // so reachability and fixed tile sizes can be checked without a GPU/window.
    app.add_plugins(bevy::app::HierarchyPropagatePlugin::<ComputedUiTargetCamera>::new(PostUpdate));
    app.add_plugins(bevy::app::HierarchyPropagatePlugin::<
        ComputedUiRenderTargetInfo,
    >::new(PostUpdate));
    app.init_resource::<UiScale>()
        .init_resource::<bevy::ui::ui_surface::UiSurface>()
        .init_resource::<bevy::text::TextPipeline>()
        .init_resource::<bevy::text::CosmicFontSystem>()
        .init_resource::<bevy::text::SwashCache>()
        .init_resource::<bevy::transform::StaticTransformOptimizations>();
    app.add_systems(
        PostUpdate,
        (
            bevy::ui::update::propagate_ui_target_cameras,
            bevy::ui::ui_layout_system,
            sync_scrollbars,
        )
            .chain(),
    );
    app.configure_sets(
        PostUpdate,
        bevy::app::PropagateSet::<ComputedUiTargetCamera>::default()
            .after(bevy::ui::update::propagate_ui_target_cameras)
            .before(bevy::ui::ui_layout_system),
    );
    app.configure_sets(
        PostUpdate,
        bevy::app::PropagateSet::<ComputedUiRenderTargetInfo>::default()
            .after(bevy::ui::update::propagate_ui_target_cameras)
            .before(bevy::ui::ui_layout_system),
    );
    let window = app
        .world_mut()
        .spawn((Window::default(), PrimaryWindow))
        .id();
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo {
                    physical_size: UVec2::new(260, 550),
                    scale_factor: 1.0,
                }),
                ..default()
            },
            viewport: Some(Viewport {
                physical_size: UVec2::new(260, 550),
                ..default()
            }),
            ..default()
        },
    ));
    app.world_mut()
        .commands()
        .spawn(Node {
            width: px(260),
            height: px(550),
            flex_direction: FlexDirection::Column,
            ..default()
        })
        .with_children(|parent| {
            crate::infrastructure::ui::inspector::panels::drawer::spawn_tile_drawer(
                parent, None, None,
            )
        });
    app.world_mut().flush();
    (app, window)
}
fn pointer<E: std::fmt::Debug + Clone + Reflect>(
    window: Entity,
    target: Entity,
    position: Vec2,
    event: E,
) -> Pointer<E> {
    Pointer::new(
        PointerId::Mouse,
        Location {
            target: NormalizedRenderTarget::Window(
                WindowRef::Entity(window).normalize(None).unwrap(),
            ),
            position,
        },
        event,
        target,
    )
}
fn hit(window: Entity) -> HitData {
    HitData::new(window, 0.0, None, None)
}
fn body(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<TileLibraryBody>>()
        .single(app.world())
        .unwrap()
}
fn source(app: &mut App, tile: &TileSpawnKind) -> Entity {
    app.world_mut()
        .query::<(Entity, &DrawerTileSource)>()
        .iter(app.world())
        .find(|(_, source)| &source.tile == tile)
        .unwrap()
        .0
}
fn keyboard(app: &mut App, window: Entity, code: KeyCode, text: Option<&str>) {
    app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: text
            .map(|s| Key::Character(s.into()))
            .unwrap_or(Key::Escape),
        state: ButtonState::Pressed,
        text: text.map(Into::into),
        repeat: false,
        window,
    });
    app.update();
}

#[test]
fn catalog_has_numbers_and_nested_patterns_without_standalone_octaves() {
    for context in [
        TileLibraryContextKind::RootBoard,
        TileLibraryContextKind::ContainerBody,
    ] {
        let options = basic_tile_options(context);
        for number in 0..=9 {
            assert_eq!(
                options
                    .iter()
                    .filter(|item| item.spawn
                        == TileSpawnKind::Atom {
                            atom: AtomValue::Number(number)
                        })
                    .count(),
                1
            );
        }
        assert!(!options.iter().any(|item| matches!(
            item.spawn,
            TileSpawnKind::Atom {
                atom: AtomValue::Ratio(_)
            }
        )));
        assert!(!options.iter().any(|item| matches!(
            item.spawn,
            TileSpawnKind::Atom {
                atom: AtomValue::Number(-1)
                    | AtomValue::Octave(_)
                    | AtomValue::Accidental(crate::domain::document::Accidental::Natural)
            }
        )));
        let pattern_kinds = options
            .iter()
            .filter_map(|item| match &item.spawn {
                TileSpawnKind::Container { kind } => Some(*kind),
                _ => None,
            })
            .collect::<Vec<_>>();
        use crate::domain::document::ContainerKind;
        assert_eq!(
            pattern_kinds,
            [
                ContainerKind::Sequence,
                ContainerKind::Arrangement,
                ContainerKind::Subdivision,
                ContainerKind::Alternating,
                ContainerKind::Parallel,
            ],
            "Both library contexts must expose each supported pattern kind once"
        );
        assert_eq!(
            options.iter().any(|item| matches!(
                item.spawn,
                TileSpawnKind::Atom {
                    atom: AtomValue::NoteName(_)
                }
            )),
            context == TileLibraryContextKind::ContainerBody
        );
        for hit in crate::domain::document::DrumHit::ALL {
            assert_eq!(
                options
                    .iter()
                    .filter(|item| {
                        matches!(
                            item.spawn,
                            TileSpawnKind::Atom {
                                atom: AtomValue::DrumHit(value)
                            } if value == hit
                        )
                    })
                    .count(),
                usize::from(context == TileLibraryContextKind::ContainerBody),
                "each named drum hit is authored from the pattern library exactly once"
            );
        }
        assert_eq!(
            options
                .iter()
                .any(|item| matches!(item.spawn, TileSpawnKind::FlowControl { .. })),
            context == TileLibraryContextKind::RootBoard
        );
    }
}

#[test]
fn native_library_has_every_named_tile_and_scroll_reaches_the_last_row() {
    let (mut app, window) = fixture();
    let expected = basic_tile_options(TileLibraryContextKind::ContainerBody).len();
    assert_eq!(
        app.world_mut()
            .query::<&DrawerTileSource>()
            .iter(app.world())
            .count(),
        expected
    );
    assert_eq!(
        app.world_mut()
            .query_filtered::<Entity, With<Mesh3d>>()
            .iter(app.world())
            .count(),
        0
    );
    let body = body(&mut app);
    let computed = *app.world().get::<ComputedNode>(body).unwrap();
    assert!(
        computed.size.y > 200.0,
        "the scroll viewport must occupy useful space: {computed:?}"
    );
    assert!(
        computed.content_size.y > computed.size.y,
        "catalog must overflow rather than shrink: {computed:?}"
    );
    let mut headings: Vec<_> = app
        .world_mut()
        .query::<(&Text, &UiGlobalTransform)>()
        .iter(app.world())
        .filter_map(|(text, transform)| {
            TileLibraryCategory::ALL
                .into_iter()
                .find(|category| category.label() == text.0)
                .map(|category| (category, transform.translation.y))
        })
        .collect();
    headings.sort_by(|a, b| a.1.total_cmp(&b.1));
    assert_eq!(
        headings.first().map(|entry| entry.0),
        Some(TileLibraryCategory::Numbers)
    );
    assert!(
        headings
            .iter()
            .any(|entry| entry.0 == TileLibraryCategory::Modulation)
    );
    let first = source(
        &mut app,
        &TileSpawnKind::Atom {
            atom: AtomValue::Number(0),
        },
    );
    assert_eq!(
        app.world().get::<ComputedNode>(first).unwrap().size.y,
        46.0,
        "simple number entries keep the compact library row height"
    );
    let children = app.world().get::<Children>(first).unwrap();
    let face = children[0];
    assert_eq!(
        app.world().get::<ComputedNode>(face).unwrap().size,
        Vec2::splat(36.0)
    );
    let label = app.world().get::<Children>(face).unwrap()[0];
    assert_eq!(app.world().get::<Text>(label).unwrap().0, "0");
    app.world_mut().trigger(pointer(
        window,
        first,
        Vec2::ZERO,
        Scroll {
            unit: MouseScrollUnit::Pixel,
            x: 0.0,
            y: -100_000.0,
            hit: hit(window),
        },
    ));
    app.update();
    let maximum = computed.content_size.y - computed.size.y;
    assert_eq!(app.world().get::<ScrollPosition>(body).unwrap().y, maximum);
    let bottom = app
        .world_mut()
        .query::<(&DrawerTileSource, &UiGlobalTransform, &ComputedNode)>()
        .iter(app.world())
        .map(|(_, transform, node)| transform.translation.y + node.size.y * 0.5)
        .fold(f32::NEG_INFINITY, f32::max);
    let viewport_bottom = app
        .world()
        .get::<UiGlobalTransform>(body)
        .unwrap()
        .translation
        .y
        + computed.size.y * 0.5;
    assert!(
        bottom <= viewport_bottom + 1.0,
        "last row must be reachable: bottom={bottom}, viewport={viewport_bottom}"
    );
    app.world_mut().trigger(pointer(
        window,
        body,
        Vec2::ZERO,
        Scroll {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: 1.0,
            hit: hit(window),
        },
    ));
    assert_eq!(
        app.world().get::<ScrollPosition>(body).unwrap().y,
        maximum - 28.0
    );
}

#[test]
fn focused_search_keeps_its_field_filters_categories_and_resets_scroll() {
    let (mut app, window) = fixture();
    let body = body(&mut app);
    app.world_mut().get_mut::<ScrollPosition>(body).unwrap().y = 700.0;
    let field = app
        .world_mut()
        .query_filtered::<Entity, With<super::super::library_search::SearchField>>()
        .single(app.world())
        .unwrap();
    app.world_mut().resource_mut::<InputFocus>().set(field);
    keyboard(&mut app, window, KeyCode::KeyD, Some("delay"));
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::KeyD)
    );
    assert_eq!(app.world().resource::<InputFocus>().0, Some(field));
    assert_eq!(app.world().get::<ScrollPosition>(body).unwrap().y, 0.0);
    let labels: Vec<_> = app
        .world_mut()
        .query::<&TileLibraryLabel>()
        .iter(app.world())
        .map(|l| l.0.clone())
        .collect();
    assert_eq!(labels, ["Delay"]);
    keyboard(&mut app, window, KeyCode::Escape, None);
    assert_eq!(app.world().resource::<InputFocus>().0, None);
    assert_eq!(
        app.world_mut()
            .query::<&DrawerTileSource>()
            .iter(app.world())
            .count(),
        basic_tile_options(TileLibraryContextKind::ContainerBody).len()
    );
    app.world_mut().resource_mut::<InputFocus>().set(field);
    keyboard(&mut app, window, KeyCode::KeyN, Some("octave"));
    assert_eq!(
        app.world_mut()
            .query::<&DrawerTileSource>()
            .iter(app.world())
            .count(),
        10,
        "octave search should discover the editable number inputs"
    );
}

#[test]
fn native_press_uses_event_position_and_ignores_secondary_buttons() {
    let (mut app, window) = fixture();
    let tile = TileSpawnKind::Atom {
        atom: AtomValue::Number(4),
    };
    let source = source(&mut app, &tile);
    app.world_mut().resource_mut::<InputFocus>().set(source);
    let start = Vec2::new(120.0, 240.0);
    app.world_mut().trigger(pointer(
        window,
        source,
        start,
        Press {
            button: PointerButton::Secondary,
            hit: hit(window),
        },
    ));
    assert!(
        app.world()
            .resource::<DrawerTilePressQueue>()
            .pending
            .is_none()
    );
    app.world_mut().trigger(pointer(
        window,
        source,
        start,
        Press {
            button: PointerButton::Primary,
            hit: hit(window),
        },
    ));
    let pending = app
        .world()
        .resource::<DrawerTilePressQueue>()
        .pending
        .as_ref()
        .unwrap();
    assert_eq!(pending.tile, tile);
    assert_eq!(pending.start_screen, start);
    assert_eq!(app.world().resource::<InputFocus>().0, None);
    app.world_mut().trigger(pointer(
        window,
        source,
        start,
        Release {
            button: PointerButton::Primary,
            hit: hit(window),
        },
    ));
    assert!(
        app.world()
            .resource::<DrawerTilePressQueue>()
            .pending
            .is_some(),
        "release is owned by the editor session, not the library"
    );
}

#[test]
fn native_tile_press_release_arms_and_motion_starts_drag_in_real_editor_session() {
    use crate::{
        application::{
            editor::{EditorPlugin, EditorSession},
            pipeline::PlaybackPlugin,
        },
        infrastructure::app::{AppState, TransportMode},
    };
    let (mut app, window) = fixture_unstarted();
    app.add_plugins(bevy::state::app::StatesPlugin)
        .init_state::<AppState>()
        .init_state::<TransportMode>()
        .add_plugins((EditorPlugin, PlaybackPlugin));
    crate::infrastructure::app::configure_pipeline_schedule(&mut app);
    app.insert_state(AppState::Editor);
    app.update();
    app.update();
    let tile = TileSpawnKind::Atom {
        atom: AtomValue::Number(4),
    };
    let start = Vec2::new(140.0, 180.0);
    let card = source(&mut app, &tile);
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(start));
    app.world_mut().trigger(pointer(
        window,
        card,
        start,
        Press {
            button: PointerButton::Primary,
            hit: hit(window),
        },
    ));
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window,
    });
    app.update();
    app.world_mut().trigger(pointer(
        window,
        card,
        start,
        Release {
            button: PointerButton::Primary,
            hit: hit(window),
        },
    ));
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window,
    });
    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<EditorSession>().armed_tile(),
        Some(&tile)
    );
    let card = source(&mut app, &tile);
    app.world_mut().trigger(pointer(
        window,
        card,
        start,
        Press {
            button: PointerButton::Primary,
            hit: hit(window),
        },
    ));
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Pressed,
        window,
    });
    app.update();
    app.world_mut()
        .get_mut::<Window>(window)
        .unwrap()
        .set_cursor_position(Some(start + Vec2::new(30.0, 0.0)));
    app.update();
    assert!(
        app.world()
            .resource::<EditorSession>()
            .is_placing_from_drawer()
    );
    app.world_mut().trigger(pointer(
        window,
        card,
        start + Vec2::new(30.0, 0.0),
        Release {
            button: PointerButton::Primary,
            hit: hit(window),
        },
    ));
    app.world_mut().write_message(MouseButtonInput {
        button: MouseButton::Left,
        state: ButtonState::Released,
        window,
    });
    app.update();
    app.update();
    assert!(
        !app.world()
            .resource::<EditorSession>()
            .is_placing_from_drawer(),
        "dropping outside the board must finish the drag"
    );
}

#[test]
fn library_grid_has_one_tab_stop_and_keyboard_choice_only_arms_once() {
    use super::super::library_search::LibraryPosition;
    use crate::application::command::{EditorCommand, EditorCommandBus};
    use bevy::input_focus::tab_navigation::TabIndex;
    let (mut app, window) = fixture();
    app.add_message::<EditorCommandBus>();
    let mut entries: Vec<_> = app
        .world_mut()
        .query::<(Entity, &LibraryPosition)>()
        .iter(app.world())
        .map(|(e, p)| (e, p.order))
        .collect();
    entries.sort_by_key(|(_, order)| *order);
    let entry = |index: usize| entries[index].0;
    assert!(entries.len() > 6);
    app.world_mut().resource_mut::<InputFocus>().set(entry(0));
    for (key, index) in [
        (KeyCode::ArrowRight, 1),
        (KeyCode::ArrowDown, 6),
        (KeyCode::ArrowUp, 1),
        (KeyCode::End, entries.len() - 1),
    ] {
        app.world_mut().write_message(KeyboardInput {
            key_code: key,
            logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window,
        });
        app.update();
        assert_eq!(app.world().resource::<InputFocus>().0, Some(entry(index)));
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(key)
        );
        assert_eq!(
            entries
                .iter()
                .filter(|(e, _)| app.world().get::<TabIndex>(*e).unwrap().0 == 0)
                .count(),
            1
        );
    }
    let chosen = app
        .world()
        .get::<DrawerTileSource>(entry(entries.len() - 1))
        .unwrap()
        .tile
        .clone();
    for repeat in [false, true] {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Enter,
            logical_key: Key::Enter,
            state: ButtonState::Pressed,
            text: None,
            repeat,
            window,
        });
        app.update();
    }
    let messages: Vec<_> = app
        .world_mut()
        .resource_mut::<Messages<EditorCommandBus>>()
        .drain()
        .collect();
    assert_eq!(messages.len(), 1);
    assert!(matches!(&messages[0].0,EditorCommand::ArmPlacementTool{tile} if tile==&chosen));
    assert!(app.world().resource::<InputFocus>().0.is_none());
    assert!(
        app.world()
            .resource::<DrawerTilePressQueue>()
            .pending
            .is_none(),
        "keyboard must not start a pointer drag"
    );
}

#[test]
fn favoriting_from_the_grid_keeps_focus_and_never_arms_or_places_a_tile() {
    use super::super::library_search::{LibraryCollection, SearchField};
    use crate::application::editor::preferences::EditorPreferences;
    let (mut app, window) = fixture();
    let tile = TileSpawnKind::Atom {
        atom: AtomValue::Number(4),
    };
    let card = source(&mut app, &tile);
    app.world_mut().resource_mut::<InputFocus>().set(card);
    let press_f = |app: &mut App| {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyF,
            logical_key: Key::Character("f".into()),
            state: ButtonState::Pressed,
            text: Some("f".into()),
            repeat: false,
            window,
        });
        app.update();
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::KeyF)
        );
    };
    press_f(&mut app);
    assert_eq!(app.world().resource::<InputFocus>().0, Some(card));
    assert_eq!(
        app.world()
            .resource::<EditorPreferences>()
            .library
            .favorites()
            .len(),
        1
    );
    assert!(
        app.world()
            .resource::<EditorPreferences>()
            .library
            .recent()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<DrawerTilePressQueue>()
            .pending
            .is_none()
    );
    assert!(
        app.world()
            .resource::<Messages<crate::application::command::EditorCommandBus>>()
            .is_empty()
    );
    app.world_mut()
        .resource_mut::<TileLibrarySearch>()
        .collection = LibraryCollection::Favorites;
    app.update();
    let chosen = source(&mut app, &tile);
    assert_eq!(app.world().resource::<InputFocus>().0, Some(chosen));
    assert_eq!(
        app.world()
            .get::<bevy::input_focus::tab_navigation::TabIndex>(chosen)
            .unwrap()
            .0,
        0
    );
    assert_eq!(
        app.world_mut()
            .query::<&DrawerTileSource>()
            .iter(app.world())
            .count(),
        1
    );
    press_f(&mut app);
    assert!(
        app.world()
            .resource::<EditorPreferences>()
            .library
            .favorites()
            .is_empty()
    );
    let focused = app.world().resource::<InputFocus>().0.unwrap();
    assert!(
        app.world().get::<SearchField>(focused).is_some(),
        "empty favorites return to library search, not board shortcuts"
    );
    assert!(
        app.world()
            .resource::<Messages<crate::application::command::EditorCommandBus>>()
            .is_empty()
    );
}

#[test]
fn saved_collections_intersect_context_and_search_without_losing_hidden_choices() {
    use super::super::library_search::LibraryCollection;
    use crate::application::editor::preferences::library::{LibraryPreferences, LibraryTileKey};
    let root = basic_tile_options(TileLibraryContextKind::RootBoard);
    let sound = root.iter().find(|item| item.label == "Sound").unwrap();
    let number = root
        .iter()
        .find(|item| {
            matches!(
                item.spawn,
                TileSpawnKind::Atom {
                    atom: AtomValue::Number(4)
                }
            )
        })
        .unwrap();
    let mut prefs = LibraryPreferences::default();
    for item in [sound, number] {
        let key = LibraryTileKey::new(&item.spawn, &item.label);
        prefs.choose(key.clone());
        prefs.toggle(key).unwrap();
    }
    let search = TileLibrarySearch {
        collection: LibraryCollection::Recent,
        ..default()
    };
    let sections = library_sections(&root, &search, &prefs);
    assert_eq!(sections[0].2, vec![number, sound]);
    let inside = basic_tile_options(TileLibraryContextKind::ContainerBody);
    let sections = library_sections(&inside, &search, &prefs);
    assert!(sections[0].2.iter().all(|item| item.label != "Sound"));
    let search = TileLibrarySearch {
        query: "sound".into(),
        collection: LibraryCollection::Favorites,
        ..default()
    };
    let sections = library_sections(&root, &search, &prefs);
    assert_eq!(sections[0].2, vec![sound]);
    assert_eq!(
        prefs.favorites().len(),
        2,
        "filtering does not delete unavailable favorites"
    );
}

#[test]
fn search_down_enters_results_without_arming_a_tile_or_moving_the_board() {
    let (mut app, window) = fixture();
    app.world_mut().resource_mut::<TileLibrarySearch>().query = "octave".into();
    app.update();
    let field = app
        .world_mut()
        .query_filtered::<Entity, With<super::super::library_search::SearchField>>()
        .single(app.world())
        .unwrap();
    app.world_mut().resource_mut::<InputFocus>().set(field);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ArrowDown,
        logical_key: Key::ArrowDown,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    let focused = app.world().resource::<InputFocus>().0.unwrap();
    assert!(app.world().get::<DrawerTileSource>(focused).is_some());
    assert_eq!(
        app.world()
            .get::<super::super::library_search::LibraryPosition>(focused)
            .unwrap()
            .order,
        0
    );
    assert_eq!(
        app.world()
            .get::<bevy::input_focus::tab_navigation::TabIndex>(focused)
            .unwrap()
            .0,
        0
    );
    assert!(
        !app.world()
            .resource::<ButtonInput<KeyCode>>()
            .just_pressed(KeyCode::ArrowDown)
    );
    assert!(
        app.world()
            .resource::<Messages<crate::application::command::EditorCommandBus>>()
            .is_empty()
    );
    assert!(
        app.world()
            .resource::<DrawerTilePressQueue>()
            .pending
            .is_none()
    );
}
