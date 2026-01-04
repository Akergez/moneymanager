//! Main application state using StatefulWidgets

use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::layout::Rect;
use diesel::prelude::*;
use crate::models::{Category, Expense, TopUpCategory, TopUp};

use super::types::Tab;
use super::views::{
    ExpensesViewState, TopUpsViewState,
    ExpenseCategoriesViewState, TopUpCategoriesViewState,
    PieChartViewState, BarChartViewState, LineChartViewState,
    ViewInputResult, ViewState,
};
use super::forms::{CategoryFormState, ExpenseFormState, TopUpCategoryFormState, TopUpFormState, FormInputResult};

/// Main application state
pub struct AppState {
    pub running: bool,
    pub current_tab: Tab,

    // Data
    pub categories: Vec<Category>,
    pub expenses: Vec<Expense>,
    pub top_up_categories: Vec<TopUpCategory>,
    pub top_ups: Vec<TopUp>,

    // View states
    pub expenses_view: ExpensesViewState,
    pub top_ups_view: TopUpsViewState,
    pub expense_categories_view: ExpenseCategoriesViewState,
    pub top_up_categories_view: TopUpCategoriesViewState,
    pub pie_chart_view: PieChartViewState,
    pub bar_chart_view: BarChartViewState,
    pub line_chart_view: LineChartViewState,

    // Form states
    pub category_form: CategoryFormState,
    pub expense_form: ExpenseFormState,
    pub top_up_category_form: TopUpCategoryFormState,
    pub top_up_form: TopUpFormState,
}

impl AppState {
    pub fn new(conn: &mut SqliteConnection) -> Result<Self, Box<dyn std::error::Error>> {
        let categories = Category::read_all(conn)?;
        let expenses = Expense::read_all(conn)?;
        let top_up_categories = TopUpCategory::read_all(conn)?;
        let top_ups = TopUp::read_all(conn)?;

        Ok(Self {
            running: true,
            current_tab: Tab::ExpenseCategories,
            categories,
            expenses,
            top_up_categories,
            top_ups,
            expenses_view: ExpensesViewState::new(),
            top_ups_view: TopUpsViewState::new(),
            expense_categories_view: ExpenseCategoriesViewState::new(),
            top_up_categories_view: TopUpCategoriesViewState::new(),
            pie_chart_view: PieChartViewState::new(),
            bar_chart_view: BarChartViewState::new(),
            line_chart_view: LineChartViewState::new(),
            category_form: CategoryFormState::new(),
            expense_form: ExpenseFormState::new(),
            top_up_category_form: TopUpCategoryFormState::new(),
            top_up_form: TopUpFormState::new(),
        })
    }

    pub fn reload_data(&mut self, conn: &mut SqliteConnection) -> Result<(), Box<dyn std::error::Error>> {
        self.categories = Category::read_all(conn)?;
        self.expenses = Expense::read_all(conn)?;
        self.top_up_categories = TopUpCategory::read_all(conn)?;
        self.top_ups = TopUp::read_all(conn)?;
        Ok(())
    }

    pub fn quit(&mut self) {
        self.running = false;
    }

    pub fn next_tab(&mut self) {
        self.current_tab = self.current_tab.next();
    }

    pub fn previous_tab(&mut self) {
        self.current_tab = self.current_tab.previous();
    }

    /// Get mutable reference to the current view's state
    fn current_view_mut(&mut self) -> &mut dyn ViewState {
        match self.current_tab {
            Tab::ExpenseCategories => &mut self.expense_categories_view,
            Tab::Expenses => &mut self.expenses_view,
            Tab::TopUpCategories => &mut self.top_up_categories_view,
            Tab::TopUps => &mut self.top_ups_view,
            Tab::ExpensePieChart => &mut self.pie_chart_view,
            Tab::ExpenseBarChart => &mut self.bar_chart_view,
            Tab::ExpenseLineChart => &mut self.line_chart_view,
        }
    }

