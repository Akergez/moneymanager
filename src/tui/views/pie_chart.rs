use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::tui::app::App;

const COLORS: [Color; 10] = [
    Color::Cyan,
    Color::Green,
    Color::Yellow,
    Color::Blue,
    Color::Magenta,
    Color::Red,
    Color::LightCyan,
    Color::LightGreen,
    Color::LightYellow,
    Color::LightBlue,
];

pub fn render(frame: &mut Frame, app: &App, area: Rect) {
    let category_data = app.get_expense_by_category();
    
    // Filter out categories with 0 amount and sort by amount
    let mut data: Vec<(String, f64)> = category_data
        .into_iter()
        .filter(|(_, (_, amount))| *amount > 0.0)
        .map(|(_, (name, amount))| (name, amount))
        .collect();
    
    data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let total: f64 = data.iter().map(|(_, amt)| amt).sum();

    if data.is_empty() {
        let paragraph = Paragraph::new("No expense data available")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Expense by Category - {} (Total: {:.2})", 
                        app.pie_chart_mode.title(), total))
            );
        frame.render_widget(paragraph, area);
        return;
    }

    // Create a text-based representation
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Total Expenses: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.2}", total), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];

    for (i, (name, amount)) in data.iter().enumerate() {
        let percentage = (amount / total) * 100.0;
        let bar_width = (percentage / 2.0) as usize; // Scale down for display
        let bar = "█".repeat(bar_width.min(50));
        
        let color = COLORS[i % COLORS.len()];
        
        lines.push(Line::from(vec![
            Span::styled(format!("{:20}", name), Style::default().fg(color).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled(bar, Style::default().fg(color)),
        ]));
        
        lines.push(Line::from(vec![
            Span::raw("                     "),
            Span::styled(format!("{:.2} ({:.1}%)", amount, percentage), Style::default().fg(Color::White)),
        ]));
        lines.push(Line::from(""));
    }

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Expense by Category - {} (Total: {:.2})", 
                    app.pie_chart_mode.title(), total))
        )
        .scroll((app.scroll_offset as u16, 0));

    frame.render_widget(paragraph, area);
}

