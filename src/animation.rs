use std::{num::NonZeroU32, time::Duration};

use tuirealm::ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnimationSettings {
    pub enabled: bool,
    pub target_fps: NonZeroU32,
    pub max_dt: Duration,
    pub default_duration: Duration,
    pub default_easing: Easing,
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            target_fps: NonZeroU32::new(60).expect("60 is non-zero"),
            max_dt: Duration::from_millis(100),
            default_duration: Duration::from_millis(150),
            default_easing: Easing::EaseInOut,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn resolve_disables_when_global_setting_is_disabled() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;

        let resolved = settings.resolve(AnimationSpec {
            enabled: Some(true),
            duration: Some(Duration::from_millis(25)),
            easing: Some(Easing::Linear),
        });

        assert!(!resolved.enabled);
        assert_eq!(resolved.duration, Duration::from_millis(25));
        assert_eq!(resolved.easing, Easing::Linear);
    }

    #[test]
    fn resolve_obeys_component_disabled_override() {
        let resolved = AnimationSettings::default().resolve(AnimationSpec {
            enabled: Some(false),
            duration: None,
            easing: None,
        });

        assert!(!resolved.enabled);
    }

    #[test]
    fn color_tween_interpolates_rgb_channels() {
        let mut tween = ColorTween::idle(Color::Rgb(0, 0, 0));

        tween.start(
            Color::Rgb(100, 50, 200),
            AnimationSettings::default(),
            AnimationSpec {
                duration: Some(Duration::from_millis(100)),
                easing: Some(Easing::Linear),
                enabled: None,
            },
        );
        tween.tick(Duration::from_millis(50), AnimationSettings::default());

        assert_eq!(tween.value(), Color::Rgb(50, 25, 100));
    }

    #[test]
    fn color_tween_snaps_when_disabled() {
        let mut settings = AnimationSettings::default();
        settings.enabled = false;
        let mut tween = ColorTween::idle(Color::Rgb(0, 0, 0));

        tween.start(Color::Rgb(100, 50, 200), settings, AnimationSpec::default());

        assert_eq!(tween.value(), Color::Rgb(100, 50, 200));
        assert!(!tween.is_active());
    }
}

