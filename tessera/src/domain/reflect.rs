//! Reflect support for Tessera domain types when the `bevy` feature is enabled.

#[cfg(feature = "bevy")]
pub use bevy_reflect::Reflect;

/// Derives `Reflect` when the `bevy` feature is enabled.
#[cfg(feature = "bevy")]
#[macro_export]
macro_rules! tessera_reflect {
    ($($tt:tt)*) => {
        #[cfg_attr(feature = "bevy", derive(bevy_reflect::Reflect))]
        $($tt)*
    };
}
