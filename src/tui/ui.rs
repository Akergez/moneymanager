//! UI rendering using StatefulWidgets

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};

use super::app_state::AppState;
use super::types::Tab;
use super::views::{
    ExpensesView, TopUpsView,
    ExpenseCategoriesView, TopUpCategoriesView,
    PieChartView, BarChartView, LineChartView,
};
use super::forms::{CategoryFormWidget, ExpenseFormWidget, TopUpCategoryFormWidget, TopUpFormWidget};

pub fn draw(frame: &mut Frame, state: &mut AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_tabs(frame, state, chunks[0]);
    draw_content(frame, state, chunks[1]);
    draw_footer(frame, state, chunks[2]);

    // Render active form on top
    if state.category_form.is_active {
        frame.render_stateful_widget(
            CategoryFormWidget::new(),
            frame.area(),
            &mut state.category_form,
        );
    } else if state.expense_form.is_active {
        frame.render_stateful_widget(
            ExpenseFormWidget::new(),
            frame.area(),
            &mut state.expense_form,
        );
    } else if state.top_up_category_form.is_active {
        frame.render_stateful_widget(
            TopUpCategoryFormWidget::new(),
            frame.area(),
            &mut state.top_up_category_form,
        );
    } else if state.top_up_form.is_active {
        frame.render_stateful_widget(
            TopUpFormWidget::new(),
            frame.area(),
            &mut state.top_up_form,
        );
    }
}

