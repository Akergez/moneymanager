//! Pie chart view for top-ups as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use chrono::Datelike;
use std::collections::HashMap;
use crate::models::{TopUpCategory, TopUp};
use crate::tui::utils::color_from_name;

/// Button action for navigation
#[derive(Debug, Clone, Copy)]
pub enum ButtonAction {
    PrevMonth,
    NextMonth,
    ToggleMode,
}

/// Display mode for the pie chart
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TopUpPieChartMode {
    #[default]
    SelectedMonth,
    AllTime,
}

impl TopUpPieChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            TopUpPieChartMode::SelectedMonth => TopUpPieChartMode::AllTime,
            TopUpPieChartMode::AllTime => TopUpPieChartMode::SelectedMonth,
        }
    }
}

/// State for the top-up pie chart view
#[derive(Debug, Clone)]
pub struct TopUpPieChartViewState {
    pub mode: TopUpPieChartMode,
    pub scroll_offset: usize,
    pub selected_year: i32,
    pub selected_month: u32,
    pub pending_button_action: Option<ButtonAction>,
}

impl Default for TopUpPieChartViewState {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            mode: TopUpPieChartMode::default(),
            scroll_offset: 0,
            selected_year: now.year(),
            selected_month: now.month(),
            pending_button_action: None,
        }
    }
}

impl TopUpPieChartViewState {
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
            TopUpPieChartMode::SelectedMonth => {
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
            TopUpPieChartMode::AllTime => "All Time".to_string(),
        }
    }

    /// Calculate top-up totals by category
    pub fn get_top_up_by_category(
        &self,
        top_ups: &[TopUp],
        categories: &[TopUpCategory],
    ) -> Vec<(String, f64)> {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = categories
            .iter()
            .map(|c| (c.id.clone(), (c.name.clone(), 0.0)))
            .collect();

        let filtered = self.filter_top_ups(top_ups);

        for top_up in filtered {
            if let Some(entry) = category_map.get_mut(&top_up.category_id) {
                entry.1 += top_up.amount;
            }
        }

        let mut data: Vec<(String, f64)> = category_map
            .into_values()
            .filter(|(_, amount)| *amount > 0.0)
            .collect();

        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        data
    }

    fn filter_top_ups<'a>(&self, top_ups: &'a [TopUp]) -> Vec<&'a TopUp> {
        match self.mode {
            TopUpPieChartMode::SelectedMonth => {
                top_ups
                    .iter()
                    .filter(|t| t.date.month() == self.selected_month && t.date.year() == self.selected_year)
                    .collect()
            }
            TopUpPieChartMode::AllTime => top_ups.iter().collect(),
        }
    }
}

impl super::expenses_widget::ViewState for TopUpPieChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
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

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: ratatui::layout::Rect) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::{MouseEventKind, MouseButton};
        use super::expenses_widget::ViewInputResult;

        // Button bar is 3 rows at the bottom
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
                    // Buttons are right-aligned: [◀ Prev]  [Mode]  [Next ▶]
                    let mode_text_len = match self.mode {
                        TopUpPieChartMode::SelectedMonth => 9,  // "[Monthly]"
                        TopUpPieChartMode::AllTime => 10,       // "[All Time]"
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
}

/// Widget for rendering the top-up pie chart
pub struct TopUpPieChartView<'a> {
    top_ups: &'a [TopUp],
    categories: &'a [TopUpCategory],
}

impl<'a> TopUpPieChartView<'a> {
    pub fn new(top_ups: &'a [TopUp], categories: &'a [TopUpCategory]) -> Self {
        Self { top_ups, categories }
    }
}

impl<'a> StatefulWidget for TopUpPieChartView<'a> {
    type State = TopUpPieChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(3),
            ])
            .split(area);

        // Process any pending button actions
        if let Some(action) = state.pending_button_action.take() {
            match action {
                ButtonAction::PrevMonth => state.previous_month(),
                ButtonAction::NextMonth => state.next_month(),
                ButtonAction::ToggleMode => state.toggle_mode(),
            }
        }

        render_pie_chart(chunks[0], buf, state, self.top_ups, self.categories);
        render_button_bar(chunks[1], buf, state);
    }
}

fn render_button_bar(area: Rect, buf: &mut Buffer, state: &TopUpPieChartViewState) {
    let mode_text = match state.mode {
        TopUpPieChartMode::SelectedMonth => "[Monthly]",
        TopUpPieChartMode::AllTime => "[All Time]",
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

fn render_pie_chart(area: Rect, buf: &mut Buffer, state: &mut TopUpPieChartViewState, top_ups: &[TopUp], categories: &[TopUpCategory]) {
    let data = state.get_top_up_by_category(top_ups, categories);
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

    let title = if area.width < 60 {
        format!("{} ({:.0})", state.get_month_title(), total)
    } else {
        format!("Top-Up by Category - {} (Total: {:.2})", state.get_month_title(), total)
    };

    if data.is_empty() {
        let paragraph = Paragraph::new("No top-up data available")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
            );
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
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
        )
        .scroll((state.scroll_offset as u16, 0));

    Widget::render(paragraph, area, buf);
}

