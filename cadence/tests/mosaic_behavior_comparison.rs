use cadence::{
    application::{clock::Clock, renderer_core::RendererCore},
    domain::{
        intent::Intent,
        moment::Moment,
        mosaic::{Mosaic, ops},
        rational::Time,
        span::Span,
    },
};
use std::time::Instant;

fn time(numerator: i64, denominator: i64) -> Time {
    Time::new(numerator, denominator)
}

fn sample_moment(start: (i64, i64), end: (i64, i64), sample: &str, id: u64) -> Moment {
    Moment::spanning(
        time(start.0, start.1),
        time(end.0, end.1),
        Intent::sample(sample),
    )
    .unwrap()
    .with_id(id)
}

fn span_signature(span: Span) -> ((i64, i64), (i64, i64)) {
    (
        (span.start().numerator(), span.start().denominator()),
        (span.end().numerator(), span.end().denominator()),
    )
}

fn mosaic_signature(mosaic: &Mosaic) -> Vec<(((i64, i64), (i64, i64)), Option<u64>, String)> {
    mosaic
        .iter()
        .map(|moment| {
            let sample = match moment.intent() {
                Intent::Sample(sample) => sample.sample_id.clone(),
                other => format!("{other:?}"),
            };

            (
                span_signature(moment.span()),
                moment.id().map(|id| id.value()),
                sample,
            )
        })
        .collect()
}

fn render_signature(
    mosaic: Mosaic,
    window: Span,
) -> Vec<(
    ((i64, i64), (i64, i64)),
    ((i64, i64), (i64, i64)),
    Option<u64>,
    String,
)> {
    let renderer = RendererCore::mosaic(mosaic);
    let clock = Clock::new(time(2, 1), Instant::now());

    renderer
        .render_window(&window, &clock)
        .unwrap()
        .into_iter()
        .map(|intent| {
            let sample = match intent.intent() {
                Intent::Sample(sample) => sample.sample_id.clone(),
                other => format!("{other:?}"),
            };

            (
                span_signature(intent.whole()),
                span_signature(intent.visible()),
                intent.moment_id().map(|id| id.value()),
                sample,
            )
        })
        .collect()
}

#[test]
fn overlay_matches_legacy_stack_for_finite_material() {
    let kick = Mosaic::new(vec![sample_moment((0, 1), (1, 2), "kick", 1)]);
    let hat = Mosaic::new(vec![sample_moment((0, 1), (1, 2), "hat", 2)]);

    let layered = ops::overlay(vec![kick, hat]);

    assert_eq!(
        mosaic_signature(&layered),
        vec![
            (((0, 1), (1, 2)), Some(1), "kick".to_string()),
            (((0, 1), (1, 2)), Some(2), "hat".to_string()),
        ]
    );
}

#[test]
fn subdivide_matches_equal_slot_output_for_two_three_and_four_parts() {
    let source = sample_moment((0, 1), (1, 1), "vox", 7);

    let halves = ops::subdivide(&source, 2);
    let thirds = ops::subdivide(&source, 3);
    let quarters = ops::subdivide(&source, 4);

    assert_eq!(
        mosaic_signature(&halves),
        vec![
            (((0, 1), (1, 2)), Some(7), "vox".to_string()),
            (((1, 2), (1, 1)), Some(7), "vox".to_string()),
        ]
    );
    assert_eq!(
        mosaic_signature(&thirds),
        vec![
            (((0, 1), (1, 3)), Some(7), "vox".to_string()),
            (((1, 3), (2, 3)), Some(7), "vox".to_string()),
            (((2, 3), (1, 1)), Some(7), "vox".to_string()),
        ]
    );
    assert_eq!(
        mosaic_signature(&quarters),
        vec![
            (((0, 1), (1, 4)), Some(7), "vox".to_string()),
            (((1, 4), (1, 2)), Some(7), "vox".to_string()),
            (((1, 2), (3, 4)), Some(7), "vox".to_string()),
            (((3, 4), (1, 1)), Some(7), "vox".to_string()),
        ]
    );
}

#[test]
fn stretch_and_mirror_match_direct_span_transform_behavior() {
    let source = Mosaic::new(vec![sample_moment((0, 1), (1, 4), "snare", 9)]);
    let stretched = ops::stretch(&source, time(2, 1));
    let mirrored = ops::mirror(&stretched, Span::new(Time::ZERO, time(1, 1)).unwrap());

    assert_eq!(
        mosaic_signature(&stretched),
        vec![(((0, 1), (1, 2)), Some(9), "snare".to_string())]
    );
    assert_eq!(
        mosaic_signature(&mirrored),
        vec![(((1, 2), (1, 1)), Some(9), "snare".to_string())]
    );
}

#[test]
fn renderer_preserves_whole_span_and_clips_visible_span_like_old_event_model() {
    let source = Mosaic::new(vec![sample_moment((0, 1), (1, 1), "pad", 4)]);
    let window = Span::new(time(1, 4), time(1, 2)).unwrap();

    assert_eq!(
        render_signature(source, window),
        vec![(
            ((0, 1), (1, 1)),
            ((1, 4), (1, 2)),
            Some(4),
            "pad".to_string()
        )]
    );
}

#[test]
fn chain_is_an_intentional_divergence_from_cycle_repeating_concat() {
    let first = Mosaic::new(vec![sample_moment((2, 1), (5, 2), "kick", 1)]);
    let second = Mosaic::new(vec![sample_moment((4, 1), (5, 1), "snare", 2)]);
    let chained = ops::chain(vec![first, second]);

    assert_eq!(
        mosaic_signature(&chained),
        vec![
            (((0, 1), (1, 2)), Some(1), "kick".to_string()),
            (((1, 2), (3, 2)), Some(2), "snare".to_string()),
        ]
    );

    let later_window = Span::new(time(2, 1), time(3, 1)).unwrap();
    assert!(
        render_signature(chained, later_window).is_empty(),
        "chained mosaics are finite and do not implicitly repeat by cycle"
    );
}
