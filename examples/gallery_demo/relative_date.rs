use std::time::Duration;

use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};
use time::{Date, OffsetDateTime, Time, UtcOffset};
use tuicore::{
    AnimationSettings, ChildKey, CrossAlign, EventCtx, EventOutcome, EventRoute, Flex, FlexItem,
    FocusCtx, FocusTarget, LayoutCtx, LayoutResult, LifecycleCtx, RelativeDate, RelativeDateMode,
    RenderCtx, TickResult, TimePicker, TimePrecision, TuiEvent, TuiNode,
};

use super::super::Msg;

pub(crate) struct RelativeDateDemo {
    showcase: Flex<Msg>,
    pickers: Flex<Msg>,
    distance: RelativeDate,
    calendar_week: RelativeDate,
    reference: OffsetDateTime,
    target: OffsetDateTime,
    offset: UtcOffset,
}

struct ShowcaseRegion;

impl TuiNode<Msg> for ShowcaseRegion {
    fn layout(&mut self, area: Rect, _ctx: &mut LayoutCtx) -> LayoutResult {
        LayoutResult::new(area)
    }

    fn render(&self, _frame: &mut Frame, _area: Rect, _ctx: &mut RenderCtx<'_>) {}
}

impl RelativeDateDemo {
    pub(crate) fn new() -> Self {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let reference_date = now.date();
        let target_date = reference_date
            .checked_add(time::Duration::days(7))
            .unwrap_or(Date::MAX);
        let offset = now.offset();
        let picker_time = Time::from_hms(now.hour(), now.minute(), 0)
            .expect("current local hour and minute form a valid time");
        let reference = date_at_time(reference_date, picker_time, offset);
        let target = date_at_time(target_date, picker_time, offset);

        Self {
            showcase: showcase_layout(),
            pickers: Flex::row()
                .gap(2)
                .align(CrossAlign::Start)
                .child(
                    reference_picker_child_key(),
                    tuicore::DatePicker::new()
                        .today(reference_date)
                        .value(Some(reference_date))
                        .hotkey("rr")
                        .on_select(Msg::RelativeReferenceSelected),
                    FlexItem::fit_content(),
                )
                .child(
                    reference_time_picker_child_key(),
                    TimePicker::new()
                        .value(picker_time)
                        .precision(TimePrecision::HourMinute)
                        .minute_step(5)
                        .hotkey("rh")
                        .on_select(Msg::RelativeReferenceTimeSelected),
                    FlexItem::fit_content(),
                )
                .child(
                    target_picker_child_key(),
                    tuicore::DatePicker::new()
                        .today(reference_date)
                        .value(Some(target_date))
                        .hotkey("rt")
                        .on_select(Msg::RelativeTargetSelected),
                    FlexItem::fit_content(),
                )
                .child(
                    target_time_picker_child_key(),
                    TimePicker::new()
                        .value(picker_time)
                        .precision(TimePrecision::HourMinute)
                        .minute_step(5)
                        .hotkey("th")
                        .on_select(Msg::RelativeTargetTimeSelected),
                    FlexItem::fit_content(),
                ),
            distance: RelativeDate::new(target)
                .reference(reference)
                .mode(RelativeDateMode::Distance),
            calendar_week: RelativeDate::new(target)
                .reference(reference)
                .mode(RelativeDateMode::CalendarWeek),
            reference,
            target,
            offset,
        }
    }

    pub(crate) fn apply_message(&mut self, message: &Msg) -> bool {
        match message {
            Msg::RelativeReferenceSelected(date) => {
                self.reference = date_at_time(*date, self.reference.time(), self.offset);
                self.distance.set_reference(self.reference);
                self.calendar_week.set_reference(self.reference);
            }
            Msg::RelativeReferenceTimeSelected(time) => {
                self.reference = date_at_time(self.reference.date(), *time, self.offset);
                self.distance.set_reference(self.reference);
                self.calendar_week.set_reference(self.reference);
            }
            Msg::RelativeTargetSelected(date) => {
                self.target = date_at_time(*date, self.target.time(), self.offset);
                self.distance.set_target(self.target);
                self.calendar_week.set_target(self.target);
            }
            Msg::RelativeTargetTimeSelected(time) => {
                self.target = date_at_time(self.target.date(), *time, self.offset);
                self.distance.set_target(self.target);
                self.calendar_week.set_target(self.target);
            }
            _ => return false,
        }
        true
    }

