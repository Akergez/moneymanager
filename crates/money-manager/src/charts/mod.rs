//! All three charts on one screen: where the money went by category, how
//! much each month, and how the chosen month built up day by day.
//!
//! The screen holds only what a person chose on it — the month, whether the
//! pie is of that month or of everything, and which categories the line
//! leaves out. What is drawn is worked out from the ledger on every frame by
//! the plain functions in [`data`].

mod data;

use std::collections::HashSet;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::chart::{BarChart, LineChart, PieChart};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme, Selectable, Sizable, StyledExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, Context, Entity, Hsla, SharedString, Subscription, Window, div};
use money_core::format::{format_amount_short, format_money};

use data::{DayTotal, Month, MonthTotal};

use crate::book::{self, Book, Entry, Mode};
use crate::settings::Settings;
use crate::shell::Layout;
use crate::ui::{self, Lucide};

/// How many months the bar chart shows, ending with the chosen one.
const MONTHS_SHOWN: usize = 12;
const MONTHS_SHOWN_ON_PHONE: usize = 6;

/// A slice of the pie, with everything the legend says about it.
#[derive(Clone)]
struct Share {
    category_id: Vec<u8>,
    name: SharedString,
    color: Hsla,
    amount: f64,
    share: f64,
}

/// The two charts that can leave categories out. Each keeps its own choice:
/// the bars answer "how did this change over the months", the line "how did
/// this month build up", and they are rarely asked about the same categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Filtered {
    Bars,
    Line,
}

impl Filtered {
    /// What its toggles' ids begin with.
    fn key(self) -> &'static str {
        match self {
            Filtered::Bars => "bars",
            Filtered::Line => "line",
        }
    }
}

pub struct ChartsView {
    book: Entity<Book>,
    mode: Mode,
    month: Month,
    /// Whether the pie is of everything rather than of the chosen month.
    all_time: bool,
    /// Categories the monthly totals leave out.
    hidden_in_bars: HashSet<Vec<u8>>,
    /// Categories the running total leaves out.
    hidden_in_line: HashSet<Vec<u8>>,
    _subscriptions: Vec<Subscription>,
}

