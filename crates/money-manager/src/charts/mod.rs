//! All three charts on one screen: how much each month, where the chosen
//! month's money went by category, and how that month built up day by day.
//!
//! The screen holds only what a person chose on it — the month, whether the
//! shares are of that month or of everything, and which categories the bars
//! and the line are of. What is drawn is worked out from the ledger by the
//! plain functions in [`data`].
//!
//! It is worked out once for each thing asked ([`Asked`]) and kept
//! ([`Figures`]), not on every frame. A chart under the pointer animates,
//! and every frame of that draws this whole screen again: going through
//! every record of the ledger for each of them made the charts slower the
//! longer the ledger had been kept.
//!
//! Nothing here can be read only by hovering: a phone has no pointer to hover
//! with. The shares are written out beside the ring, each with its icon, its
//! amount and a bar of its size; the tooltips only repeat them.

mod data;

use std::rc::Rc;

use gpui_adaptive::chart::{BarChart, LineChart, PieChart};
use gpui_adaptive::prim::{Scrolls as _, Surface, column, row, surface};
use gpui_adaptive::{
    ActiveColors as _, ActiveScale as _, Chips, Command, Env, Segments, Text, TileSize, Toggle,
};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Entity, Hsla, Pixels, ScrollHandle, SharedString, Subscription,
    Window, div, point, relative, rems,
};
use money_core::format::{format_amount_short, format_money};
use money_core::store::hex_encode;

use data::{Chosen, DayTotal, Month, MonthTotal};

use crate::book::{Book, CategoryLook, Entry, Mode};
use crate::settings::Settings;
use crate::ui::{self, Lucide};

/// The fewest months the bars show, so a young ledger still has a row of
/// them. An older one has every month since its first record, and scrolls.
const MONTHS_AT_LEAST: usize = 12;
const MONTHS_AT_LEAST_ON_PHONE: usize = 6;

/// How wide a month's bar and its label need to be, in rems. More months
/// than fit at this width make the chart wider than its card, not narrower
/// than this.
const MONTH_WIDTH: f32 = 4.5;

/// A category's share of a period, with everything the screen says about it.
#[derive(Clone)]
struct Share {
    category_id: Vec<u8>,
    name: SharedString,
    look: CategoryLook,
    amount: f64,
    share: f64,
}

/// The two charts that can be of some categories only. Each keeps its own
/// choice: the bars answer "how did this change over the months", the line
/// "how did this month build up", and they are rarely asked about the same
/// categories.
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

/// Everything the figures depend on. Two frames that ask the same get the
/// same figures, so this is compared and the ledger is not read.
#[derive(Clone, PartialEq)]
struct Asked {
    /// Which state of the ledger: counted up every time the book changes.
    ledger: u64,
    mode: Mode,
    month: Month,
    all_time: bool,
    chosen_in_bars: Chosen,
    chosen_in_line: Chosen,
    hide_transfers: bool,
    /// The bars end at this month and the line stops at this day.
    today: chrono::NaiveDate,
    months_at_least: usize,
    /// A category without a colour of its own is drawn in this one.
    muted: Hsla,
}

/// What the charts draw.
struct Figures {
    /// The shares of the chosen month, or of everything.
    shares: Vec<Share>,
    /// The shares of everything, which is what the toggles offer.
    every_share: Vec<Share>,
    totals: Vec<MonthTotal>,
    points: Vec<DayTotal>,
}

pub struct ChartsView {
    book: Entity<Book>,
    /// How many times the book has changed; see [`Asked::ledger`].
    ledger: u64,
    figures: Option<(Asked, Rc<Figures>)>,
    mode: Mode,
    month: Month,
    /// Whether the shares are of everything rather than of the chosen month.
    all_time: bool,
    /// The categories the monthly totals are of; none is all of them.
    chosen_in_bars: Chosen,
    /// The categories the running total is of; none is all of them.
    chosen_in_line: Chosen,
    /// Where the bars are scrolled to.
    bars_scroll: ScrollHandle,
    /// Whether the chosen month still has to be brought into view: it can
    /// only be once the bars have been laid out.
    reveal_month: bool,
    _subscriptions: Vec<Subscription>,
}