    pub(crate) fn picker_position(&self, key: &ChildKey) -> Option<usize> {
        picker_child_keys()
            .iter()
            .position(|candidate| candidate == key)
    }

    fn showcase_rect(&self, key: ChildKey) -> Rect {
        self.showcase
            .child_rect(&key)
            .expect("showcase region should be laid out")
    }

    #[cfg(test)]
    pub(crate) fn reference(&self) -> OffsetDateTime {
        self.reference
    }

    #[cfg(test)]
    pub(crate) fn target(&self) -> OffsetDateTime {
        self.target
    }

    #[cfg(test)]
    pub(crate) fn distance_text(&self) -> &str {
        self.distance.text()
    }

    #[cfg(test)]
    pub(crate) fn calendar_week_text(&self) -> &str {
        self.calendar_week.text()
    }
}

impl TuiNode<Msg> for RelativeDateDemo {
    fn layout(&mut self, area: Rect, ctx: &mut LayoutCtx) -> LayoutResult {
        self.showcase.layout(area, ctx);
        let pickers = self.showcase_rect(pickers_region_key());
        let distance = self.showcase_rect(distance_region_key());
        let calendar_week = self.showcase_rect(calendar_week_region_key());
        self.pickers.layout(pickers, ctx);
        ctx.push_slot(distance_child_key(), distance, |ctx| {
            <RelativeDate as TuiNode<Msg>>::layout(&mut self.distance, distance, ctx);
        });
        ctx.push_slot(calendar_week_child_key(), calendar_week, |ctx| {
            <RelativeDate as TuiNode<Msg>>::layout(&mut self.calendar_week, calendar_week, ctx);
        });
        LayoutResult::new(area)
    }

    fn render<'a>(&'a self, frame: &mut Frame, _area: Rect, ctx: &mut RenderCtx<'a>) {
        let intro = self.showcase_rect(intro_region_key());
        let picker_labels = self.showcase_rect(picker_labels_region_key());
        let pickers = self.showcase_rect(pickers_region_key());
        let distance_label = self.showcase_rect(distance_label_region_key());
        let distance = self.showcase_rect(distance_region_key());
        let calendar_label = self.showcase_rect(calendar_label_region_key());
        let calendar = self.showcase_rect(calendar_week_region_key());
        let label_style = Style::default().fg(tuicore::theme().accent_fg());
        frame.render_widget(
            Paragraph::new(
                "Select reference and target date-times. Both modes use one captured local offset.\n\
                 Date: arrows/hjkl move day/week. Time: left/right or h/l selects field; up/down or k/j changes value. Hotkeys: |rr|/|rh| reference, |rt|/|th| target.",
            ),
            intro,
        );
        for (key, label) in [
            (reference_picker_child_key(), "Reference date"),
            (reference_time_picker_child_key(), "Reference time"),
            (target_picker_child_key(), "Target date"),
            (target_time_picker_child_key(), "Target time"),
        ] {
            if let Some(rect) = self.pickers.child_rect(&key) {
                frame.render_widget(
                    Paragraph::new(label).style(label_style),
                    Rect::new(rect.x, picker_labels.y, rect.width, picker_labels.height),
                );
            }
        }
        self.pickers.render(frame, pickers, ctx);
        frame.render_widget(
            Paragraph::new("Distance mode").style(label_style),
            distance_label,
        );
        <RelativeDate as TuiNode<Msg>>::render(&self.distance, frame, distance, ctx);
        frame.render_widget(
            Paragraph::new("CalendarWeek mode").style(label_style),
            calendar_label,
        );
        <RelativeDate as TuiNode<Msg>>::render(&self.calendar_week, frame, calendar, ctx);
    }

    fn dispatch_event(
        &mut self,
        route: &EventRoute,
        event: &TuiEvent,
        ctx: &mut EventCtx<Msg>,
    ) -> EventOutcome {
        if route
            .path
            .first()
            .is_some_and(|key| self.picker_position(key).is_some())
        {
            return self.pickers.dispatch_event(route, event, ctx);
        }
        if let Some(route) = route
            .path
            .without_first_if(&distance_child_key())
            .map(EventRoute::new)
        {
            return self.distance.dispatch_event(&route, event, ctx);
        }
        let Some(route) = route
            .path
            .without_first_if(&calendar_week_child_key())
            .map(EventRoute::new)
        else {
            return EventOutcome::Ignored;
        };
        self.calendar_week.dispatch_event(&route, event, ctx)
    }

