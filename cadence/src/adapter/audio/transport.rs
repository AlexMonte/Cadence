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
    loop_has_wrapped: bool,
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
            loop_has_wrapped: false,
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
        self.loop_has_wrapped = false;
    }

    /// Advances by one output frame in the current direction.
    pub fn advance(&mut self) {
        if self.reverse {
            self.decrement_position();
        } else {
            self.increment_position();
        }
    }

    // Skip source frames in constant time, including arbitrarily many loop turns.
    pub(crate) fn advance_frames(&mut self, frames: usize) {
        if !self.playing || frames == 0 {
            return;
        }
        if let Some((start, end)) = self.loop_region {
            let length = end - start;
            if self.reverse {
                let distance = self.position.saturating_sub(start);
                if frames <= distance {
                    self.position -= frames;
                } else {
                    self.position = end - 1 - (frames - distance - 1) % length;
                    self.loop_has_wrapped = true;
                }
            } else {
                let distance = end.saturating_sub(self.position).max(1);
                if frames < distance {
                    self.position += frames;
                } else {
                    self.position = start + (frames - distance) % length;
                    self.loop_has_wrapped = true;
                }
            }
        } else if self.reverse {
            let available = self.position - self.playback_region.0;
            if frames > available {
                self.position = self.playback_region.0;
                self.playing = false;
            } else {
                self.position -= frames;
            }
        } else {
            let available = self.playback_region.1 - self.position;
            if frames >= available {
                self.position = self.playback_region.1 - 1;
                self.playing = false;
            } else {
                self.position += frames;
            }
        }
    }

    // Interpolation must never borrow audio from an adjacent slice. For a loop
    // the neighboring taps wrap; a one-shot holds its edge sample instead.
    pub(crate) fn sample_index(&self, offset: isize) -> usize {
        let target = self.position as i128 + offset as i128;
        if let Some((start, end)) = self.loop_region {
            // First entry into an internal sustain loop still interpolates
            // against the attack. Only subsequent turns borrow the loop's end.
            let crossed_end = if self.reverse {
                target < start as i128
            } else {
                target >= end as i128
            };
            let inside = (start..end).contains(&self.position);
            let no_attack = if self.reverse {
                end == self.playback_region.1
            } else {
                start == self.playback_region.0
            };
            if crossed_end || (inside && (self.loop_has_wrapped || no_attack)) {
                return start + (target - start as i128).rem_euclid((end - start) as i128) as usize;
            }
        }
        target.clamp(
            self.playback_region.0 as i128,
            self.playback_region.1.saturating_sub(1) as i128,
        ) as usize
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
            self.loop_has_wrapped = true;
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
            self.loop_has_wrapped = true;
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

        self.loop_has_wrapped = false;
        if let Some((loop_start, loop_end)) = self.loop_region
            && target >= loop_end
        {
            target = loop_start
                + (target as i128 - loop_start as i128).rem_euclid((loop_end - loop_start) as i128)
                    as usize;
            self.loop_has_wrapped = true;
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
    fn frame_skipping_matches_stepwise_transport_and_wraps_huge_loops() {
        for reverse in [false, true] {
            for looping in [false, true] {
                let original = Transport::new(
                    PlaybackPosition::default(),
                    Some((1_u64..9_u64).into()),
                    looping.then(|| (3_u64..7_u64).into()),
                    reverse,
                    8_000,
                    12,
                );
                for frames in [0, 1, 2, 6, 9, 37, 1024] {
                    let mut reference = original.clone();
                    let mut skipped = original.clone();
                    for _ in 0..frames {
                        reference.advance();
                    }
                    skipped.advance_frames(frames);
                    assert_eq!(
                        skipped, reference,
                        "reverse={reverse} loop={looping} frames={frames}"
                    );
                }
            }
        }
        let mut huge = Transport::new(
            PlaybackPosition::default(),
            Some((0_u64..4_u64).into()),
            Some((0_u64..4_u64).into()),
            false,
            8_000,
            4,
        );
        huge.advance_frames(usize::MAX);
        assert_eq!(huge.position(), usize::MAX % 4);
        assert!(huge.playing());
    }

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

    #[test]
    fn sustain_interpolation_keeps_attack_history_until_the_first_wrap() {
        let mut transport = Transport::new(
            PlaybackPosition::default(),
            Some((0_u64..8_u64).into()),
            Some((2_u64..6_u64).into()),
            false,
            8000,
            8,
        );
        transport.advance_frames(2);
        assert_eq!(
            transport.sample_index(-1),
            1,
            "first loop entry borrows the attack"
        );
        transport.advance_frames(4);
        assert_eq!(
            transport.sample_index(-1),
            5,
            "later turns borrow the sustain end"
        );
        transport.seek_to(PlaybackPosition::Samples(1), 8000, 8);
        assert_eq!(
            transport.position(),
            1,
            "seeking into the attack must retain it"
        );
        transport.advance();
        assert_eq!(transport.sample_index(-1), 1);
        let mut short = Transport::new(
            PlaybackPosition::default(),
            Some((0_u64..8_u64).into()),
            Some((2_u64..3_u64).into()),
            false,
            8000,
            8,
        );
        short.advance();
        assert_eq!(
            short.sample_index(2),
            2,
            "interpolation crossing a one-frame loop must not borrow its tail"
        );
    }
}
