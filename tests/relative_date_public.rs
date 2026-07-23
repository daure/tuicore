use time::OffsetDateTime;
use tuicore::components::date_time::{RelativeDate, RelativeDateMode};

#[test]
fn relative_date_is_available_from_public_date_time_module() {
    let now = OffsetDateTime::UNIX_EPOCH;
    let relative = RelativeDate::new(now)
        .reference(now)
        .mode(RelativeDateMode::Distance);

    assert_eq!(relative.text(), "Now");
}
