use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eframe::egui::ColorImage;

pub struct Frame {
    pub pixels: Arc<ColorImage>,
    pub delay: Duration,
}

#[derive(Clone, Copy)]
enum Playback {
    Paused,
    Playing { next_change: Instant },
}

pub struct Animation {
    frames: Vec<Frame>,
    current: usize,
    playback: Playback,
    plays: Option<u32>,
    plays_left: Option<u32>,
}

impl Animation {
    pub fn new(frames: Vec<Frame>, plays: Option<NonZeroU32>) -> Self {
        let plays = plays.map(NonZeroU32::get);
        Self {
            frames,
            current: 0,
            playback: Playback::Paused,
            plays,
            plays_left: plays,
        }
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn index(&self) -> usize {
        self.current
    }

    pub fn pixels(&self) -> &Arc<ColorImage> {
        &self.frames[self.current].pixels
    }

    pub fn is_playing(&self) -> bool {
        matches!(self.playback, Playback::Playing { .. })
    }

    pub fn next_change(&self) -> Option<Instant> {
        match self.playback {
            Playback::Playing { next_change } => Some(next_change),
            Playback::Paused => None,
        }
    }

    pub fn toggle(&mut self, now: Instant) -> Option<usize> {
        if self.is_playing() {
            self.playback = Playback::Paused;
            return None;
        }
        let restarted = (self.plays_left == Some(0)).then(|| {
            self.plays_left = self.plays;
            self.current = 0;
            0
        });
        self.schedule(now);
        restarted
    }

    pub fn tick(&mut self, now: Instant) -> Option<usize> {
        let Playback::Playing { next_change } = self.playback else {
            return None;
        };
        if now < next_change {
            return None;
        }
        if self.current + 1 < self.frames.len() {
            self.current += 1;
        } else if self.finish_play() {
            self.playback = Playback::Paused;
            return None;
        } else {
            self.current = 0;
        }
        self.schedule(now);
        Some(self.current)
    }

    pub fn step(&mut self, forward: bool) -> Option<usize> {
        if self.is_playing() {
            return None;
        }
        let count = self.frames.len();
        self.current = if forward {
            (self.current + 1) % count
        } else {
            (self.current + count - 1) % count
        };
        Some(self.current)
    }

    fn finish_play(&mut self) -> bool {
        let Some(left) = self.plays_left else {
            return false;
        };
        let left = left.saturating_sub(1);
        self.plays_left = Some(left);
        left == 0
    }

    fn schedule(&mut self, now: Instant) {
        self.playback = Playback::Playing {
            next_change: now + self.frames[self.current].delay,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DELAY: Duration = Duration::from_millis(100);

    fn animation(frames: usize, plays: Option<u32>) -> Animation {
        let frame = || Frame {
            pixels: Arc::new(ColorImage::example()),
            delay: DELAY,
        };
        Animation::new(
            (0..frames).map(|_| frame()).collect(),
            plays.and_then(NonZeroU32::new),
        )
    }

    #[test]
    fn starts_paused_on_the_first_frame() {
        let animation = animation(3, None);
        assert!(!animation.is_playing());
        assert_eq!(animation.index(), 0);
        assert_eq!(animation.next_change(), None);
    }

    #[test]
    fn playing_advances_only_when_a_delay_has_passed() {
        let start = Instant::now();
        let mut animation = animation(3, None);
        assert_eq!(animation.toggle(start), None);
        assert_eq!(animation.tick(start + DELAY / 2), None);
        assert_eq!(animation.tick(start + DELAY), Some(1));
        assert_eq!(animation.next_change(), Some(start + DELAY * 2));
        assert_eq!(animation.tick(start + DELAY * 2), Some(2));
    }

    #[test]
    fn an_endless_animation_wraps_around() {
        let start = Instant::now();
        let mut animation = animation(2, None);
        animation.toggle(start);
        assert_eq!(animation.tick(start + DELAY), Some(1));
        assert_eq!(animation.tick(start + DELAY * 2), Some(0));
        assert!(animation.is_playing());
    }

    #[test]
    fn a_finite_animation_stops_on_its_last_frame_and_restarts_on_play() {
        let start = Instant::now();
        let mut animation = animation(2, Some(1));
        animation.toggle(start);
        assert_eq!(animation.tick(start + DELAY), Some(1));
        assert_eq!(animation.tick(start + DELAY * 2), None);
        assert!(!animation.is_playing());
        assert_eq!(animation.index(), 1);
        assert_eq!(animation.toggle(start + DELAY * 3), Some(0));
        assert!(animation.is_playing());
    }

    #[test]
    fn several_plays_repeat_before_stopping() {
        let start = Instant::now();
        let mut animation = animation(2, Some(2));
        animation.toggle(start);
        let mut now = start;
        let mut changes = Vec::new();
        while animation.is_playing() {
            now += DELAY;
            changes.extend(animation.tick(now));
        }
        assert_eq!(changes, [1, 0, 1]);
    }

    #[test]
    fn stepping_wraps_and_is_refused_while_playing() {
        let start = Instant::now();
        let mut animation = animation(3, None);
        assert_eq!(animation.step(false), Some(2));
        assert_eq!(animation.step(true), Some(0));
        assert_eq!(animation.step(true), Some(1));
        animation.toggle(start);
        assert_eq!(animation.step(true), None);
        assert_eq!(animation.index(), 1);
    }

    #[test]
    fn pausing_keeps_the_shown_frame() {
        let start = Instant::now();
        let mut animation = animation(3, None);
        animation.toggle(start);
        animation.tick(start + DELAY);
        assert_eq!(animation.toggle(start + DELAY), None);
        assert!(!animation.is_playing());
        assert_eq!(animation.index(), 1);
    }
}
