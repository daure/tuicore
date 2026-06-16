use std::time::{Duration, Instant};

use crate::AnimationSettings;

const DISABLED_ANIMATION_TIMEOUT: Duration = Duration::from_millis(i32::MAX as u64);

#[derive(Debug, Clone)]
pub struct Scheduler {
    enabled: bool,
    frame_duration: Duration,
    last_tick: Instant,
}

impl Scheduler {
    pub fn new(settings: AnimationSettings) -> Self {
        Self {
            enabled: settings.enabled,
            frame_duration: settings.frame_duration(),
            last_tick: Instant::now(),
        }
    }

    pub fn timeout(&self) -> Duration {
        if !self.enabled {
            return DISABLED_ANIMATION_TIMEOUT;
        }

        self.frame_duration.saturating_sub(self.last_tick.elapsed())
    }

    pub fn tick_due(&self) -> bool {
        if !self.enabled {
            return false;
        }

        self.last_tick.elapsed() >= self.frame_duration
    }

    pub fn tick(&mut self, max_dt: Duration) -> Option<Duration> {
        if !self.tick_due() {
            return None;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).min(max_dt);
        self.last_tick = now;
        Some(dt)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use super::*;
    use crate::Easing;

    #[test]
    fn timeout_uses_animation_frame_duration() {
        let settings = AnimationSettings {
            target_fps: NonZeroU32::new(20).expect("non-zero"),
            enabled: true,
            max_dt: Duration::from_millis(100),
            default_duration: Duration::from_millis(150),
            default_easing: Easing::Linear,
        };

        let scheduler = Scheduler::new(settings);

        assert!(scheduler.timeout() <= Duration::from_millis(50));
    }

    #[test]
    fn disabled_animation_never_produces_ticks() {
        let mut scheduler = Scheduler {
            enabled: false,
            frame_duration: Duration::from_millis(16),
            last_tick: Instant::now() - Duration::from_secs(1),
        };

        assert!(!scheduler.tick_due());
        assert_eq!(scheduler.tick(Duration::from_millis(100)), None);
        assert_eq!(scheduler.timeout(), DISABLED_ANIMATION_TIMEOUT);
    }
}