impl ChartsView {
    pub fn new(book: Entity<Book>, mode: Mode, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![
            cx.observe(&book, |this, _, cx| {
                this.ledger += 1;
                cx.notify();
            }),
            cx.observe_global::<Settings>(|_, cx| cx.notify()),
        ];
        ChartsView {
            book,
            ledger: 0,
            figures: None,
            mode,
            month: Month::of(crate::today()),
            all_time: false,
            chosen_in_bars: Chosen::new(),
            chosen_in_line: Chosen::new(),
            bars_scroll: ScrollHandle::new(),
            reveal_month: true,
            _subscriptions: subscriptions,
        }
    }

    pub fn set_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.mode != mode {
            self.mode = mode;
            // The categories chosen were of the other kind.
            self.chosen_in_bars.clear();
            self.chosen_in_line.clear();
            self.reveal_month = true;
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
        let muted = cx.colors().muted;
        data::by_category(entries, period)
            .into_iter()
            .map(|slice| {
                let name = book.category_name(self.mode, &slice.category_id);
                let style = book.category_style(self.mode, &slice.category_id);
                Share {
                    look: CategoryLook::of(&style, &slice.category_id, &name, muted),
                    category_id: slice.category_id,
                    name,
                    amount: slice.amount,
                    share: slice.share,
                }
            })
            .collect()
    }

    /// The figures for what is asked now: the ones kept, if that is what
    /// they are of, and otherwise worked out and kept.
    fn figures(&mut self, compact: bool, cx: &Context<Self>) -> Rc<Figures> {
        let asked = Asked {
            ledger: self.ledger,
            mode: self.mode,
            month: self.month,
            all_time: self.all_time,
            chosen_in_bars: self.chosen_in_bars.clone(),
            chosen_in_line: self.chosen_in_line.clone(),
            hide_transfers: Settings::global(cx).hides_transfers(),
            today: crate::today(),
            months_at_least: if compact {
                MONTHS_AT_LEAST_ON_PHONE
            } else {
                MONTHS_AT_LEAST
            },
            muted: cx.colors().muted,
        };
        if let Some((kept, figures)) = &self.figures
            && *kept == asked
        {
            return figures.clone();
        }

        let entries = self.entries(cx);
        let shares = self.shares(&entries, (!asked.all_time).then_some(asked.month), cx);
        // Both sets of toggles offer every category there is a record of,
        // whichever month is chosen: a row of toggles that changed with the
        // month would move everything under it.
        let every_share = if asked.all_time {
            shares.clone()
        } else {
            self.shares(&entries, None, cx)
        };
        // The bars run from the first record to this month, or to the chosen
        // month if that is later.
        let last = asked.month.max(Month::of(asked.today));
        let first = data::first_month(&entries, last, asked.months_at_least).min(asked.month);
        let figures = Rc::new(Figures {
            shares,
            every_share,
            totals: data::by_month(&entries, first, last, &asked.chosen_in_bars),
            points: data::cumulative(&entries, asked.month, &asked.chosen_in_line, asked.today),
        });
        self.figures = Some((asked, figures.clone()));
        figures
    }

    fn step_month(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.month = if forward {
            self.month.next()
        } else {
            self.month.previous()
        };
        self.reveal_month = true;
        cx.notify();
    }

    /// How many categories the bars and the line are each narrowed to, none
    /// being all of them — for a UI scenario to check that a toggle took.
    pub fn chosen_counts(&self) -> (usize, usize) {
        (self.chosen_in_bars.len(), self.chosen_in_line.len())
    }

    fn chosen(&self, chart: Filtered) -> &Chosen {
        match chart {
            Filtered::Bars => &self.chosen_in_bars,
            Filtered::Line => &self.chosen_in_line,
        }
    }

    fn chosen_mut(&mut self, chart: Filtered) -> &mut Chosen {
        match chart {
            Filtered::Bars => &mut self.chosen_in_bars,
            Filtered::Line => &mut self.chosen_in_line,
        }
    }

