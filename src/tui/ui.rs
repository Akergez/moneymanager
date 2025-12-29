use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph, Tabs},
    Frame,
};
use crate::tui::app::{App, Tab};
use crate::tui::views;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(frame.area());

    draw_tabs(frame, app, chunks[0]);
    draw_content(frame, app, chunks[1]);
    draw_footer(frame, app, chunks[2]);

    // Render forms on top if in input mode
    match app.input_mode {
        crate::tui::app::InputMode::CreatingCategory => views::forms::render_category_form(frame, app),
        crate::tui::app::InputMode::CreatingExpense => views::forms::render_expense_form(frame, app),
        crate::tui::app::InputMode::Normal => {},
    }
}

fn draw_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let tab_titles = vec![
        "1:Exp.Cat",
        "2:Expenses",
        "3:TopUp.Cat",
        "4:TopUps",
        "5:Pie Chart",
        "6:Bar Chart",
    ];

    let tabs = Tabs::new(tab_titles)
        .block(Block::default().borders(Borders::ALL).title("Money Manager"))
        .select(match app.current_tab {
            Tab::ExpenseCategories => 0,
            Tab::Expenses => 1,
            Tab::TopUpCategories => 2,
            Tab::TopUps => 3,
            Tab::ExpensePieChart => 4,
            Tab::ExpenseBarChart => 5,
        })
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

    frame.render_widget(tabs, area);
}

fn draw_content(frame: &mut Frame, app: &App, area: Rect) {
    match app.current_tab {
        Tab::ExpenseCategories => views::expense_categories::render(frame, app, area),
        Tab::Expenses => views::expenses::render(frame, app, area),
        Tab::TopUpCategories => views::top_up_categories::render(frame, app, area),
        Tab::TopUps => views::top_ups::render(frame, app, area),
        Tab::ExpensePieChart => views::pie_chart::render(frame, app, area),
        Tab::ExpenseBarChart => views::bar_chart::render(frame, app, area),
    }
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    use crate::tui::app::InputMode;

    let footer_text = match app.input_mode {
        InputMode::Normal => match app.current_tab {
            Tab::ExpenseCategories => {
                "n: New Category | Tab: Switch | ↑/↓: Scroll | r: Refresh | q: Quit"
            }
            Tab::Expenses => {
                "n: New Expense | Tab: Switch | ←/→: Sort | ↑/↓: Scroll | r: Refresh | q: Quit"
            }
            Tab::TopUps => {
                "Tab: Switch | ←/→: Sort | ↑/↓: Scroll | r: Refresh | q: Quit"
            }
            Tab::ExpensePieChart => {
                "Tab: Switch | m: Toggle Mode | r: Refresh | q: Quit"
            }
            _ => {
                "Tab: Switch | ↑/↓: Scroll | r: Refresh | q: Quit"
            }
        },
        InputMode::CreatingCategory => {
            "Enter: Submit | Esc: Cancel | Type to edit"
        }
        InputMode::CreatingExpense => {
            "Tab: Next Field | Enter: Submit | Esc: Cancel | Type to edit"
        }
    };

    let footer = Paragraph::new(footer_text)
        .style(Style::default().fg(Color::Cyan))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    frame.render_widget(footer, area);
}
