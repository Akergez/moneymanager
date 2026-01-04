//! Bar chart view as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, StatefulWidget, Widget},
};
use chrono::Datelike;
use std::collections::HashMap;
use crate::models::Expense;
use crate::tui::utils::color_from_name;

/// State for the bar chart view
#[derive(Debug, Clone, Default)]
pub struct BarChartViewState {
    // Currently no mutable state needed, but keeping for consistency
}

impl BarChartViewState {
    pub fn new() -> Self {
        Self::default()
    }


    /// Calculate monthly expense totals
    pub fn get_monthly_expenses(&self, expenses: &[Expense]) -> Vec<(String, f64)> {
        let mut monthly_map: HashMap<String, f64> = HashMap::new();

        for expense in expenses {
            let month_key = format!("{}-{:02}", expense.date.year(), expense.date.month());
            *monthly_map.entry(month_key).or_insert(0.0) += expense.amount;
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

impl super::expenses_widget::ViewState for BarChartViewState {
    fn handle_input(&mut self, _key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        // Bar chart has no specific input handling
        super::expenses_widget::ViewInputResult::NotConsumed
    }
}

/// Widget for rendering the bar chart
pub struct BarChartView<'a> {
    expenses: &'a [Expense],
}

impl<'a> BarChartView<'a> {
    pub fn new(expenses: &'a [Expense]) -> Self {
        Self { expenses }
    }
}

impl<'a> StatefulWidget for BarChartView<'a> {
    type State = BarChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let monthly_data = state.get_monthly_expenses(self.expenses);

        // Responsive title based on width
        let title = if area.width < 60 {
            "Monthly Expenses"
        } else {
            "Monthly Expenses (Last 12 Months)"
        };

        if monthly_data.is_empty() {
            let paragraph = Paragraph::new("No monthly expense data available")
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

        // Responsive bar width and gap
        let (bar_width, bar_gap) = if area.width < 60 {
            (3, 0)  // Compact for narrow screens
        } else if area.width < 80 {
            (4, 1)  // Medium
        } else {
            (5, 1)  // Original for wide screens
        };

        let chart = BarChart::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
            )
            .data(BarGroup::default().bars(&bars))
            .bar_width(bar_width)
            .bar_gap(bar_gap)
            .max(max_amount)
            .style(Style::default().fg(Color::White));

        Widget::render(chart, area, buf);
    }
}