    fn dispatch_focus(&mut self, target: &FocusTarget, focused: bool, ctx: &mut FocusCtx<Msg>) {
        self.pickers.dispatch_focus(target, focused, ctx);
        if let Some(target) = target.for_child(&distance_child_key()) {
            self.distance.dispatch_focus(&target, focused, ctx);
        } else if let Some(target) = target.for_child(&calendar_week_child_key()) {
            self.calendar_week.dispatch_focus(&target, focused, ctx);
        }
    }

    fn tick(&mut self, dt: Duration, settings: AnimationSettings) -> TickResult {
        self.pickers
            .tick(dt, settings)
            .merge(<RelativeDate as TuiNode<Msg>>::tick(
                &mut self.distance,
                dt,
                settings,
            ))
            .merge(<RelativeDate as TuiNode<Msg>>::tick(
                &mut self.calendar_week,
                dt,
                settings,
            ))
    }

    fn init(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.pickers.init(ctx);
        self.distance.init(ctx);
        self.calendar_week.init(ctx);
    }

    fn mount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.pickers.mount(ctx);
        self.distance.mount(ctx);
        self.calendar_week.mount(ctx);
    }

    fn unmount(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.pickers.unmount(ctx);
        self.distance.unmount(ctx);
        self.calendar_week.unmount(ctx);
    }

    fn destroy(&mut self, ctx: &mut LifecycleCtx<Msg>) {
        self.pickers.destroy(ctx);
        self.distance.destroy(ctx);
        self.calendar_week.destroy(ctx);
    }
}

pub(crate) fn reference_picker_child_key() -> ChildKey {
    ChildKey::new("relative-reference-picker")
}

fn reference_time_picker_child_key() -> ChildKey {
    ChildKey::new("relative-reference-time-picker")
}

fn target_picker_child_key() -> ChildKey {
    ChildKey::new("relative-target-picker")
}

fn target_time_picker_child_key() -> ChildKey {
    ChildKey::new("relative-target-time-picker")
}

fn distance_child_key() -> ChildKey {
    ChildKey::new("relative-distance")
}

fn calendar_week_child_key() -> ChildKey {
    ChildKey::new("relative-calendar-week")
}

fn intro_region_key() -> ChildKey {
    ChildKey::new("relative-intro-region")
}

fn picker_labels_region_key() -> ChildKey {
    ChildKey::new("relative-picker-labels-region")
}

fn pickers_region_key() -> ChildKey {
    ChildKey::new("relative-pickers-region")
}

fn distance_label_region_key() -> ChildKey {
    ChildKey::new("relative-distance-label-region")
}

fn distance_region_key() -> ChildKey {
    ChildKey::new("relative-distance-region")
}

fn calendar_label_region_key() -> ChildKey {
    ChildKey::new("relative-calendar-label-region")
}

fn calendar_week_region_key() -> ChildKey {
    ChildKey::new("relative-calendar-week-region")
}

fn picker_child_keys() -> [ChildKey; 4] {
    [
        reference_picker_child_key(),
        reference_time_picker_child_key(),
        target_picker_child_key(),
        target_time_picker_child_key(),
    ]
}

fn showcase_layout() -> Flex<Msg> {
    Flex::column()
        .child(intro_region_key(), ShowcaseRegion, FlexItem::fixed(3))
        .child(
            picker_labels_region_key(),
            ShowcaseRegion,
            FlexItem::fixed(1),
        )
        .child(pickers_region_key(), ShowcaseRegion, FlexItem::fixed(10))
        .child(
            distance_label_region_key(),
            ShowcaseRegion,
            FlexItem::fixed(1),
        )
        .child(distance_region_key(), ShowcaseRegion, FlexItem::fixed(1))
        .child(
            calendar_label_region_key(),
            ShowcaseRegion,
            FlexItem::fixed(1),
        )
        .child(
            calendar_week_region_key(),
            ShowcaseRegion,
            FlexItem::fixed(1),
        )
}

fn date_at_time(date: Date, time: Time, offset: UtcOffset) -> OffsetDateTime {
    date.with_time(time).assume_offset(offset)
}
