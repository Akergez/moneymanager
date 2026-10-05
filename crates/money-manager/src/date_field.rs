//! A date, typed or picked.
//!
//! The field is a line of text first: a date is four or eight key presses,
//! and a calendar is a dozen. The calendar is still there, behind the button
//! at the end of the field, for when the day of the week is what is known.
//! The two are one value — typing a date moves the calendar to it, and
//! picking a day writes it into the field.
//!
//! This takes the place of the component library's date picker, which can
//! only be clicked, and whose calendar is laid out in pixels and so does not
//! fit its own popup once the interface is drawn larger. The calendar here
//! is given its width in rems.

use chrono::{Datelike, NaiveDate};
use gpui_kit::component::button::Button;
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState, Date};
use gpui_kit::component::h_flex;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::popover::Popover;
use gpui_kit::prelude::*;
use gpui_kit::{
    AppContext, Context, Entity, FocusHandle, Focusable, Subscription, Window, div, rems,
};

use crate::ui::Lucide;

/// How a date is written in the field.
const FORMAT: &str = "%Y-%m-%d";

/// How wide the calendar is, in rems: seven days of a size a finger can hit.
const CALENDAR_WIDTH: f32 = 17.0;

/// Reads a date as a person types one. The full form is `2026-10-04`; the
/// rest are what is quicker when the year, or the month too, is the one
/// `near` is in: `04.10.2026`, `4.10`, and a bare `4`.
pub fn parse_date(text: &str, near: NaiveDate) -> Option<NaiveDate> {
    let text = text.trim();
    if let Ok(date) = NaiveDate::parse_from_str(text, FORMAT) {
        return Some(date);
    }
    let parts: Vec<u32> = text
        .split(['.', '/', ' '])
        .map(|part| part.trim().parse().ok())
        .collect::<Option<_>>()?;
    match parts[..] {
        [day] => NaiveDate::from_ymd_opt(near.year(), near.month(), day),
        [day, month] => NaiveDate::from_ymd_opt(near.year(), month, day),
        [day, month, year] => NaiveDate::from_ymd_opt(i32::try_from(year).ok()?, month, day),
        _ => None,
    }
}

pub struct DateField {
    input: Entity<InputState>,
    calendar: Entity<CalendarState>,
    /// The date the field opened on: what a short form such as `4.10` is
    /// read against.
    near: NaiveDate,
    /// Whether the calendar is showing.
    open: bool,
    _subscriptions: Vec<Subscription>,
}

impl DateField {
    pub fn new(date: NaiveDate, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("YYYY-MM-DD")
                .default_value(date.format(FORMAT).to_string())
        });
        let calendar = cx.new(|cx| {
            let mut calendar = CalendarState::new(window, cx);
            calendar.set_date(date, window, cx);
            calendar
        });
        let subscriptions = vec![
            // A date typed whole moves the calendar to it.
            cx.subscribe_in(
                &input,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if !matches!(event, InputEvent::Change) {
                        return;
                    }
                    let typed = parse_date(&input.read(cx).value(), this.near);
                    if let Some(date) = typed {
                        this.calendar
                            .update(cx, |calendar, cx| calendar.set_date(date, window, cx));
                    }
                },
            ),
            // A day picked is written into the field, and the calendar has
            // done what it was opened for.
            cx.subscribe_in(
                &calendar,
                window,
                |this, _, event: &CalendarEvent, window, cx| {
                    let CalendarEvent::Selected(Date::Single(Some(date))) = event else {
                        return;
                    };
                    let text = date.format(FORMAT).to_string();
                    this.input
                        .update(cx, |input, cx| input.set_value(text, window, cx));
                    this.open = false;
                    this.input.update(cx, |input, cx| input.focus(window, cx));
                    cx.notify();
                },
            ),
        ];
        DateField {
            input,
            calendar,
            near: date,
            open: false,
            _subscriptions: subscriptions,
        }
    }

    /// The date in the field, or nothing if what is typed there is not one.
    pub fn date(&self, cx: &gpui_kit::App) -> Option<NaiveDate> {
        parse_date(&self.input.read(cx).value(), self.near)
    }
}

impl Focusable for DateField {
    fn focus_handle(&self, cx: &gpui_kit::App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}

impl Render for DateField {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let calendar = self.calendar.clone();
        let field = cx.entity().downgrade();
        h_flex()
            .w_full()
            .gap_2()
            .child(div().flex_1().min_w_0().child(Input::new(&self.input)))
            .child(
                Popover::new("calendar")
                    // From the button's trailing corner, so it opens back
                    // over the form it belongs to and not past its edge.
                    .anchor(gpui_kit::Anchor::TopRight)
                    .trigger(
                        Button::new("open-calendar")
                            .icon(Lucide::Calendar)
                            .outline()
                            .accessibility_label("Choose from a calendar"),
                    )
                    .open(self.open)
                    .on_open_change(move |open, _, cx| {
                        let open = *open;
                        field
                            .update(cx, |this, cx| {
                                this.open = open;
                                cx.notify();
                            })
                            .ok();
                    })
                    .content(move |_, _, _| {
                        // Its own border and padding would double the
                        // popup's.
                        Calendar::new(&calendar)
                            .border_0()
                            .p_0()
                            .w(rems(CALENDAR_WIDTH))
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
        NaiveDate::from_ymd_opt(year, month, day)
    }

    #[test]
    fn a_date_is_read_whole_or_against_the_month_the_field_opened_on() {
        let near = day(2026, 10, 4).unwrap();
        assert_eq!(parse_date("2026-09-30", near), day(2026, 9, 30));
        assert_eq!(parse_date(" 2026-9-3 ", near), day(2026, 9, 3));
        assert_eq!(parse_date("30.09.2025", near), day(2025, 9, 30));
        // Without a year it is this one; without a month, this one too.
        assert_eq!(parse_date("1.9", near), day(2026, 9, 1));
        assert_eq!(parse_date("17", near), day(2026, 10, 17));
        assert_eq!(parse_date("17/10", near), day(2026, 10, 17));
    }

    #[test]
    fn what_is_not_a_date_is_not_read_as_one() {
        let near = day(2026, 10, 4).unwrap();
        for text in [
            "",
            "soon",
            "31.02",
            "32",
            "0",
            "2026-13-01",
            "1.2.3.4",
            "-5",
        ] {
            assert_eq!(parse_date(text, near), None, "{text:?}");
        }
    }
}
