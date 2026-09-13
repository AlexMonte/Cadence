//! Reacts to Bevy asset load-failure messages for tracked dependencies.

use bevy_asset::UntypedAssetLoadFailedEvent;
use bevy_ecs::prelude::*;

use crate::prepared::apply_asset_load_failures;

/// Reads [`UntypedAssetLoadFailedEvent`] messages and routes them through the dependency recovery pipeline.
pub fn process_untyped_asset_load_failures(
    mut reader: MessageReader<UntypedAssetLoadFailedEvent>,
    mut commands: Commands,
) {
    let failures: Vec<_> = reader.read().cloned().collect();
    if failures.is_empty() {
        return;
    }

    commands.queue(move |world: &mut World| {
        apply_asset_load_failures(world, &failures);
    });
}
