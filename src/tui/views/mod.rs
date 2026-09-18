//! View widgets using StatefulWidget pattern

pub mod state;
pub mod expenses_widget;
pub mod top_ups_widget;
pub mod expense_categories_widget;
pub mod top_up_categories_widget;
pub mod generic_chart;
pub mod pie_chart_widget;
pub mod bar_chart_widget;
pub mod line_chart_widget;
pub mod top_up_pie_chart_widget;
pub mod top_up_bar_chart_widget;
pub mod accounts_widget;
pub mod transfers_widget;
pub mod confirm_dialog;

// Re-exports for convenience
pub use expenses_widget::{ExpensesView, ExpensesViewState, ViewInputResult, ViewState};
pub use top_ups_widget::{TopUpsView, TopUpsViewState};
pub use expense_categories_widget::{ExpenseCategoriesView, ExpenseCategoriesViewState};
pub use top_up_categories_widget::{TopUpCategoriesView, TopUpCategoriesViewState};
pub use pie_chart_widget::{PieChartView, PieChartViewState};
pub use bar_chart_widget::{BarChartView, BarChartViewState};
pub use line_chart_widget::{LineChartView, LineChartViewState};
pub use top_up_pie_chart_widget::{TopUpPieChartView, TopUpPieChartViewState};
pub use top_up_bar_chart_widget::{TopUpBarChartView, TopUpBarChartViewState};
pub use accounts_widget::{AccountsView, AccountsViewState};
pub use transfers_widget::{TransfersView, TransfersViewState};
pub use confirm_dialog::{ConfirmDialog, ConfirmDialogState, RecordType};

