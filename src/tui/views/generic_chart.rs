//! Generic chart components shared between expense and top-up charts

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Widget},
};
use chrono::{Datelike, NaiveDate};
use std::collections::HashMap;
use crate::tui::utils::color_from_name;

/// Button action for navigation in pie charts
#[derive(Debug, Clone, Copy)]
pub enum ButtonAction {
    PrevMonth,
    NextMonth,
    ToggleMode,
}

/// Display mode for pie charts
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ChartMode {
    #[default]
    SelectedMonth,
    AllTime,
}

impl ChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            ChartMode::SelectedMonth => ChartMode::AllTime,
            ChartMode::AllTime => ChartMode::SelectedMonth,
        }
    }
}

/// Trait for items that can be charted (expenses, top-ups, etc.)
pub trait ChartableItem {
    fn amount(&self) -> f64;
    fn date(&self) -> NaiveDate;
    fn category_id(&self) -> &[u8];
}

/// Trait for categories
pub trait ChartableCategory {
    fn id(&self) -> &[u8];
    fn name(&self) -> &str;
}

// Implement for Expense
impl ChartableItem for crate::models::Expense {
    fn amount(&self) -> f64 { self.amount }
    fn date(&self) -> NaiveDate { self.date }
    fn category_id(&self) -> &[u8] { &self.category_id }
}

// Implement for TopUp
impl ChartableItem for crate::models::TopUp {
    fn amount(&self) -> f64 { self.amount }
    fn date(&self) -> NaiveDate { self.date }
    fn category_id(&self) -> &[u8] { &self.category_id }
}

// Implement for Category
impl ChartableCategory for crate::models::Category {
    fn id(&self) -> &[u8] { &self.id }
    fn name(&self) -> &str { &self.name }
}

// Implement for TopUpCategory
impl ChartableCategory for crate::models::TopUpCategory {
    fn id(&self) -> &[u8] { &self.id }
    fn name(&self) -> &str { &self.name }
}

/// Shared state for pie chart views
#[derive(Debug, Clone)]
pub struct GenericPieChartState {
    pub mode: ChartMode,
    pub scroll_offset: usize,
    pub selected_year: i32,
    pub selected_month: u32,
    pub pending_button_action: Option<ButtonAction>,
}

impl Default for GenericPieChartState {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            mode: ChartMode::default(),
            scroll_offset: 0,
            selected_year: now.year(),
            selected_month: now.month(),
            pending_button_action: None,
        }
    }
}

