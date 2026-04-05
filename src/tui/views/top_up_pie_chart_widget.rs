//! Pie chart view for top-ups as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::StatefulWidget,
};
use crate::models::{TopUpCategory, TopUp};
use super::generic_chart::{GenericPieChartState, ChartType, render_pie_chart_button_bar, render_treemap_content};

/// State for the top-up pie chart view (wrapper around generic state)
#[derive(Debug, Clone, Default)]
pub struct TopUpPieChartViewState(pub GenericPieChartState);

impl TopUpPieChartViewState {
    pub fn new() -> Self {
        Self(GenericPieChartState::new())
    }
}

impl super::expenses_widget::ViewState for TopUpPieChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        self.0.handle_pie_chart_input(key)
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        self.0.handle_pie_chart_mouse(mouse, area)
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
        state.0.process_pending_actions();

        // Get data and render
        let all_data = state.0.get_by_category(self.top_ups, self.categories);
        let full_total: f64 = all_data.iter().map(|(_, amt)| amt).sum();

        // Hide the top N largest categories from the treemap
        let visible_start = state.0.hidden_count.min(all_data.len().saturating_sub(1));
        let data = &all_data[visible_start..];

        let hidden = state.0.hidden_count.min(all_data.len().saturating_sub(1));
        let title = if area.width < 60 {
            if hidden > 0 {
                format!("{} ({:.0}, -{hidden})", state.0.get_month_title(), full_total)
            } else {
                format!("{} ({:.0})", state.0.get_month_title(), full_total)
            }
        } else if hidden > 0 {
            format!("Top-Up by Category - {} (Total: {:.2}, -{hidden} hidden)", state.0.get_month_title(), full_total)
        } else {
            format!("Top-Up by Category - {} (Total: {:.2})", state.0.get_month_title(), full_total)
        };

        render_treemap_content(
            chunks[0],
            buf,
            data,
            full_total,
            &title,
            "No top-up data available",
            ChartType::TopUp,
        );

        render_pie_chart_button_bar(chunks[1], buf, state.0.mode, hidden);
    }
}
