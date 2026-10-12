use super::*;

#[test]
fn elapsed_deadline_refreshes_current_clock_with_zero_or_capped_animation_time() {
    for format in [
        DateTimeIndicatorFormat::Time,
        DateTimeIndicatorFormat::DateTime,
    ] {
        for dt in [Duration::ZERO, Duration::from_millis(100)] {
            let mut indicator = DateTimeIndicator::<()>::new().format(format);
            indicator.time_text = "stale clock".to_string();
            indicator.update_due = Instant::now() - Duration::from_secs(1);
            let before = format_datetime(local_datetime(), format);
            let tick_started = Instant::now();

            let result = indicator.tick(dt, AnimationSettings::default());

            let after = format_datetime(local_datetime(), format);
            assert!(indicator.time_text == before || indicator.time_text == after);
            assert!(result.changed);
            assert!(!result.active);
            assert!(indicator.update_due > tick_started);
        }
    }
}

#[test]
fn clock_keeps_current_label_until_deadline_even_with_large_animation_time() {
    let mut indicator = DateTimeIndicator::<()>::new();
    indicator.time_text = "stored clock".to_string();
    indicator.update_due = Instant::now() + Duration::from_secs(60);
    let update_due = indicator.update_due;

    let result = indicator.tick(Duration::from_secs(120), AnimationSettings::default());

    assert_eq!(indicator.time_text, "stored clock");
    assert!(!result.changed);
    assert!(!result.active);
    assert_eq!(indicator.update_due, update_due);
}

#[test]
fn static_clock_builder_keeps_override_after_deadline() {
    let mut indicator = DateTimeIndicator::<()>::new().time_text("fixed clock");
    indicator.update_due = Instant::now() - Duration::from_secs(1);
    let update_due = indicator.update_due;

    let result = indicator.tick(Duration::ZERO, AnimationSettings::default());

    assert_eq!(indicator.time_text, "fixed clock");
    assert!(!result.changed);
    assert!(!result.active);
    assert_eq!(indicator.update_due, update_due);
}

#[test]
fn static_clock_setter_keeps_override_after_deadline() {
    let mut indicator = DateTimeIndicator::<()>::new();
    indicator.update_due = Instant::now() - Duration::from_secs(1);
    let update_due = indicator.update_due;
    indicator.set_time_text("fixed clock");

    let result = indicator.tick(Duration::from_millis(100), AnimationSettings::default());

    assert_eq!(indicator.time_text, "fixed clock");
    assert!(!result.changed);
    assert!(!result.active);
    assert_eq!(indicator.update_due, update_due);
}
