//! Pie chart view as a StatefulWidget

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, StatefulWidget, Widget},
};
use chrono::Datelike;
use std::collections::HashMap;
use crate::models::{Category, Expense};
use crate::tui::utils::color_from_name;

/// Display mode for the pie chart
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PieChartMode {
    #[default]
    SelectedMonth,
    AllTime,
}

impl PieChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            PieChartMode::SelectedMonth => PieChartMode::AllTime,
            PieChartMode::AllTime => PieChartMode::SelectedMonth,
        }
    }
}

/// State for the pie chart view
#[derive(Debug, Clone)]
pub struct PieChartViewState {
    pub mode: PieChartMode,
    pub scroll_offset: usize,
    pub selected_year: i32,
    pub selected_month: u32,
}

impl Default for PieChartViewState {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            mode: PieChartMode::default(),
            scroll_offset: 0,
            selected_year: now.year(),
            selected_month: now.month(),
        }
    }
}

impl PieChartViewState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn toggle_mode(&mut self) {
        self.mode = self.mode.toggle();
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset += 1;
    }

    pub fn previous_month(&mut self) {
        if self.selected_month == 1 {
            self.selected_month = 12;
            self.selected_year -= 1;
        } else {
            self.selected_month -= 1;
        }
    }

    pub fn next_month(&mut self) {
        if self.selected_month == 12 {
            self.selected_month = 1;
            self.selected_year += 1;
        } else {
            self.selected_month += 1;
        }
    }

    pub fn get_month_title(&self) -> String {
        match self.mode {
            PieChartMode::SelectedMonth => {
                let month_name = match self.selected_month {
                    1 => "January",
                    2 => "February",
                    3 => "March",
                    4 => "April",
                    5 => "May",
                    6 => "June",
                    7 => "July",
                    8 => "August",
                    9 => "September",
                    10 => "October",
                    11 => "November",
                    12 => "December",
                    _ => "Unknown",
                };
                format!("{} {}", month_name, self.selected_year)
            }
            PieChartMode::AllTime => "All Time".to_string(),
        }
    }


    /// Calculate expense totals by category
    pub fn get_expense_by_category(
        &self,
        expenses: &[Expense],
        categories: &[Category],
    ) -> Vec<(String, f64)> {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = categories
            .iter()
            .map(|c| (c.id.clone(), (c.name.clone(), 0.0)))
            .collect();

        let filtered = self.filter_expenses(expenses);

        for expense in filtered {
            if let Some(entry) = category_map.get_mut(&expense.category_id) {
                entry.1 += expense.amount;
            }
        }

        let mut data: Vec<(String, f64)> = category_map
            .into_values()
            .filter(|(_, amount)| *amount > 0.0)
            .collect();

        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        data
    }

    fn filter_expenses<'a>(&self, expenses: &'a [Expense]) -> Vec<&'a Expense> {
        match self.mode {
            PieChartMode::SelectedMonth => {
                expenses
                    .iter()
                    .filter(|e| e.date.month() == self.selected_month && e.date.year() == self.selected_year)
                    .collect()
            }
            PieChartMode::AllTime => expenses.iter().collect(),
        }
    }
}

impl super::expenses_widget::ViewState for PieChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::KeyCode;
        use super::expenses_widget::ViewInputResult;
        
        match key {
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.toggle_mode();
                ViewInputResult::Consumed
            }
            KeyCode::Up => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            KeyCode::Left => {
                self.previous_month();
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                self.next_month();
                ViewInputResult::Consumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, _area: ratatui::layout::Rect) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::MouseEventKind;
        use super::expenses_widget::ViewInputResult;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }
}

/// Widget for rendering the pie chart
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
        let data = state.get_expense_by_category(self.expenses, self.categories);
        let total: f64 = data.iter().map(|(_, amt)| amt).sum();

        // Responsive column width based on screen size
        let name_width = if area.width < 60 {
            10  // Narrow screens
        } else if area.width < 80 {
            15  // Medium screens
        } else {
            20  // Wide screens
        };

        // Responsive max bar width
        let max_bar_width = if area.width < 60 {
            15
        } else if area.width < 80 {
            30
        } else {
            50
        };

        if data.is_empty() {
            let title = if area.width < 60 {
                format!("{} ({:.0})", state.get_month_title(), total)
            } else {
                format!("Expense by Category - {} (Total: {:.2})", state.get_month_title(), total)
            };
            let paragraph = Paragraph::new("No expense data available")
                .alignment(Alignment::Center)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(title)
                );
            Widget::render(paragraph, area, buf);
            return;
        }

        // Create text-based bar chart representation
        let mut lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled("Total: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:.2}", total), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
        ];

        for (name, amount) in data.iter() {
            let percentage = (amount / total) * 100.0;
            let bar_width = ((percentage / 100.0) * max_bar_width as f64) as usize;
            let bar = "█".repeat(bar_width.min(max_bar_width));

            let color = color_from_name(name);

            // Truncate name for narrow screens
            let display_name = if name.len() > name_width {
                format!("{:.width$}", name, width = name_width - 1)
            } else {
                format!("{:width$}", name, width = name_width)
            };

            lines.push(Line::from(vec![
                Span::styled(display_name, Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Span::raw(" "),
                Span::styled(bar, Style::default().fg(color)),
            ]));

            // Responsive amount formatting
            let padding = " ".repeat(name_width + 1);
            let amount_text = if area.width < 60 {
                format!("{:.0} ({:.0}%)", amount, percentage)
            } else {
                format!("{:.2} ({:.1}%)", amount, percentage)
            };

            lines.push(Line::from(vec![
                Span::raw(padding),
                Span::styled(amount_text, Style::default().fg(Color::White)),
            ]));
            lines.push(Line::from(""));
        }

        let title = if area.width < 60 {
            format!("{} ({:.0})", state.get_month_title(), total)
        } else {
            format!("Expense by Category - {} (Total: {:.2})", state.get_month_title(), total)
        };

        let paragraph = Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
            )
            .scroll((state.scroll_offset as u16, 0));

        Widget::render(paragraph, area, buf);
    }
}

