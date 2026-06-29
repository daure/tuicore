use std::time::{Duration, Instant};

use crate::AnimationSettings;

const IDLE_TIMEOUT: Duration = Duration::from_millis(i32::MAX as u64);

#[derive(Debug, Clone)]
pub struct Scheduler {
    active: bool,
    frame_duration: Duration,
    last_tick: Instant,
    scheduled_wake: Option<Instant>,
}

impl Scheduler {
    pub fn new(settings: AnimationSettings) -> Self {
        Self {
            active: false,
            frame_duration: settings.frame_duration(),
            last_tick: Instant::now(),
            scheduled_wake: None,
        }
    }

    pub fn timeout(&self) -> Duration {
        let active_timeout = self
            .active
            .then(|| self.frame_duration.saturating_sub(self.last_tick.elapsed()));
        let scheduled_timeout = self
            .scheduled_wake
            .map(|wake| wake.saturating_duration_since(Instant::now()));

        match (active_timeout, scheduled_timeout) {
            (Some(active), Some(scheduled)) => active.min(scheduled),
            (Some(timeout), None) | (None, Some(timeout)) => timeout,
            (None, None) => IDLE_TIMEOUT,
        }
    }

    pub fn tick_due(&self) -> bool {
        (self.active && self.last_tick.elapsed() >= self.frame_duration)
            || self
                .scheduled_wake
                .is_some_and(|wake| Instant::now() >= wake)
    }

    pub fn wake(&mut self) {
        if !self.active {
            self.active = true;
            self.last_tick = Instant::now();
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if self.active {
            self.last_tick = Instant::now();
        }
    }

    pub fn schedule_after(&mut self, delay: Duration) {
        let wake = Instant::now() + delay;
        self.scheduled_wake = Some(
            self.scheduled_wake
                .map(|current| current.min(wake))
                .unwrap_or(wake),
        );
    }

    pub fn tick(&mut self, max_dt: Duration) -> Option<Duration> {
        if !self.tick_due() {
            return None;
        }

        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).min(max_dt);
        self.last_tick = now;
        if self.scheduled_wake.is_some_and(|wake| now >= wake) {
            self.scheduled_wake = None;
        }
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

        let mut scheduler = Scheduler::new(settings);
        scheduler.wake();

        assert!(scheduler.timeout() <= Duration::from_millis(50));
    }

    #[test]
    fn enabled_animation_stays_idle_until_woken() {
        let scheduler = Scheduler::new(AnimationSettings::default());

        assert!(!scheduler.tick_due());
        assert_eq!(scheduler.timeout(), IDLE_TIMEOUT);
    }

    #[test]
    fn disabled_animation_still_produces_active_ticks() {
        let mut scheduler = Scheduler {
            active: true,
            frame_duration: Duration::from_millis(16),
            last_tick: Instant::now() - Duration::from_secs(1),
            scheduled_wake: None,
        };

        assert!(scheduler.tick_due());
        assert_eq!(
            scheduler.tick(Duration::from_millis(100)),
            Some(Duration::from_millis(100))
        );
    }

    #[test]
    fn scheduled_wake_sets_idle_timeout() {
        let mut scheduler = Scheduler::new(AnimationSettings::default());

        scheduler.schedule_after(Duration::from_millis(50));

        assert!(scheduler.timeout() <= Duration::from_millis(50));
        assert!(!scheduler.tick_due());
    }
}