    /// Scrolls the bars so the chosen month's is in the middle of what
    /// shows, or as near to it as the ends allow.
    fn scroll_to_month(&mut self, index: usize, count: usize, cx: &mut Context<Self>) {
        let reach = self.bars_scroll.max_offset().x;
        let showing = self.bars_scroll.bounds().size.width;
        let middle = (reach + showing) * ((index as f32 + 0.5) / count.max(1) as f32);
        let left = (middle - showing / 2.).max(Pixels::ZERO).min(reach);
        self.bars_scroll
            .set_offset(point(-left, self.bars_scroll.offset().y));
        cx.notify();
    }

    /// "Expenses · Card": what the page's subtitle says.
    pub fn detail(&self, cx: &App) -> String {
        let book = self.book.read(cx);
        let account = book
            .current_account()
            .map_or(String::new(), |account| account.name.clone());
        format!("{} · {account}", self.mode.title())
    }

    /// Which categories a chart is of: "All", and one chip for each category
    /// there is. Pressing a category from "All" leaves that one alone, so
    /// looking at a single category is one press, not one for every other.
    fn render_filters(&self, chart: Filtered, shares: &[Share], cx: &mut Context<Self>) -> Chips {
        let chosen = self.chosen(chart);
        let offered = shares.len();
        let view = cx.entity().downgrade();
        let keys: Vec<(SharedString, Vec<u8>)> = shares
            .iter()
            .map(|share| (hex_encode(&share.category_id).into(), share.category_id.clone()))
            .collect();
        shares
            .iter()
            .fold(Chips::new(chart.key()).all("All"), |chips, share| {
                chips
                    .chip(
                        hex_encode(&share.category_id),
                        share.name.clone(),
                        chosen.contains(&share.category_id),
                    )
                    .dot(share.look.color)
            })
            .on_toggle(move |key, _, cx| {
                let category = key.and_then(|key| keys.iter().find(|(k, _)| k == key));
                view.update(cx, |this, cx| {
                    match category {
                        Some((_, id)) => data::toggle(this.chosen_mut(chart), id, offered),
                        None => this.chosen_mut(chart).clear(),
                    }
                    cx.notify();
                })
                .ok();
            })
    }

    /// The month every chart is anchored to, and the way to another.
    fn render_month_stepper(&self, cx: &mut Context<Self>) -> AnyElement {
        let (back, on) = (cx.entity().downgrade(), cx.entity().downgrade());
        row()
            .flex_none()
            .gap(cx.scale().space_xs)
            .child(
                Command::new("previous-month", "Previous month")
                    .icon(Lucide::ChevronLeft)
                    .on_trigger(move |_, cx| {
                        back.update(cx, |this, cx| this.step_month(false, cx)).ok();
                    })
                    .as_icon_button(),
            )
            // A fixed lane, so the arrows stay where they are from one month
            // to the next and can be pressed again without looking.
            .child(div().w(rems(10.)).child(Text::body(self.month.title()).strong().center()))
            .child(
                Command::new("next-month", "Next month")
                    .icon(Lucide::ChevronRight)
                    .on_trigger(move |_, cx| {
                        on.update(cx, |this, cx| this.step_month(true, cx)).ok();
                    })
                    .as_icon_button(),
            )
            .into_any_element()
    }

