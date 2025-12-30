//! Pie chart view as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use chrono::Datelike;
use std::collections::HashMap;
use crate::models::{Category, Expense};

/// Color palette for chart elements
const CHART_COLORS: [Color; 10] = [
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Red,
    Color::LightCyan,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
];

/// Display mode for the pie chart
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PieChartMode {
    #[default]
    CurrentMonth,
    AllTime,
}

impl PieChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            PieChartMode::CurrentMonth => PieChartMode::AllTime,
            PieChartMode::AllTime => PieChartMode::CurrentMonth,
        }
    }

    pub fn title(&self) -> &str {
        match self {
            PieChartMode::CurrentMonth => "Current Month",
            PieChartMode::AllTime => "All Time",
        }
    }
}

/// State for the pie chart view
#[derive(Debug, Clone, Default)]
pub struct PieChartViewState {
    pub mode: PieChartMode,
    pub scroll_offset: usize,
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
            PieChartMode::CurrentMonth => {
                let now = chrono::Local::now();
                let (current_month, current_year) = (now.month(), now.year());

                expenses
                    .iter()
                    .filter(|e| e.date.month() == current_month && e.date.year() == current_year)
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
        let data = state.get_expense_by_category(self.expenses, self.categories);
        let total: f64 = data.iter().map(|(_, amt)| amt).sum();

        if data.is_empty() {
            let paragraph = Paragraph::new("No expense data available")
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!("Expense by Category - {} (Total: {:.2})", state.mode.title(), total))
                );
            Widget::render(paragraph, area, buf);
            return;
        }

        // Create text-based bar chart representation
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("Total Expenses: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:.2}", total), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
        ];

        for (i, (name, amount)) in data.iter().enumerate() {
            let percentage = (amount / total) * 100.0;
            let bar_width = (percentage / 2.0) as usize;
            let bar = "█".repeat(bar_width.min(50));

            let color = CHART_COLORS[i % CHART_COLORS.len()];

            lines.push(Line::from(vec![
                Span::styled(format!("{:20}", name), Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(bar, Style::default().fg(color)),
            ]));

            lines.push(Line::from(vec![
                Span::raw("                     "),
                Span::styled(format!("{:.2} ({:.1}%)", amount, percentage), Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(""));
        }

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Expense by Category - {} (Total: {:.2})", state.mode.title(), total))
            )
            .scroll((state.scroll_offset as u16, 0));

        Widget::render(paragraph, area, buf);
    }
}

