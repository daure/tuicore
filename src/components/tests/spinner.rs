use super::*;

#[test]
fn spinner_schedules_frame_changes_and_preserves_partial_intervals() {
    let mut spinner = Spinner::new();
    let settings = AnimationSettings::default();
    assert_eq!(spinner.glyph(), "⠋");

    let result = Animated::tick(&mut spinner, Duration::from_millis(50), settings);
    assert!(!result.changed);
    assert!(!result.active);
    assert_eq!(result.next_tick, Some(Duration::from_millis(30)));
    assert_eq!(spinner.glyph(), "⠋");

    let result = Animated::tick(&mut spinner, Duration::from_millis(60), settings);
    assert!(result.changed);
    assert!(!result.active);
    assert_eq!(result.next_tick, Some(Duration::from_millis(50)));
    assert_eq!(spinner.glyph(), "⠙");

    let result = Animated::tick(&mut spinner, Duration::from_millis(850), settings);
    assert!(result.changed);
    assert_eq!(spinner.glyph(), "⠹");
    assert_eq!(result.next_tick, Some(Duration::from_millis(80)));
}

#[test]
fn spinner_stays_idle_when_animations_disabled() {
    let mut spinner = Spinner::new();
    let result = Animated::tick(
        &mut spinner,
        Duration::from_millis(150),
        AnimationSettings {
            enabled: false,
            ..Default::default()
        },
    );
    assert_eq!(result, TickResult::IDLE);
    assert_eq!(spinner.glyph(), "⠋");
}
