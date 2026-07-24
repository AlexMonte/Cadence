//! Playback transport over a decoded audio region.

use crate::adapter::audio::{
    playback_position::PlaybackPosition,
    region::{EndPosition, Region},
};

#[derive(Debug, Clone, PartialEq)]
/// Forward or reverse transport through a playback region.
pub struct Transport {
    position: usize,
    playback_region: (usize, usize),
    loop_region: Option<(usize, usize)>,
    playing: bool,
    reverse: bool,
}

impl Transport {
    /// Creates a transport over the provided playback and loop regions.
    #[must_use]
    pub fn new(
        start_position: PlaybackPosition,
        playback_region: Option<Region>,
        loop_region: Option<Region>,
        reverse: bool,
        sample_rate: u32,
        num_frames: usize,
    ) -> Self {
        let playback_region =
            region_bounds(playback_region, sample_rate, num_frames).unwrap_or((0, num_frames));
        let loop_region = region_bounds(loop_region, sample_rate, num_frames)
            .and_then(|region| clip_region(region, playback_region));
        let playback_len = playback_region.1.saturating_sub(playback_region.0);
        let offset = start_position
            .into_samples(sample_rate)
            .min(playback_len.saturating_sub(1));
        let playing = num_frames > 0 && playback_region.0 < playback_region.1;
        let position = if !playing {
            playback_region.0
        } else if reverse {
            playback_region.1.saturating_sub(1 + offset)
        } else {
            playback_region.0 + offset
        };

        Self {
            position,
            playback_region,
            loop_region,
            playing,
            reverse,
        }
    }

    /// Returns the current frame position.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// Returns whether playback is still active.
    #[must_use]
    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Returns whether playback is running in reverse.
    #[must_use]
    pub fn reverse(&self) -> bool {
        self.reverse
    }

    /// Replaces the loop region, clipping it to the playback region.
    pub fn set_loop_region(
        &mut self,
        loop_region: Option<Region>,
        sample_rate: u32,
        num_frames: usize,
    ) {
        self.loop_region = region_bounds(loop_region, sample_rate, num_frames)
            .and_then(|region| clip_region(region, self.playback_region));
    }

    /// Advances by one output frame in the current direction.
    pub fn advance(&mut self) {
        if self.reverse {
            self.decrement_position();
        } else {
            self.increment_position();
        }
    }

    /// Advances one frame forward.
    pub fn increment_position(&mut self) {
        if !self.playing {
            return;
        }

        let (playback_start, playback_end) = self.playback_region;
        let next = self.position.saturating_add(1);

        if let Some((loop_start, loop_end)) = self.loop_region
            && next >= loop_end
        {
            self.position = loop_start;
            return;
        }

        if next >= playback_end {
            self.playing = false;
            self.position = playback_end.saturating_sub(1).max(playback_start);
        } else {
            self.position = next;
        }
    }

    /// Advances one frame backward.
    pub fn decrement_position(&mut self) {
        if !self.playing {
            return;
        }

        let (playback_start, playback_end) = self.playback_region;

        if let Some((loop_start, loop_end)) = self.loop_region
            && self.position <= loop_start
        {
            self.position = loop_end.saturating_sub(1);
            return;
        }

        if self.position <= playback_start {
            self.playing = false;
            self.position = playback_start.min(playback_end.saturating_sub(1));
        } else {
            self.position -= 1;
        }
    }

    /// Seeks to a new playback position inside the current playback/loop
    /// bounds.
    pub fn seek_to(&mut self, position: PlaybackPosition, sample_rate: u32, num_frames: usize) {
        let (playback_start, playback_end) = self.playback_region;
        let playback_len = playback_end.saturating_sub(playback_start);
        let offset = position
            .into_samples(sample_rate)
            .min(playback_len.saturating_sub(1));
        let mut target = playback_start + offset;

        if let Some((loop_start, loop_end)) = self.loop_region {
            if target > self.position {
                while target >= loop_end {
                    target -= loop_end - loop_start;
                }
            } else {
                while target < loop_start {
                    target += loop_end - loop_start;
                }
            }
        }

        self.position = target.min(num_frames.saturating_sub(1));
        self.playing = self.position < playback_end;
    }
}

fn region_bounds(
    region: Option<Region>,
    sample_rate: u32,
    num_frames: usize,
) -> Option<(usize, usize)> {
    region.map(|region| {
        let start = region.start.into_samples(sample_rate).min(num_frames);
        let end = match region.end {
            EndPosition::EndOfAudio => num_frames,
            EndPosition::Custom(position) => position.into_samples(sample_rate).min(num_frames),
        };

        (start, end.max(start))
    })
}

fn clip_region(region: (usize, usize), bounds: (usize, usize)) -> Option<(usize, usize)> {
    let start = region.0.max(bounds.0);
    let end = region.1.min(bounds.1);

    (start < end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_starts_and_stops_at_region_bounds() {
        let mut transport = Transport::new(
            PlaybackPosition::Samples(0),
            Some((1_u64..3_u64).into()),
            None,
            false,
            44_100,
            4,
        );

        assert_eq!(transport.position(), 1);
        transport.advance();
        assert_eq!(transport.position(), 2);
        transport.advance();
        assert!(!transport.playing());
    }

    #[test]
    fn transport_loops_forward() {
        let mut transport = Transport::new(
            PlaybackPosition::Samples(0),
            Some((1_u64..4_u64).into()),
            Some((2_u64..4_u64).into()),
            false,
            44_100,
            5,
        );

        transport.advance();
        assert_eq!(transport.position(), 2);
        transport.advance();
        assert_eq!(transport.position(), 3);
        transport.advance();
        assert_eq!(transport.position(), 2);
    }

    #[test]
    fn transport_reverse_starts_from_end_of_region() {
        let transport = Transport::new(
            PlaybackPosition::default(),
            Some((1_u64..4_u64).into()),
            None,
            true,
            44_100,
            5,
        );

        assert_eq!(transport.position(), 3);
    }

    #[test]
    fn transport_seeks_using_seconds() {
        let mut transport = Transport::new(PlaybackPosition::default(), None, None, false, 10, 100);

        transport.seek_to(PlaybackPosition::Seconds(1.5), 10, 100);

        assert_eq!(transport.position(), 15);
    }
}
