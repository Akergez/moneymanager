//! Bar chart view for top-ups as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, StatefulWidget, Widget},
};
use chrono::Datelike;
use std::collections::HashMap;
use crate::models::TopUp;
use crate::tui::utils::color_from_name;

/// State for the top-up bar chart view
#[derive(Debug, Clone, Default)]
pub struct TopUpBarChartViewState {
    // Currently no mutable state needed, but keeping for consistency
}

impl TopUpBarChartViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate monthly top-up totals
    pub fn get_monthly_top_ups(&self, top_ups: &[TopUp]) -> Vec<(String, f64)> {
        let mut monthly_map: HashMap<String, f64> = HashMap::new();

        for top_up in top_ups {
            let month_key = format!("{}-{:02}", top_up.date.year(), top_up.date.month());
            *monthly_map.entry(month_key).or_insert(0.0) += top_up.amount;
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

impl super::expenses_widget::ViewState for TopUpBarChartViewState {
    fn handle_input(&mut self, _key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        // Bar chart has no specific input handling
        super::expenses_widget::ViewInputResult::NotConsumed
    }
}

/// Widget for rendering the top-up bar chart
pub struct TopUpBarChartView<'a> {
    top_ups: &'a [TopUp],
}

impl<'a> TopUpBarChartView<'a> {
    pub fn new(top_ups: &'a [TopUp]) -> Self {
        Self { top_ups }
    }
}

impl<'a> StatefulWidget for TopUpBarChartView<'a> {
    type State = TopUpBarChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let monthly_data = state.get_monthly_top_ups(self.top_ups);

        // Responsive title based on width
        let title = if area.width < 60 {
            "Monthly Top-Ups"
        } else {
            "Monthly Top-Ups (Last 12 Months)"
        };

        if monthly_data.is_empty() {
            let paragraph = Paragraph::new("No monthly top-up data available")
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
            (3, 0)
        } else if area.width < 80 {
            (4, 1)
        } else {
            (5, 1)
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