fn draw_tabs(frame: &mut Frame, state: &AppState, area: Rect) {
    // Use compact tab titles for narrow screens (mobile-like resolution)
    let tab_titles: Vec<&str> = if area.width < 60 {
        // Ultra-compact for very narrow screens
        vec!["1:EC", "2:Ex", "3:TC", "4:TU", "5:Pie", "6:Bar", "7:Ln"]
    } else if area.width < 80 {
        // Compact for medium screens
        vec!["1:Cat", "2:Exp", "3:Cat", "4:Top", "5:Pie", "6:Bar", "7:Line"]
    } else {
        // Full names for wider screens
        vec![
            "1:Exp.Cat",
            "2:Expenses",
            "3:TopUp.Cat",
            "4:TopUps",
            "5:Pie Chart",
            "6:Bar Chart",
            "7:Line Chart",
        ]
    };

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title("Money Manager"))
        .select(match state.current_tab {
            Tab::ExpenseCategories => 0,
            Tab::Expenses => 1,
            Tab::TopUpCategories => 2,
            Tab::TopUps => 3,
            Tab::ExpensePieChart => 4,
            Tab::ExpenseBarChart => 5,
            Tab::ExpenseLineChart => 6,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    frame.render_widget(tabs, area);
}

fn draw_content(frame: &mut Frame, state: &mut AppState, area: Rect) {
    match state.current_tab {
        Tab::ExpenseCategories => {
            frame.render_stateful_widget(
                ExpenseCategoriesView::new(&state.categories),
                area,
                &mut state.expense_categories_view,
            );
        }
        Tab::Expenses => {
            frame.render_stateful_widget(
                ExpensesView::new(&state.expenses, &state.categories),
                area,
                &mut state.expenses_view,
            );
        }
        Tab::TopUpCategories => {
            frame.render_stateful_widget(
                TopUpCategoriesView::new(&state.top_up_categories),
                area,
                &mut state.top_up_categories_view,
            );
        }
        Tab::TopUps => {
            frame.render_stateful_widget(
                TopUpsView::new(&state.top_ups, &state.top_up_categories),
                area,
                &mut state.top_ups_view,
            );
        }
        Tab::ExpensePieChart => {
            frame.render_stateful_widget(
                PieChartView::new(&state.expenses, &state.categories),
                area,
                &mut state.pie_chart_view,
            );
        }
        Tab::ExpenseBarChart => {
            frame.render_stateful_widget(
                BarChartView::new(&state.expenses),
                area,
                &mut state.bar_chart_view,
            );
        }
        Tab::ExpenseLineChart => {
            frame.render_stateful_widget(
                LineChartView::new(&state.expenses, &state.categories),
                area,
                &mut state.line_chart_view,
            );
        }
    }
}

fn draw_footer(frame: &mut Frame, state: &AppState, area: Rect) {
    let is_narrow = area.width < 60;
    let is_medium = area.width < 80;
    
    let footer_text = if state.category_form.is_active {
        if is_narrow {
            "Enter:OK | Esc:Back"
        } else {
            "Enter: Submit | Esc: Cancel | Type to edit"
        }
    } else if state.expense_form.is_active {
        if is_narrow {
            "Tab:Next | Enter:OK | Esc:Back"
        } else {
            "Tab: Next Field | Enter: Submit | Esc: Cancel | Type to edit"
        }
    } else if state.top_up_category_form.is_active {
        if is_narrow {
            "Enter:OK | Esc:Back"
        } else {
            "Enter: Submit | Esc: Cancel | Type to edit"
        }
    } else if state.top_up_form.is_active {
        if is_narrow {
            "Tab:Next | Enter:OK | Esc:Back"
        } else {
            "Tab: Next Field | Enter: Submit | Esc: Cancel | Type to edit"
        }
    } else {
        match state.current_tab {
            Tab::ExpenseCategories => {
                if is_narrow {
                    "n:New | ↑/↓:Nav | r:Ref | q:Quit"
                } else if is_medium {
                    "n:New | Tab:Switch | ↑/↓:Scroll | r:Refresh | q:Quit"
                } else {
                    "n: New Category | Tab: Switch | ↑/↓: Scroll | r: Refresh | q: Quit"
                }
            }
            Tab::Expenses => {
                if is_narrow {
                    "n:New | ←/→:Sort | ↑/↓:Nav | q:Quit"
                } else if is_medium {
                    "n:New | ←/→:Sort | ↑/↓:Scroll | r:Refresh | q:Quit"
                } else {
                    "n: New Expense | Tab: Switch | ←/→: Sort | ↑/↓: Scroll | r: Refresh | q: Quit"
                }
            }
            Tab::TopUpCategories => {
                if is_narrow {
                    "n:New | ↑/↓:Nav | r:Ref | q:Quit"
                } else if is_medium {
                    "n:New | Tab:Switch | ↑/↓:Scroll | r:Refresh | q:Quit"
                } else {
                    "n: New Category | Tab: Switch | ↑/↓: Scroll | r: Refresh | q: Quit"
                }
            }
            Tab::TopUps => {
                if is_narrow {
                    "n:New | ←/→:Sort | ↑/↓:Nav | q:Quit"
                } else if is_medium {
                    "n:New | ←/→:Sort | ↑/↓:Scroll | r:Refresh | q:Quit"
                } else {
                    "n: New Top Up | Tab: Switch | ←/→: Sort | ↑/↓: Scroll | r: Refresh | q: Quit"
                }
            }
            Tab::ExpensePieChart => {
                if is_narrow {
                    "←/→:Mo | m:Mode | ↑/↓:Nav | q:Quit"
                } else if is_medium {
                    "←/→:Month | m:Mode | ↑/↓:Scroll | r:Refresh | q:Quit"
                } else {
                    "←/→: Month | m: Toggle Mode | ↑/↓: Scroll | r: Refresh | q: Quit"
                }
            }
            Tab::ExpenseLineChart => {
                if is_narrow {
                    "↑/↓:Cat | Space:Tog | ←/→:Mo | q:Quit"
                } else if is_medium {
                    "↑/↓:Cat | Space:Tog | a:All | c:Clear | ←/→:Mo | q:Quit"
                } else {
                    "↑/↓: Select Cat | Space: Toggle | a: All | c: Clear | ←/→: Month | q: Quit"
                }
            }
            _ => {
                if is_narrow {
                    "Tab:Switch | ↑/↓:Nav | q:Quit"
                } else if is_medium {
                    "Tab:Switch | ↑/↓:Scroll | r:Refresh | q:Quit"
                } else {
                    "Tab: Switch | ↑/↓: Scroll | r: Refresh | q: Quit"
                }
            }
        }
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(footer, area);
}

