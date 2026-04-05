//! Bar chart view for expenses as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::StatefulWidget,
};
use crate::models::{Category, Expense};
use super::generic_chart::{
    GenericBarChartState, CategoryAction, ChartType,
    render_bar_chart, render_bar_chart_category_selector, find_category_at_position,
    calculate_bar_color_from_categories, bar_chart_max_scroll, category_selector_needed_height,
    format_bar_amount,
};

/// State for the expense bar chart view (wrapper around generic state)
#[derive(Debug, Clone, Default)]
pub struct BarChartViewState(pub GenericBarChartState);

impl BarChartViewState {
    pub fn new() -> Self {
        Self(GenericBarChartState::new())
    }
}

impl super::expenses_widget::ViewState for BarChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        self.0.handle_bar_chart_input(key)
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        self.0.handle_bar_chart_mouse(mouse, area)
    }
}

/// Widget for rendering the expense bar chart
pub struct BarChartView<'a> {
    expenses: &'a [Expense],
    categories: &'a [Category],
}

impl<'a> BarChartView<'a> {
    pub fn new(expenses: &'a [Expense], categories: &'a [Category]) -> Self {
        Self { expenses, categories }
    }
}

impl<'a> StatefulWidget for BarChartView<'a> {
    type State = BarChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let selector_height = category_selector_needed_height(self.categories, area.width);
        state.0.category_selector_height = selector_height;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(5),
                Constraint::Length(selector_height),
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

        let monthly_data = state.0.get_monthly_totals(self.expenses);

        // On first render, jump to the last (most recent) month
        if !state.0.initialized {
            let max_lbl = monthly_data.iter()
                .map(|(_, amt)| format_bar_amount(*amt).len())
                .max()
                .unwrap_or(0) as u16;
            state.0.scroll_offset = bar_chart_max_scroll(monthly_data.len(), chunks[0].width, max_lbl);
            state.0.initialized = true;
        }
        let total: f64 = monthly_data.iter().map(|(_, amt)| *amt).sum();

        let title = format!("Monthly Expenses (Total: {:.2})", total);
        let title_short = format!("Expenses ({:.0})", total);

        // Use mixed color from selected categories, or default expense color
        let default_color = ChartType::Expense.total_color();
        let bar_color = calculate_bar_color_from_categories(&state.0, self.categories, default_color);

        let _max_scroll = render_bar_chart(
            chunks[0],
            buf,
            &monthly_data,
            state.0.scroll_offset,
            &title_short,
            &title,
            "No monthly expense data available",
            bar_color,
        );

        render_bar_chart_category_selector(chunks[1], buf, &state.0, self.categories);
    }
}