impl GenericPieChartState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn previous_month(&mut self) {
        if self.selected_month == 1 {
            self.selected_month = 12;
            self.selected_year -= 1;
        } else {
            self.selected_month -= 1;
        }
    }

    pub fn next_month(&mut self) {
        if self.selected_month == 12 {
            self.selected_month = 1;
            self.selected_year += 1;
        } else {
            self.selected_month += 1;
        }
    }

    pub fn get_month_title(&self) -> String {
        match self.mode {
            ChartMode::SelectedMonth => {
                let month_name = match self.selected_month {
                    1 => "January",
                    2 => "February",
                    3 => "March",
                    4 => "April",
                    5 => "May",
                    6 => "June",
                    7 => "July",
                    8 => "August",
                    9 => "September",
                    10 => "October",
                    11 => "November",
                    12 => "December",
                    _ => "Unknown",
                };
                format!("{} {}", month_name, self.selected_year)
            }
            ChartMode::AllTime => "All Time".to_string(),
        }
    }

    /// Calculate totals by category for any chartable items
    pub fn get_by_category<I, C>(
        &self,
        items: &[I],
        categories: &[C],
    ) -> Vec<(String, f64)>
    where
        I: ChartableItem,
        C: ChartableCategory,
    {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = categories
            .iter()
            .map(|c| (c.id().to_vec(), (c.name().to_string(), 0.0)))
            .collect();

        let filtered = self.filter_items(items);

        for item in filtered {
            if let Some(entry) = category_map.get_mut(item.category_id()) {
                entry.1 += item.amount();
            }
        }

        let mut data: Vec<(String, f64)> = category_map
            .into_values()
            .filter(|(_, amount)| *amount > 0.0)
            .collect();

        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        data
    }

    fn filter_items<'a, I: ChartableItem>(&self, items: &'a [I]) -> Vec<&'a I> {
        match self.mode {
            ChartMode::SelectedMonth => {
                items
                    .iter()
                    .filter(|i| i.date().month() == self.selected_month && i.date().year() == self.selected_year)
                    .collect()
            }
            ChartMode::AllTime => items.iter().collect(),
        }
    }

    /// Handle keyboard input for pie chart
    pub fn handle_pie_chart_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::KeyCode;
        use super::expenses_widget::ViewInputResult;

        match key {
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.toggle_mode();
                ViewInputResult::Consumed
            }
            KeyCode::Up => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            KeyCode::Left => {
                self.previous_month();
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                self.next_month();
                ViewInputResult::Consumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }

    /// Handle mouse input for pie chart
    pub fn handle_pie_chart_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::{MouseEventKind, MouseButton};
        use super::expenses_widget::ViewInputResult;

        let button_bar_height = 3u16;
        let button_bar_start = area.y + area.height.saturating_sub(button_bar_height);
        let is_in_button_bar = mouse.row >= button_bar_start;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if is_in_button_bar {
                    let mode_text_len = match self.mode {
                        ChartMode::SelectedMonth => 9,
                        ChartMode::AllTime => 10,
                    };
                    let content_end = area.x + area.width - 1;

                    let next_start = content_end.saturating_sub(8);
                    let mode_end = next_start.saturating_sub(2);
                    let mode_start = mode_end.saturating_sub(mode_text_len as u16);
                    let prev_end = mode_start.saturating_sub(2);
                    let prev_start = prev_end.saturating_sub(8);

                    if mouse.column >= next_start && mouse.column < content_end {
                        self.pending_button_action = Some(ButtonAction::NextMonth);
                        return ViewInputResult::Consumed;
                    }
                    if mouse.column >= mode_start && mouse.column < mode_end {
                        self.pending_button_action = Some(ButtonAction::ToggleMode);
                        return ViewInputResult::Consumed;
                    }
                    if mouse.column >= prev_start && mouse.column < prev_end {
                        self.pending_button_action = Some(ButtonAction::PrevMonth);
                        return ViewInputResult::Consumed;
                    }
                }
                ViewInputResult::NotConsumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }

    /// Process pending button actions
    pub fn process_pending_actions(&mut self) {
        if let Some(action) = self.pending_button_action.take() {
            match action {
                ButtonAction::PrevMonth => self.previous_month(),
                ButtonAction::NextMonth => self.next_month(),
                ButtonAction::ToggleMode => self.toggle_mode(),
            }
        }
    }
}

