//! Nine-slice background bundle builder.

use bevy::prelude::*;

/// Builds a stretchable nine-slice [`ImageNode`] with consistent scaling behavior.
pub fn nine_slice_background(image: Handle<Image>, border: BorderRect) -> impl Bundle {
    (
        ImageNode {
            image,
            image_mode: NodeImageMode::Sliced(TextureSlicer {
                border,
                center_scale_mode: SliceScaleMode::Stretch,
                sides_scale_mode: SliceScaleMode::Stretch,
                max_corner_scale: 1.0,
            }),
            ..default()
        },
        Transform::default(),
    )
}
