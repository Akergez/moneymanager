//! Bar chart view for top-ups as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::StatefulWidget,
};
use crate::models::{TopUp, TopUpCategory};
use super::generic_chart::{
    GenericBarChartState, CategoryAction, ChartType,
    render_bar_chart, render_bar_chart_category_selector, find_category_at_position,
    calculate_bar_color_from_categories
};

/// State for the top-up bar chart view (wrapper around generic state)
#[derive(Debug, Clone, Default)]
pub struct TopUpBarChartViewState(pub GenericBarChartState);

impl TopUpBarChartViewState {
    pub fn new() -> Self {
        Self(GenericBarChartState::new())
    }
}

impl super::expenses_widget::ViewState for TopUpBarChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        self.0.handle_bar_chart_input(key)
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        self.0.handle_bar_chart_mouse(mouse, area)
    }
}

/// Widget for rendering the top-up bar chart
pub struct TopUpBarChartView<'a> {
    top_ups: &'a [TopUp],
    categories: &'a [TopUpCategory],
}

impl<'a> TopUpBarChartView<'a> {
    pub fn new(top_ups: &'a [TopUp], categories: &'a [TopUpCategory]) -> Self {
        Self { top_ups, categories }
    }
}

impl<'a> StatefulWidget for TopUpBarChartView<'a> {
    type State = TopUpBarChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),       // Chart
                Constraint::Length(5),    // Category selector
            ])
            .split(area);

        // Process any pending category actions
        if let Some(action) = state.0.pending_category_action.take() {
            match action {
                CategoryAction::Next => state.0.select_next_category(self.categories.len()),
                CategoryAction::Previous => state.0.select_previous_category(self.categories.len()),
                CategoryAction::Toggle => state.0.toggle_selected_category(self.categories),
                CategoryAction::SelectAll => state.0.select_all_categories(self.categories),
                CategoryAction::ClickAt { x, y } => {
                    if let Some(idx) = find_category_at_position(x, y, chunks[1], self.categories) {
                        state.0.category_list_state.select(Some(idx));
                        state.0.toggle_selected_category(self.categories);
                    }
                }
            }
        }

        let monthly_data = state.0.get_monthly_totals(self.top_ups);
        let total: f64 = monthly_data.iter().map(|(_, amt)| *amt).sum();
        
        let title = format!("Monthly Top-Ups (Total: {:.2})", total);
        let title_short = format!("Top-Ups ({:.0})", total);
        
        // Use mixed color from selected categories, or default top-up color
        let default_color = ChartType::TopUp.total_color();
        let bar_color = calculate_bar_color_from_categories(&state.0, self.categories, default_color);

        let _max_scroll = render_bar_chart(
            chunks[0],
            buf,
            &monthly_data,
            state.0.scroll_offset,
            &title_short,
            &title,
            "No monthly top-up data available",
            bar_color,
        );

        render_bar_chart_category_selector(chunks[1], buf, &state.0, self.categories);
    }
}