/// Render the button bar for pie charts
pub fn render_pie_chart_button_bar(area: Rect, buf: &mut Buffer, mode: ChartMode) {
    let mode_text = match mode {
        ChartMode::SelectedMonth => "[Monthly]",
        ChartMode::AllTime => "[All Time]",
    };
    let available_width = area.width.saturating_sub(2) as usize;
    let buttons_width = 8 + 2 + mode_text.len() + 2 + 8;
    let left_padding = available_width.saturating_sub(buttons_width);

    let line = Line::from(vec![
        Span::raw(" ".repeat(left_padding)),
        Span::styled("[◀ Prev]", Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(mode_text, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled("[Next ▶]", Style::default().fg(Color::Cyan)),
    ]);

    let paragraph = Paragraph::new(vec![line])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
        );

    Widget::render(paragraph, area, buf);
}

/// Render pie chart content for any chartable data
pub fn render_pie_chart_content(
    area: Rect,
    buf: &mut Buffer,
    data: &[(String, f64)],
    scroll_offset: usize,
    title: &str,
    empty_message: &str,
) {
    let total: f64 = data.iter().map(|(_, amt)| amt).sum();

    let name_width = if area.width < 60 {
        10
    } else if area.width < 80 {
        15
    } else {
        20
    };

    let max_bar_width = if area.width < 60 {
        15
    } else if area.width < 80 {
        30
    } else {
        50
    };

    if data.is_empty() {
        let paragraph = Paragraph::new(empty_message)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(title));
        Widget::render(paragraph, area, buf);
        return;
    }

    let total_text = format!("{:.2}", total);
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(total_text, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];

    for (name, amount) in data.iter() {
        let percentage = (amount / total) * 100.0;
        let bar_width = ((percentage / 100.0) * max_bar_width as f64) as usize;
        let bar = "█".repeat(bar_width.min(max_bar_width));

        let color = color_from_name(name);

        let display_name = if name.len() > name_width {
            format!("{:.width$}", name, width = name_width - 1)
        } else {
            format!("{:width$}", name, width = name_width)
        };

        lines.push(Line::from(vec![
            Span::styled(display_name, Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(bar, Style::default().fg(color)),
        ]));

        let padding = " ".repeat(name_width + 1);
        let amount_text = if area.width < 60 {
            format!("{:.0} ({:.0}%)", amount, percentage)
        } else {
            format!("{:.2} ({:.1}%)", amount, percentage)
        };

        lines.push(Line::from(vec![
            Span::raw(padding),
            Span::styled(amount_text, Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll_offset as u16, 0));

    Widget::render(paragraph, area, buf);
}

/// Shared state for bar chart views (minimal state)
#[derive(Debug, Clone, Copy, Default)]
pub struct GenericBarChartState;

impl GenericBarChartState {

    /// Calculate monthly totals for any chartable items
    pub fn get_monthly_totals<I: ChartableItem>(items: &[I]) -> Vec<(String, f64)> {
        let mut monthly_map: HashMap<String, f64> = HashMap::new();

        for item in items {
            let month_key = format!("{}-{:02}", item.date().year(), item.date().month());
            *monthly_map.entry(month_key).or_insert(0.0) += item.amount();
        }

        let mut monthly_vec: Vec<_> = monthly_map.into_iter().collect();
        monthly_vec.sort_by(|a, b| a.0.cmp(&b.0));

        // Take last 12 months
        if monthly_vec.len() > 12 {
            monthly_vec.drain(..monthly_vec.len() - 12);
        }

        monthly_vec
    }
}

/// Render bar chart for any monthly data
pub fn render_bar_chart(
    area: Rect,
    buf: &mut Buffer,
    monthly_data: &[(String, f64)],
    title_short: &str,
    title_long: &str,
    empty_message: &str,
) {
    let title = if area.width < 60 { title_short } else { title_long };

    if monthly_data.is_empty() {
        let paragraph = Paragraph::new(empty_message)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(title));
        Widget::render(paragraph, area, buf);
        return;
    }

    let max_amount = monthly_data.iter().map(|(_, amt)| *amt as u64).max().unwrap_or(1);

    let bars: Vec<Bar> = monthly_data
        .iter()
        .map(|(month, amount)| {
            let label = month.split('-').nth(1).unwrap_or(month);
            let color = color_from_name(month);
            Bar::default()
                .value(*amount as u64)
                .label(Line::from(label))
                .style(Style::default().fg(color))
                .value_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(color)
                        .add_modifier(Modifier::BOLD)
                )
        })
        .collect();

    let (bar_width, bar_gap) = if area.width < 60 {
        (3, 0)
    } else if area.width < 80 {
        (4, 1)
    } else {
        (5, 1)
    };

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(title))
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(bar_gap)
        .max(max_amount)
        .style(Style::default().fg(Color::White));

    Widget::render(chart, area, buf);
}

