use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph},
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
    let monthly_data = app.get_monthly_expenses();

    if monthly_data.is_empty() {
        let paragraph = Paragraph::new("No monthly expense data available")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Monthly Expenses"));
        frame.render_widget(paragraph, area);
        return;
    }

    let max_amount = monthly_data.iter().map(|(_, amt)| *amt as u64).max().unwrap_or(1);

    let bars: Vec<Bar> = monthly_data
        .iter()
        .enumerate()
        .map(|(i, (month, amount))| {
            let label = month.split('-').nth(1).unwrap_or(month);
            Bar::default()
                .value(*amount as u64)
                .label(Line::from(label))
                .style(Style::default().fg(COLORS[i % COLORS.len()]))
                .value_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(COLORS[i % COLORS.len()])
                        .add_modifier(Modifier::BOLD)
                )
        })
        .collect();

    let chart = BarChart::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Monthly Expenses (Last 12 Months)")
        )
        .data(BarGroup::default().bars(&bars))
        .bar_width(5)
        .bar_gap(1)
        .max(max_amount)
        .style(Style::default().fg(Color::White));

    frame.render_widget(chart, area);
}