impl AnimationSettings {
    pub fn frame_duration(self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.target_fps.get()))
    }

    pub fn resolve(self, spec: AnimationSpec) -> ResolvedAnimationSpec {
        ResolvedAnimationSpec {
            enabled: self.enabled && spec.enabled.unwrap_or(true),
            duration: spec.duration.unwrap_or(self.default_duration),
            easing: spec.easing.unwrap_or(self.default_easing),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AnimationSpec {
    pub enabled: Option<bool>,
    pub duration: Option<Duration>,
    pub easing: Option<Easing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedAnimationSpec {
    pub enabled: bool,
    pub duration: Duration,
    pub easing: Easing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Easing {
    Linear,
    EaseInOut,
    EaseOutQuad,
    EaseOutCubic,
}

impl Easing {
    pub fn apply(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseInOut => {
                if t < 0.5 {
                    4.0 * t.powi(3)
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Self::EaseOutQuad => 1.0 - (1.0 - t).powi(2),
            Self::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickResult {
    pub changed: bool,
    pub active: bool,
}

impl TickResult {
    pub const IDLE: Self = Self {
        changed: false,
        active: false,
    };

    pub const CHANGED: Self = Self {
        changed: true,
        active: false,
    };

    pub const ACTIVE: Self = Self {
        changed: true,
        active: true,
    };

    pub fn merge(self, other: Self) -> Self {
        Self {
            changed: self.changed || other.changed,
            active: self.active || other.active,
        }
    }
}

pub trait Animated {
    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult;
}

#[derive(Debug, Clone)]
pub struct Tween {
    from: f64,
    to: f64,
    current: f64,
    elapsed: Duration,
    duration: Duration,
    easing: Easing,
    active: bool,
}

impl Tween {
    pub fn idle(value: f64) -> Self {
        Self {
            from: value,
            to: value,
            current: value,
            elapsed: Duration::ZERO,
            duration: Duration::ZERO,
            easing: Easing::Linear,
            active: false,
        }
    }

    pub fn start(&mut self, from: f64, to: f64, duration: Duration, easing: Easing) {
        self.from = from;
        self.to = to;
        self.current = from;
        self.elapsed = Duration::ZERO;
        self.duration = duration;
        self.easing = easing;
        self.active = from != to && !duration.is_zero();
        if !self.active {
            self.current = to;
        }
    }

    pub fn value(&self) -> f64 {
        self.current
    }

    pub fn progress(&self) -> f64 {
        if self.duration.is_zero() {
            return 1.0;
        }
        (self.elapsed.as_secs_f64() / self.duration.as_secs_f64()).clamp(0.0, 1.0)
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn eased_progress(&self) -> f64 {
        self.easing.apply(self.progress())
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn snap_to_end(&mut self) -> TickResult {
        let changed = self.current != self.to || self.active;
        self.current = self.to;
        self.elapsed = self.duration;
        self.active = false;
        TickResult {
            changed,
            active: false,
        }
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        if !self.active {
            return TickResult::IDLE;
        }
        if !settings.enabled {
            return self.snap_to_end();
        }

        let dt = dt.min(settings.max_dt);
        self.elapsed = self.elapsed.saturating_add(dt).min(self.duration);
        let eased = self.easing.apply(self.progress());
        self.current = self.from + (self.to - self.from) * eased;

        if self.elapsed >= self.duration {
            self.current = self.to;
            self.active = false;
            TickResult::CHANGED
        } else {
            TickResult::ACTIVE
        }
    }
}

#[derive(Debug, Clone)]
pub struct ColorTween {
    from: Color,
    to: Color,
    current: Color,
    tween: Tween,
}

impl ColorTween {
    pub fn idle(value: Color) -> Self {
        Self {
            from: value,
            to: value,
            current: value,
            tween: Tween::idle(1.0),
        }
    }

    pub fn value(&self) -> Color {
        self.current
    }

    pub fn is_active(&self) -> bool {
        self.tween.is_active()
    }

    pub fn start(&mut self, target: Color, settings: AnimationSettings, spec: AnimationSpec) {
        let animation = settings.resolve(spec);
        if !animation.enabled || self.current == target || !colors_can_tween(self.current, target) {
            self.snap_to(target);
            return;
        }

        self.from = self.current;
        self.to = target;
        self.tween
            .start(0.0, 1.0, animation.duration, animation.easing);
    }

    pub fn snap_to(&mut self, target: Color) -> TickResult {
        let changed = self.current != target || self.tween.is_active();
        self.from = target;
        self.to = target;
        self.current = target;
        self.tween.snap_to_end();
        TickResult {
            changed,
            active: false,
        }
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        let result = self.tween.tick(dt, settings);
        if result.changed {
            self.current = lerp_color(self.from, self.to, self.tween.value());
        }
        result
    }
}

pub fn lerp_color(from: Color, to: Color, progress: f64) -> Color {
    let progress = progress.clamp(0.0, 1.0);
    match (from, to) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(tr, tg, tb)) => Color::Rgb(
            lerp_u8(fr, tr, progress),
            lerp_u8(fg, tg, progress),
            lerp_u8(fb, tb, progress),
        ),
        _ if progress >= 1.0 => to,
        _ => from,
    }
}

fn colors_can_tween(from: Color, to: Color) -> bool {
    matches!((from, to), (Color::Rgb(_, _, _), Color::Rgb(_, _, _)))
}

fn lerp_u8(from: u8, to: u8, progress: f64) -> u8 {
    (from as f64 + (to as f64 - from as f64) * progress).round() as u8
}

#[derive(Debug, Clone)]
pub struct ScrollAnimator {
    current: f64,
    target: f64,
}

impl ScrollAnimator {
    pub fn new(value: f64) -> Self {
        Self {
            current: value,
            target: value,
        }
    }

    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    pub fn snap_to(&mut self, target: f64) {
        self.current = target;
        self.target = target;
    }

    pub fn current(&self) -> f64 {
        self.current
    }

    pub fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        if self.current == self.target {
            return TickResult::IDLE;
        }
        if !settings.enabled {
            let changed = self.current != self.target;
            self.current = self.target;
            return TickResult {
                changed,
                active: false,
            };
        }

        let before = self.current;
        let speed = 24.0 * dt.min(settings.max_dt).as_secs_f64();
        self.current += (self.target - self.current) * speed.clamp(0.0, 1.0);
        if (self.target - self.current).abs() < 0.5 {
            self.current = self.target;
        }

        TickResult {
            changed: before != self.current,
            active: self.current != self.target,
        }
    }
}
