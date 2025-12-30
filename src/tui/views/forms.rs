use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::tui::app::App;
use crate::tui::forms::expense_form::ExpenseField;
use super::common::{centered_rect, styles};

pub fn render_category_form(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 40, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Create New Category")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Min(1),
        ])
        .split(inner);

    // Category name input
    let input_style = Style::default().fg(Color::Yellow);
    let input = Paragraph::new(app.category_form.name.as_str())
        .style(input_style)
        .block(Block::default().borders(Borders::ALL).title("Category Name"));
    frame.render_widget(input, chunks[0]);

    // Instructions
    let instructions = Paragraph::new("Press Enter to submit, Esc to cancel")
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    frame.render_widget(instructions, chunks[1]);

    // Error message
    if let Some(error) = &app.category_form.error_message {
        let error_msg = Paragraph::new(error.as_str())
            .style(Style::default().fg(Color::Red))
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(error_msg, chunks[2]);
    }
}

pub fn render_expense_form(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 60, frame.area());

    frame.render_widget(Clear, area);

    let block = Block::default()
        .title("Create New Expense")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),  // Category
            Constraint::Length(3),  // Amount
            Constraint::Length(3),  // Date
            Constraint::Length(3),  // Comment
            Constraint::Length(3),  // Instructions
            Constraint::Min(1),     // Error/Categories list
        ])
        .split(inner);

    let form = &app.expense_form;

    // Category selector
    let cat_style = if form.current_field == ExpenseField::Category {
        styles::highlight()
    } else {
        styles::normal()
    };
    
    let selected_category_text = form.selected_category_name()
        .unwrap_or("No categories available");
    
    let cat_input = Paragraph::new(selected_category_text)
        .style(cat_style)
        .block(Block::default().borders(Borders::ALL).title("Category (↑/↓ to select, Enter to confirm)"));
    frame.render_widget(cat_input, chunks[0]);

    // Amount input
    let amt_style = if form.current_field == ExpenseField::Amount {
        styles::highlight()
    } else {
        styles::normal()
    };
    let amt_input = Paragraph::new(form.amount.as_str())
        .style(amt_style)
        .block(Block::default().borders(Borders::ALL).title("Amount"));
    frame.render_widget(amt_input, chunks[1]);

    // Date input
    let date_style = if form.current_field == ExpenseField::Date {
        styles::highlight()
    } else {
        styles::normal()
    };
    let date_input = Paragraph::new(form.date.as_str())
        .style(date_style)
        .block(Block::default().borders(Borders::ALL).title("Date (YYYY-MM-DD)"));
    frame.render_widget(date_input, chunks[2]);

    // Comment input
    let comment_style = if form.current_field == ExpenseField::Comment {
        styles::highlight()
    } else {
        styles::normal()
    };
    let comment_input = Paragraph::new(form.comment.as_str())
        .style(comment_style)
        .block(Block::default().borders(Borders::ALL).title("Comment (optional)"));
    frame.render_widget(comment_input, chunks[3]);

    // Instructions
    let instructions_text = if form.current_field == ExpenseField::Category {
        "↑/↓: Select category | Enter/Tab: Next field | Esc: Cancel"
    } else {
        "Tab: Next field | Enter: Submit | Esc: Cancel"
    };
    let instructions = Paragraph::new(instructions_text)
        .style(styles::instruction())
        .alignment(Alignment::Center);
    frame.render_widget(instructions, chunks[4]);

    // Error message or categories list
    if let Some(error) = &form.error_message {
        let error_msg = Paragraph::new(error.as_str())
            .style(styles::error())
            .alignment(Alignment::Center)
            .wrap(Wrap { trim: true });
        frame.render_widget(error_msg, chunks[5]);
    } else if form.current_field == ExpenseField::Category {
        // Show categories as a selectable list when on category field
        let category_names = form.category_names();
        if !category_names.is_empty() {
            let items: Vec<ListItem> = category_names
                .iter()
                .enumerate()
                .map(|(idx, name)| {
                    let style = if idx == form.category_index {
                        styles::highlight()
                    } else {
                        styles::normal()
                    };
                    ListItem::new(*name).style(style)
                })
                .collect();

            let list = List::new(items)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title("Available Categories")
                        .border_style(Style::default().fg(Color::Cyan))
                );
            frame.render_widget(list, chunks[5]);
        }
    }
}


