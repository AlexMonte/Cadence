//! Playback-region helpers.

use std::ops::{Range, RangeFrom, RangeFull, RangeTo};

use crate::adapter::audio::playback_position::PlaybackPosition;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
/// End bound of an audio playback region.
pub enum EndPosition {
    /// Play until the end of the decoded audio.
    #[default]
    EndOfAudio,
    /// Stop at a custom playback position.
    Custom(PlaybackPosition),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
/// Playback region inside one decoded sample.
pub struct Region {
    /// Region start position.
    pub start: PlaybackPosition,
    /// Region end position.
    pub end: EndPosition,
}

impl Region {
    /// Creates a playback region from explicit bounds.
    #[must_use]
    pub fn new(start: PlaybackPosition, end: EndPosition) -> Self {
        Self { start, end }
    }
}

impl<T: Into<PlaybackPosition>> From<RangeFrom<T>> for Region {
    fn from(range: RangeFrom<T>) -> Self {
        Self {
            start: range.start.into(),
            end: EndPosition::EndOfAudio,
        }
    }
}

impl<T: Into<PlaybackPosition>> From<Range<T>> for Region {
    fn from(range: Range<T>) -> Self {
        Self {
            start: range.start.into(),
            end: EndPosition::Custom(range.end.into()),
        }
    }
}

impl<T: Into<PlaybackPosition>> From<RangeTo<T>> for Region {
    fn from(range: RangeTo<T>) -> Self {
        Self {
            start: PlaybackPosition::default(),
            end: EndPosition::Custom(range.end.into()),
        }
    }
}

impl From<RangeFull> for Region {
    fn from(_: RangeFull) -> Self {
        Self {
            start: PlaybackPosition::default(),
            end: EndPosition::EndOfAudio,
        }
    }
}