impl ChartsView {
    pub fn new(book: Entity<Book>, mode: Mode, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&book, |_, _, cx| cx.notify()),
            cx.observe_global::<Settings>(|_, cx| cx.notify()),
        ];
        ChartsView {
            book,
            mode,
            month: Month::of(crate::today()),
            all_time: false,
            hidden_in_bars: HashSet::new(),
            hidden_in_line: HashSet::new(),
            _subscriptions: subscriptions,
        }
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            // The hidden categories were of the other kind.
            self.hidden_in_bars.clear();
            self.hidden_in_line.clear();
            cx.notify();
        }
    }

    /// The records the charts are about: the account's, of the current
    /// kind, with or without transfer legs.
    fn entries(&self, cx: &Context<Self>) -> Vec<Entry> {
        let hide_transfers = Settings::global(cx).hides_transfers();
        self.book
            .read(cx)
            .entries(self.mode)
            .into_iter()
            .filter(|entry| !(hide_transfers && entry.is_transfer))
            .collect()
    }

    /// The categories' shares of one month, or of everything for `None`.
    fn shares(&self, entries: &[Entry], period: Option<Month>, cx: &Context<Self>) -> Vec<Share> {
        let book = self.book.read(cx);
        let muted = cx.theme().muted_foreground;
        data::by_category(entries, period)
            .into_iter()
            .map(|slice| {
                let name = book.category_name(self.mode, &slice.category_id);
                let style = book.category_style(self.mode, &slice.category_id);
                Share {
                    color: book::color_of(&style, &slice.category_id, &name, muted),
                    category_id: slice.category_id,
                    name,
                    amount: slice.amount,
                    share: slice.share,
                }
            })
            .collect()
    }

    fn step_month(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.month = if forward {
            self.month.next()
        } else {
            self.month.previous()
        };
        cx.notify();
    }

    /// How many categories the bars and the line each leave out, for a UI
    /// scenario to check that a toggle took.
    pub fn hidden_counts(&self) -> (usize, usize) {
        (self.hidden_in_bars.len(), self.hidden_in_line.len())
    }

    fn hidden(&self, chart: Filtered) -> &HashSet<Vec<u8>> {
        match chart {
            Filtered::Bars => &self.hidden_in_bars,
            Filtered::Line => &self.hidden_in_line,
        }
    }

    fn toggle_category(&mut self, chart: Filtered, category_id: &[u8], cx: &mut Context<Self>) {
        let hidden = match chart {
            Filtered::Bars => &mut self.hidden_in_bars,
            Filtered::Line => &mut self.hidden_in_line,
        };
        if !hidden.remove(category_id) {
            hidden.insert(category_id.to_vec());
        }
        cx.notify();
    }

    /// Which categories count towards a chart: one toggle each, for the
    /// categories that have anything in the period the chart covers. A
    /// category with nothing there has nothing to leave out.
    fn render_filters(
        &self,
        chart: Filtered,
        shares: &[Share],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let hidden = self.hidden(chart);
        h_flex()
            .flex_wrap()
            .gap_2()
            .children(shares.iter().map(|share| {
                let category_id = share.category_id.clone();
                let shown = !hidden.contains(&category_id);
                Button::new(SharedString::from(format!(
                    "{}-category-{}",
                    chart.key(),
                    money_core::store::hex_encode(&category_id)
                )))
                // The category's own colour, as in the pie's legend; faded
                // while the category is left out.
                .child(
                    div()
                        .flex_none()
                        .size_2()
                        .rounded_full()
                        .bg(share.color.opacity(if shown { 1.0 } else { 0.3 })),
                )
                .child(share.name.clone())
                .small()
                .outline()
                .selected(shown)
                .on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.toggle_category(chart, &category_id, cx)
                    }),
                )
            }))
            .into_any_element()
    }

    /// The month every chart is anchored to, and the way to another.
    fn render_month_stepper(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .flex_none()
            .gap_1()
            .child(
                Button::new("previous-month")
                    .icon(Lucide::ChevronLeft)
                    .ghost()
                    .small()
                    .accessibility_label("Previous month")
                    .on_click(cx.listener(|this, _, _, cx| this.step_month(false, cx))),
            )
            .child(div().text_sm().font_medium().child(self.month.title()))
            .child(
                Button::new("next-month")
                    .icon(Lucide::ChevronRight)
                    .ghost()
                    .small()
                    .accessibility_label("Next month")
                    .on_click(cx.listener(|this, _, _, cx| this.step_month(true, cx))),
            )
            .into_any_element()
    }

    fn render_pie(
        &self,
        shares: &[Share],
        layout: Layout,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let currency = self.book.read(cx).currency().to_string();
        let total: f64 = shares.iter().map(|share| share.amount).sum();
        let scope = TabBar::new("pie-scope")
            .segmented()
            .small()
            .selected_index(usize::from(self.all_time))
            .child(Tab::new().label("Month"))
            .child(Tab::new().label("All time"))
            .on_click(cx.listener(|this, index: &usize, _, cx| {
                this.all_time = *index == 1;
                cx.notify();
            }));

        let body = if shares.is_empty() {
            chart_empty("Nothing recorded in this period", cx)
        } else {
            // The ring's radii are chart geometry the component takes in
            // pixels; they are derived from the rem so they zoom with it.
            let rem = f32::from(window.rem_size());
            let currency_for_tooltip = currency.clone();
            let pie = div().flex_none().size_48().child(
                PieChart::new(shares.to_vec())
                    .value(|share| share.amount as f32)
                    .color(|share| share.color)
                    .outer_radius(rem * 5.5)
                    .inner_radius(rem * 3.4)
                    .pad_angle(0.02)
                    .tooltip_name(|share| share.name.clone())
                    .tooltip_value(move |share, _, _| {
                        format_money(share.amount, &currency_for_tooltip).into()
                    }),
            );
            let muted = cx.theme().muted_foreground;
            let legend = v_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .children(shares.iter().map(|share| {
                    h_flex()
                        .gap_2()
                        .child(div().flex_none().size_2().rounded_full().bg(share.color))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(share.name.clone()),
                        )
                        // Whole units and no symbol: the total under the
                        // rows names the currency once, and the names above
                        // need the room more.
                        .child(div().flex_none().child(format_amount_short(share.amount)))
                        // A fixed lane, so the amounts before it line up.
                        .child(
                            div()
                                .flex_none()
                                .w_12()
                                .text_right()
                                .text_sm()
                                .text_color(muted)
                                .child(format!("{:.1}%", share.share * 100.0)),
                        )
                }))
                .child(
                    h_flex()
                        .pt_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .font_medium()
                        .child(div().flex_1().child("Total"))
                        .child(format_money(total, &currency)),
                );
            if layout.is_phone() {
                v_flex()
                    .gap_4()
                    .items_center()
                    .child(pie)
                    .child(legend.w_full())
                    .into_any_element()
            } else {
                h_flex().gap_6().child(pie).child(legend).into_any_element()
            }
        };

        let hide_transfers = Settings::global(cx).hides_transfers();
        chart_card(
            "By category",
            Some(scope.into_any_element()),
            v_flex()
                .gap_4()
                .child(body)
                .child(
                    Switch::new("include-transfers")
                        .label("Include transfers")
                        .small()
                        .checked(!hide_transfers)
                        .on_click(|included, _, cx| {
                            let hidden = !*included;
                            Settings::update(cx, |settings| settings.set_hides_transfers(hidden));
                        }),
                )
                .into_any_element(),
            cx,
        )
    }

    fn render_bars(
        &self,
        totals: Vec<MonthTotal>,
        shares: &[Share],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = self.render_filters(Filtered::Bars, shares, cx);
        let body = if totals.iter().all(|total| total.amount == 0.0) {
            chart_empty("Nothing recorded in these months", cx)
        } else {
            let chosen = self.month;
            let (strong, quiet) = (cx.theme().chart_2, cx.theme().chart_2.opacity(0.35));
            div()
                .h_56()
                .child(
                    BarChart::new(totals)
                        .band(|total: &MonthTotal| total.month.short())
                        .value(|total: &MonthTotal| total.amount)
                        // The chosen month is the one the other two charts
                        // are about, so it is the one that stands out.
                        .fill(
                            move |total: &MonthTotal, _, _, _| {
                                if total.month == chosen { strong } else { quiet }
                            },
                        )
                        .label(|total: &MonthTotal| {
                            if total.amount > 0.0 {
                                format_amount_short(total.amount)
                            } else {
                                String::new()
                            }
                        }),
                )
                .into_any_element()
        };
        chart_card(
            "By month",
            None,
            v_flex()
                .gap_4()
                .child(filters)
                .child(body)
                .into_any_element(),
            cx,
        )
    }

    fn render_line(
        &self,
        points: Vec<DayTotal>,
        shares: &[Share],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = self.render_filters(Filtered::Line, shares, cx);

        let body = if points.last().is_none_or(|last| last.total == 0.0) {
            chart_empty("Nothing to add up in this month", cx)
        } else {
            let tick_margin = (points.len() / 6).max(1);
            div()
                .h_56()
                .child(
                    LineChart::new(points)
                        .x(|point: &DayTotal| point.day.to_string())
                        .y(|point: &DayTotal| point.total)
                        .stroke(cx.theme().chart_2)
                        .linear()
                        .tick_margin(tick_margin)
                        .y_axis(true)
                        .y_tick_format(format_amount_short),
                )
                .into_any_element()
        };
        chart_card(
            "Running total by day",
            None,
            v_flex()
                .gap_4()
                .child(filters)
                .child(body)
                .into_any_element(),
            cx,
        )
    }
}