    /// Handle all input - delegates to forms or views as appropriate
    pub fn handle_input(&mut self, key: KeyCode, modifiers: KeyModifiers, conn: &mut SqliteConnection) {
        // Handle form input first if a form is active
        if self.category_form.is_active {
            let result = self.category_form.handle_input(key, conn);
            if result == FormInputResult::SubmittedNeedsReload {
                let _ = self.reload_data(conn);
            }
            return;
        }

        if self.expense_form.is_active {
            let result = self.expense_form.handle_input(key, conn);
            if result == FormInputResult::SubmittedNeedsReload {
                let _ = self.reload_data(conn);
            }
            return;
        }

        if self.top_up_category_form.is_active {
            let result = self.top_up_category_form.handle_input(key, conn);
            if result == FormInputResult::SubmittedNeedsReload {
                let _ = self.reload_data(conn);
            }
            return;
        }

        if self.top_up_form.is_active {
            let result = self.top_up_form.handle_input(key, conn);
            if result == FormInputResult::SubmittedNeedsReload {
                let _ = self.reload_data(conn);
            }
            return;
        }

        // Delegate to current view
        let view_result = self.current_view_mut().handle_input(key);

        // Handle view results
        match view_result {
            ViewInputResult::OpenCategoryForm => {
                self.category_form.open();
                return;
            }
            ViewInputResult::OpenExpenseForm => {
                self.expense_form.open(&self.categories);
                return;
            }
            ViewInputResult::OpenTopUpCategoryForm => {
                self.top_up_category_form.open();
                return;
            }
            ViewInputResult::OpenTopUpForm => {
                self.top_up_form.open(&self.top_up_categories);
                return;
            }
            ViewInputResult::Consumed => return,
            ViewInputResult::NotConsumed => {}
        }

        // Global input handling
        match key {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit(),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => self.quit(),

            // Tab navigation
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),

            // Direct tab selection
            KeyCode::Char('1') => self.current_tab = Tab::ExpenseCategories,
            KeyCode::Char('2') => self.current_tab = Tab::Expenses,
            KeyCode::Char('3') => self.current_tab = Tab::TopUpCategories,
            KeyCode::Char('4') => self.current_tab = Tab::TopUps,
            KeyCode::Char('5') => self.current_tab = Tab::ExpensePieChart,
            KeyCode::Char('6') => self.current_tab = Tab::ExpenseBarChart,
            KeyCode::Char('7') => self.current_tab = Tab::ExpenseLineChart,

            // Reload
            KeyCode::Char('r') | KeyCode::Char('R') => { let _ = self.reload_data(conn); }

            _ => {}
        }
    }

    /// Handle mouse input for tab switching and view interactions
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect) {
        // Don't handle mouse if a form is active
        if self.category_form.is_active || self.expense_form.is_active
            || self.top_up_category_form.is_active || self.top_up_form.is_active {
            return;
        }

        // Only handle left button clicks for tabs
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let x = mouse.column;
            let y = mouse.row;

            // Tab bar is in the first 3 rows (height of 3)
            if y < 3 {
                // Calculate tab positions based on screen width
                // Tab bar content starts after the border (x=1)
                let content_start = 1u16;
                let click_x = x.saturating_sub(content_start);

                // Determine tab widths based on screen width (matching ui.rs logic)
                let tab_widths: Vec<u16> = if area.width < 60 {
                    // Ultra-compact: "1:EC", "2:Ex", "3:TC", "4:TU", "5:Pie", "6:Bar", "7:Ln"
                    vec![4, 4, 4, 4, 5, 5, 4]
                } else if area.width < 80 {
                    // Compact: "1:Cat", "2:Exp", "3:Cat", "4:Top", "5:Pie", "6:Bar", "7:Line"
                    vec![5, 5, 5, 5, 5, 5, 6]
                } else {
                    // Full: "1:Exp.Cat", "2:Expenses", "3:TopUp.Cat", "4:TopUps", "5:Pie Chart", "6:Bar Chart", "7:Line Chart"
                    vec![9, 10, 11, 8, 11, 11, 12]
                };

                // Find which tab was clicked
                let mut current_pos = 0u16;
                for (idx, &width) in tab_widths.iter().enumerate() {
                    // Add separator width (tabs have " | " between them, ~3 chars)
                    let separator = if idx > 0 { 3 } else { 0 };
                    let tab_start = current_pos + separator;
                    let tab_end = tab_start + width;

                    if click_x >= tab_start && click_x < tab_end {
                        self.current_tab = match idx {
                            0 => Tab::ExpenseCategories,
                            1 => Tab::Expenses,
                            2 => Tab::TopUpCategories,
                            3 => Tab::TopUps,
                            4 => Tab::ExpensePieChart,
                            5 => Tab::ExpenseBarChart,
                            6 => Tab::ExpenseLineChart,
                            _ => return,
                        };
                        return;
                    }
                    current_pos = tab_end;
                }
            }
        }

        // Calculate content area (between tab bar and footer)
        // Tab bar: rows 0-2, Footer: last 3 rows
        let content_area = Rect {
            x: area.x,
            y: area.y + 3,
            width: area.width,
            height: area.height.saturating_sub(6),
        };

        // Delegate to current view for scroll and other interactions
        let _ = match self.current_tab {
            Tab::ExpenseCategories => self.expense_categories_view.handle_mouse(mouse, content_area),
            Tab::Expenses => self.expenses_view.handle_mouse(mouse, content_area),
            Tab::TopUpCategories => self.top_up_categories_view.handle_mouse(mouse, content_area),
            Tab::TopUps => self.top_ups_view.handle_mouse(mouse, content_area),
            Tab::ExpensePieChart => self.pie_chart_view.handle_mouse(mouse, content_area),
            Tab::ExpenseBarChart => self.bar_chart_view.handle_mouse(mouse, content_area),
            Tab::ExpenseLineChart => self.line_chart_view.handle_mouse(mouse, content_area),
        };
    }
}

