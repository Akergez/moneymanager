//! Pie chart view as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use chrono::Datelike;
use std::collections::HashMap;
use crate::models::{Category, Expense};
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
pub enum PieChartMode {
    #[default]
    SelectedMonth,
    AllTime,
}

impl PieChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            PieChartMode::SelectedMonth => PieChartMode::AllTime,
            PieChartMode::AllTime => PieChartMode::SelectedMonth,
        }
    }
}

/// State for the pie chart view
#[derive(Debug, Clone)]
pub struct PieChartViewState {
    pub mode: PieChartMode,
    pub scroll_offset: usize,
    pub selected_year: i32,
    pub selected_month: u32,
    pub pending_button_action: Option<ButtonAction>,
}

impl Default for PieChartViewState {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            mode: PieChartMode::default(),
            scroll_offset: 0,
            selected_year: now.year(),
            selected_month: now.month(),
            pending_button_action: None,
        }
    }
}

impl PieChartViewState {
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
            PieChartMode::SelectedMonth => {
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
            PieChartMode::AllTime => "All Time".to_string(),
        }
    }


    /// Calculate expense totals by category
    pub fn get_expense_by_category(
        &self,
        expenses: &[Expense],
        categories: &[Category],
    ) -> Vec<(String, f64)> {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = categories
            .iter()
            .map(|c| (c.id.clone(), (c.name.clone(), 0.0)))
            .collect();

        let filtered = self.filter_expenses(expenses);

        for expense in filtered {
            if let Some(entry) = category_map.get_mut(&expense.category_id) {
                entry.1 += expense.amount;
            }
        }

        let mut data: Vec<(String, f64)> = category_map
            .into_values()
            .filter(|(_, amount)| *amount > 0.0)
            .collect();

        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        data
    }

    fn filter_expenses<'a>(&self, expenses: &'a [Expense]) -> Vec<&'a Expense> {
        match self.mode {
            PieChartMode::SelectedMonth => {
                expenses
                    .iter()
                    .filter(|e| e.date.month() == self.selected_month && e.date.year() == self.selected_year)
                    .collect()
            }
            PieChartMode::AllTime => expenses.iter().collect(),
        }
    }
}

impl super::expenses_widget::ViewState for PieChartViewState {
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
                    // Calculate button positions from right edge
                    let mode_text_len = match self.mode {
                        PieChartMode::SelectedMonth => 9,  // "[Monthly]"
                        PieChartMode::AllTime => 10,       // "[All Time]"
                    };
                    let content_end = area.x + area.width - 1; // -1 for border

                    // [Next ▶] is at the right end (8 chars)
                    let next_start = content_end.saturating_sub(8);
                    // [Mode] is before Next with 2 char gap
                    let mode_end = next_start.saturating_sub(2);
                    let mode_start = mode_end.saturating_sub(mode_text_len as u16);
                    // [◀ Prev] is before Mode with 2 char gap
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

/// Widget for rendering the pie chart
pub struct PieChartView<'a> {
    expenses: &'a [Expense],
    categories: &'a [Category],
}

impl<'a> PieChartView<'a> {
    pub fn new(expenses: &'a [Expense], categories: &'a [Category]) -> Self {
        Self { expenses, categories }
    }
}

impl<'a> StatefulWidget for PieChartView<'a> {
    type State = PieChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Split into chart on top and button bar at bottom
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),       // Chart
                Constraint::Length(3),    // Button bar
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

        // Render chart
        render_pie_chart(chunks[0], buf, state, self.expenses, self.categories);

        // Render button bar
        render_button_bar(chunks[1], buf, state);
    }
}

fn render_button_bar(area: Rect, buf: &mut Buffer, state: &PieChartViewState) {
    let mode_text = match state.mode {
        PieChartMode::SelectedMonth => "[Monthly]",
        PieChartMode::AllTime => "[All Time]",
    };
    let available_width = area.width.saturating_sub(2) as usize;
    let buttons_width = 8 + 2 + mode_text.len() + 2 + 8; // "[◀ Prev]" + "  " + mode + "  " + "[Next ▶]"
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

fn render_pie_chart(area: Rect, buf: &mut Buffer, state: &mut PieChartViewState, expenses: &[Expense], categories: &[Category]) {
    let data = state.get_expense_by_category(expenses, categories);
    let total: f64 = data.iter().map(|(_, amt)| amt).sum();

    // Responsive column width based on screen size
    let name_width = if area.width < 60 {
        10  // Narrow screens
    } else if area.width < 80 {
        15  // Medium screens
    } else {
        20  // Wide screens
    };

    // Responsive max bar width
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
        format!("Expense by Category - {} (Total: {:.2})", state.get_month_title(), total)
    };

    if data.is_empty() {
        let paragraph = Paragraph::new("No expense data available")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
            );
        Widget::render(paragraph, area, buf);
        return;
    }

    // Create text-based bar chart representation
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

        // Truncate name for narrow screens
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

        // Responsive amount formatting
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