    /// One category's share, written out: what the ring shows as an arc is
    /// here an icon, a name, an amount, a percentage and a bar, so that it
    /// can be read without telling two colours apart or pointing at one.
    fn render_share(&self, share: &Share, largest: f64, cx: &App) -> AnyElement {
        let (colors, scale) = (cx.colors(), cx.scale());
        let of_largest = if largest > 0.0 {
            (share.amount / largest) as f32
        } else {
            0.0
        };
        row()
            .gap(scale.space_m)
            .child(ui::category_tile(share.look, TileSize::Medium))
            .child(
                column()
                    .flex_1()
                    .min_w_0()
                    .gap(scale.space_xs)
                    .child(
                        row()
                            .gap(scale.space_m)
                            .child(Text::body(share.name.clone()).truncate().grow())
                            .child(Text::body(format_amount_short(share.amount)).strong())
                            // A fixed lane, so the amounts before it line up.
                            .child(
                                div()
                                    .flex_none()
                                    .w(rems(4.))
                                    .flex()
                                    .justify_end()
                                    .child(Text::small(format!("{:.1}%", share.share * 100.0)).muted()),
                            ),
                    )
                    // Against the largest share, so the first bar is full
                    // and the rest are read against it.
                    .child(
                        div()
                            .w_full()
                            .h(rems(0.375))
                            .rounded_full()
                            .bg(colors.sunken)
                            .child(
                                div()
                                    .h_full()
                                    .w(relative(of_largest))
                                    .rounded_full()
                                    .bg(share.look.color),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn render_shares(
        &self,
        shares: &[Share],
        compact: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let currency = self.book.read(cx).currency().to_string();
        let total: f64 = shares.iter().map(|share| share.amount).sum();
        let view = cx.entity().downgrade();
        // Beside the card's title the group hugs its two segments: a full
        // width would leave the title none (a touch Segments fills by
        // default, for a switch that spans its page).
        let scope = Segments::new("pie-scope", if self.all_time { "all-time" } else { "month" })
            .fill(false)
            .segment("month", "Month")
            .segment("all-time", "All time")
            .on_change(move |key, _, cx| {
                let all_time = key.as_ref() == "all-time";
                view.update(cx, |this, cx| {
                    this.all_time = all_time;
                    cx.notify();
                })
                .ok();
            });

        let body = if shares.is_empty() {
            chart_empty("Nothing recorded in this period")
        } else {
            // The ring's radii are chart geometry the component takes as
            // numbers; they are counted in rems so they scale with the rest.
            let rem = f32::from(window.rem_size());
            let currency_for_tooltip = currency.clone();
            let ring = div()
                .flex_none()
                .relative()
                .size(rems(14.))
                .child(
                    PieChart::new(shares.to_vec())
                        .value(|share| share.amount as f32)
                        .color(|share| share.look.color)
                        .outer_radius(rem * 6.75)
                        .inner_radius(rem * 4.75)
                        .pad_angle(0.03)
                        .tooltip_name(|share| share.name.clone())
                        .tooltip_value(move |share, _, _| {
                            format_money(share.amount, &currency_for_tooltip).into()
                        }),
                )
                // The total, in the hole: the one number the ring is of.
                .child(
                    column()
                        .absolute()
                        .inset_0()
                        .items_center()
                        .justify_center()
                        .child(Text::small("Total").muted())
                        .child(Text::title(format_amount_short(total)))
                        .child(Text::small(currency.clone()).muted()),
                );
            let largest = shares.first().map_or(0.0, |share| share.amount);
            let list = column().flex_1().min_w_0().gap(cx.scale().space_m).children(
                shares
                    .iter()
                    .map(|share| self.render_share(share, largest, cx)),
            );
            if compact {
                column()
                    .gap(cx.scale().space_l)
                    .items_center()
                    .child(ring)
                    .child(list.w_full())
                    .into_any_element()
            } else {
                row()
                    .gap(cx.scale().space_xl)
                    .items_start()
                    .child(ring)
                    .child(list)
                    .into_any_element()
            }
        };

        let hide_transfers = Settings::global(cx).hides_transfers();
        chart_card(
            "By category",
            Some(scope.into_any_element()),
            column()
                .gap(cx.scale().space_l)
                .child(body)
                .child(
                    Toggle::new("include-transfers", "Include transfers", !hide_transfers)
                        .immediate(true)
                        .on_change(|included, _, cx| {
                            Settings::update(cx, |settings| settings.set_hides_transfers(!included));
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
        compact: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let filters = self.render_filters(Filtered::Bars, shares, cx);
        // The same height with bars and without, so choosing a category or
        // a month never makes the card jump.
        let area = div().h(rems(if compact { 16. } else { 20. }));
        let body = if totals.iter().all(|total| total.amount == 0.0) {
            area.child(chart_empty("Nothing recorded in these months"))
                .into_any_element()
        } else {
            let chosen = self.month;
            let series = cx.colors().chart[1];
            let (strong, quiet) = (series, series.opacity(0.35));
            let months = totals.len() as f32;
            area.id("bars")
                .w_full()
                // Sideways only, so that a swipe up the page over the bars
                // still moves the page.
                .scrolls_x()
                .track_scroll(&self.bars_scroll)
                .child(
                    // As wide as the card, or as its months need: whichever
                    // is more. Past the card's width it scrolls.
                    div()
                        .h_full()
                        .w_full()
                        .min_w(rems(MONTH_WIDTH * months))
                        .child(
                            BarChart::new(totals)
                                .band(|total: &MonthTotal| total.month.short())
                                .value(|total: &MonthTotal| total.amount)
                                // The chosen month is the one the other two
                                // charts are about, so it stands out.
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
                        ),
                )
                .into_any_element()
        };
        chart_card(
            "By month",
            None,
            column()
                .gap(cx.scale().space_l)
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

        let area = div().h(rems(16.));
        let body = if points.last().is_none_or(|last| last.total == 0.0) {
            area.child(chart_empty("Nothing to add up in this month"))
        } else {
            let tick_margin = (points.len() / 6).max(1);
            area.child(
                LineChart::new(points)
                    .x(|point: &DayTotal| point.day.to_string())
                    .y(|point: &DayTotal| point.total)
                    .stroke(cx.colors().chart[1])
                    .linear()
                    .tick_margin(tick_margin)
                    .y_axis(true)
                    .y_tick_format(format_amount_short),
            )
        };
        chart_card(
            "Running total by day",
            None,
            column()
                .gap(cx.scale().space_l)
                .child(filters)
                .child(body)
                .into_any_element(),
            cx,
        )
    }
}

/// A chart with its title: one card on the page.
fn chart_card(title: &'static str, trailing: Option<AnyElement>, body: AnyElement, cx: &App) -> AnyElement {
    let scale = cx.scale();
    surface(Surface::Card, cx)
        .min_w_0()
        .p(scale.space_l)
        .gap(scale.space_l)
        .child(
            row()
                .gap(scale.space_m)
                .child(Text::title(title).grow())
                .children(trailing),
        )
        .child(body)
        .into_any_element()
}

/// What a chart's area says when there is nothing to draw. It fills the
/// area it is put in, so an empty chart is as tall as a drawn one.
fn chart_empty(message: &'static str) -> AnyElement {
    div()
        .size_full()
        .min_h(rems(8.))
        .flex()
        .items_center()
        .justify_center()
        .child(Text::body(message).muted())
        .into_any_element()
}

impl Render for ChartsView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let compact = Env::of(window, cx).is_compact();
        let figures = self.figures(compact, cx);
        let Figures {
            shares,
            every_share,
            totals,
            points,
        } = &*figures;
        let (totals, points) = (totals.clone(), points.clone());

        if std::mem::take(&mut self.reveal_month) {
            let count = totals.len();
            let index = totals
                .iter()
                .position(|total| total.month == self.month)
                .unwrap_or(count.saturating_sub(1));
            // The bars have a width only once this frame has laid them out.
            cx.on_next_frame(window, move |this, _, cx| {
                this.scroll_to_month(index, count, cx)
            });
        }

        let by_category = self.render_shares(shares, compact, window, cx);
        let bars = self.render_bars(totals, every_share, compact, cx);
        let line = self.render_line(points, every_share, cx);

        // Each chart has the full width, in every layout. Side by side,
        // none of the three has room to be read: the bars lose their
        // months, the shares their names, the line its days.
        let scale = cx.scale();
        column()
            .w_full()
            .px(scale.space_m)
            .pb(scale.space_l)
            .gap(scale.space_l)
            .child(row().justify_center().child(self.render_month_stepper(cx)))
            .child(bars)
            .child(by_category)
            .child(line)
    }
}
