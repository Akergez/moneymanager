//! Pie chart view for expenses as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::StatefulWidget,
};
use crate::models::{Category, Expense};
use super::generic_chart::{GenericPieChartState, render_pie_chart_button_bar, render_pie_chart_content};

/// State for the expense pie chart view (wrapper around generic state)
#[derive(Debug, Clone, Default)]
pub struct PieChartViewState(pub GenericPieChartState);

impl PieChartViewState {
    pub fn new() -> Self {
        Self(GenericPieChartState::new())
    }
}

impl super::expenses_widget::ViewState for PieChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        self.0.handle_pie_chart_input(key)
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        self.0.handle_pie_chart_mouse(mouse, area)
    }
}

/// Widget for rendering the expense pie chart
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
        let data = state.0.get_by_category(self.expenses, self.categories);
        let total: f64 = data.iter().map(|(_, amt)| amt).sum();
        
        let title = if area.width < 60 {
            format!("{} ({:.0})", state.0.get_month_title(), total)
        } else {
            format!("Expense by Category - {} (Total: {:.2})", state.0.get_month_title(), total)
        };

        render_pie_chart_content(
            chunks[0],
            buf,
            &data,
            state.0.scroll_offset,
            &title,
            "No expense data available",
        );

        render_pie_chart_button_bar(chunks[1], buf, state.0.mode);
    }
}
