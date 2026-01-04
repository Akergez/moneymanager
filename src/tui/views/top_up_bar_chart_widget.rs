//! Bar chart view for top-ups as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::StatefulWidget,
};
use crate::models::TopUp;
use super::generic_chart::{GenericBarChartState, render_bar_chart};

/// State for the top-up bar chart view (wrapper around generic state)
#[derive(Debug, Clone, Default)]
pub struct TopUpBarChartViewState(pub GenericBarChartState);

impl TopUpBarChartViewState {
    pub fn new() -> Self {
        Self(GenericBarChartState)
    }
}

impl super::expenses_widget::ViewState for TopUpBarChartViewState {
    fn handle_input(&mut self, _key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
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

    fn render(self, area: Rect, buf: &mut Buffer, _state: &mut Self::State) {
        let monthly_data = GenericBarChartState::get_monthly_totals(self.top_ups);
        
        render_bar_chart(
            area,
            buf,
            &monthly_data,
            "Monthly Top-Ups",
            "Monthly Top-Ups (Last 12 Months)",
            "No monthly top-up data available",
        );
    }
}
