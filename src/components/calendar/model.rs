use time::{Date, Duration, PrimitiveDateTime, Time};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarView {
    Month,
    Week,
    Day,
    EventDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarEntryRole {
    Accent,
    Success,
    Warning,
    Error,
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarSpan {
    pub start: PrimitiveDateTime,
    pub end: PrimitiveDateTime,
    pub all_day: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarTypedEvent<Id> {
    ViewChanged {
        view: CalendarView,
    },
    RangeChanged {
        start: Date,
        end: Date,
    },
    CursorChanged {
        date: Date,
    },
    DateActivated {
        date: Date,
    },
    EntryHighlighted {
        entry_id: Option<Id>,
    },
    EntryActivated {
        entry_id: Id,
    },
    DrillDown {
        from: CalendarView,
        to: CalendarView,
    },
    Back {
        from: CalendarView,
        to: CalendarView,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarOutcome {
    pub handled: bool,
    pub changed: bool,
    pub activated: bool,
}

impl CalendarSpan {
    pub fn timed(start: PrimitiveDateTime, end: PrimitiveDateTime) -> Self {
        Self {
            start,
            end,
            all_day: false,
        }
    }

    pub fn all_day(date: Date) -> Self {
        Self {
            start: date.with_time(Time::MIDNIGHT),
            end: (date + Duration::days(1)).with_time(Time::MIDNIGHT),
            all_day: true,
        }
    }

    pub fn all_day_range(start: Date, end_exclusive: Date) -> Self {
        Self {
            start: start.with_time(Time::MIDNIGHT),
            end: end_exclusive.with_time(Time::MIDNIGHT),
            all_day: true,
        }
    }

    pub fn covers_date(self, date: Date) -> bool {
        let day_start = date.with_time(Time::MIDNIGHT);
        let Some(next_day) = date.checked_add(Duration::days(1)) else {
            return self.start.date() == date;
        };
        let day_end = next_day.with_time(Time::MIDNIGHT);
        self.start < day_end && self.end > day_start
    }
}

impl CalendarOutcome {
    pub const IDLE: Self = Self {
        handled: false,
        changed: false,
        activated: false,
    };

    pub const HANDLED: Self = Self {
        handled: true,
        changed: false,
        activated: false,
    };

    pub const CHANGED: Self = Self {
        handled: true,
        changed: true,
        activated: false,
    };

    pub const ACTIVATED: Self = Self {
        handled: true,
        changed: true,
        activated: true,
    };

    pub(super) fn needs_redraw(self) -> bool {
        self.changed || self.activated
    }

    pub(super) fn with_activated(self) -> Self {
        Self {
            activated: true,
            ..self
        }
    }
}
