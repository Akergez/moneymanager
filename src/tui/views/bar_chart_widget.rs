//! Bar chart view for expenses as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};
use crate::models::Expense;
use super::generic_chart::{GenericBarChartState, render_bar_chart};

/// State for the expense bar chart view (wrapper around generic state)
#[derive(Debug, Clone, Default)]
pub struct BarChartViewState(pub GenericBarChartState);

impl BarChartViewState {
    pub fn new() -> Self {
        Self(GenericBarChartState)
    }
}

impl super::expenses_widget::ViewState for BarChartViewState {
    fn handle_input(&mut self, _key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        super::expenses_widget::ViewInputResult::NotConsumed
    }
}

/// Widget for rendering the expense bar chart
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

    fn render(self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let monthly_data = GenericBarChartState::get_monthly_totals(self.expenses);

        render_bar_chart(
            area,
            buf,
            &monthly_data,
            "Monthly Expenses",
            "Monthly Expenses (Last 12 Months)",
            "No monthly expense data available",
        );
    }
}