/// A chart with its title: one bordered region, no shadow — the charts are
/// the page, not something lifted off it.
fn chart_card(
    title: &'static str,
    trailing: Option<AnyElement>,
    body: AnyElement,
    cx: &gpui_kit::App,
) -> AnyElement {
    v_flex()
        .min_w_0()
        .p_4()
        .gap_4()
        .border_1()
        .border_color(cx.theme().border)
        .rounded(cx.theme().radius_lg)
        .child(
            h_flex()
                .gap_3()
                .child(div().flex_1().min_w_0().font_semibold().child(title))
                .children(trailing),
        )
        .child(body)
        .into_any_element()
}

fn chart_empty(message: &'static str, cx: &gpui_kit::App) -> AnyElement {
    div()
        .h_32()
        .flex()
        .items_center()
        .justify_center()
        .text_sm()
        .text_color(cx.theme().muted_foreground)
        .child(message)
        .into_any_element()
}

impl Render for ChartsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let layout = Layout::for_width(f32::from(window.viewport_size().width));
        let entries = self.entries(cx);
        let shares = self.shares(&entries, (!self.all_time).then_some(self.month), cx);
        // The line's toggles are of the month's categories whatever the pie
        // is showing.
        let month_shares = if self.all_time {
            self.shares(&entries, Some(self.month), cx)
        } else {
            shares.clone()
        };
        let months = if layout.is_phone() {
            MONTHS_SHOWN_ON_PHONE
        } else {
            MONTHS_SHOWN
        };
        // The bars' toggles are of every category the months shown hold.
        let window_shares =
            self.shares(&data::within_months(&entries, self.month, months), None, cx);
        let totals = data::by_month(&entries, self.month, months, &self.hidden_in_bars);
        let points = data::cumulative(&entries, self.month, &self.hidden_in_line, crate::today());

        let pie = self.render_pie(&shares, layout, window, cx);
        let bars = self.render_bars(totals, &window_shares, cx);
        let line = self.render_line(points, &month_shares, cx);

        // Side by side where both fit, one under the other where they do
        // not; the running total always has the full width.
        let top = if layout == Layout::Desktop {
            div().grid().grid_cols(2).gap_4().child(pie).child(bars)
        } else {
            v_flex().gap_4().child(pie).child(bars)
        };

        let detail = format!("{} · {}", self.mode.title(), {
            let book = self.book.read(cx);
            book.current_account()
                .map_or(String::new(), |account| account.name.clone())
        });
        v_flex()
            .size_full()
            .gap_3()
            .when(layout.is_phone(), |screen| screen.px_3().pt_1())
            .when(!layout.is_phone(), |screen| screen.p_4())
            .child(ui::heading(
                "Charts",
                detail,
                Some(self.render_month_stepper(cx)),
                cx,
            ))
            .child(
                // The charts scroll together, under a heading that stays.
                div()
                    .id("charts")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .child(v_flex().gap_4().pb_4().child(top).child(line)),
            )
    }
}
