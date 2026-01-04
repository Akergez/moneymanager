//! Line chart view as a StatefulWidget - showing cumulative daily expenses for current month

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, StatefulWidget, Widget},
};
use chrono::{Datelike, NaiveDate};
use std::collections::{HashMap, HashSet};
use crate::models::{Category, Expense};
use crate::tui::utils::color_from_name;

/// Pending category action to be handled during render
#[derive(Debug, Clone, Copy)]
pub enum CategoryAction {
    Next,
    Previous,
    Toggle,
    SelectAll,
}

/// State for the line chart view
#[derive(Debug, Clone)]
pub struct LineChartViewState {
    pub selected_year: i32,
    pub selected_month: u32,
    pub selected_categories: HashSet<Vec<u8>>,
    pub category_list_state: ListState,
    pub scroll_offset: usize,
    pub pending_category_action: Option<CategoryAction>,
}

impl Default for LineChartViewState {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            selected_year: now.year(),
            selected_month: now.month(),
            selected_categories: HashSet::new(),
            category_list_state: ListState::default(),
            scroll_offset: 0,
            pending_category_action: None,
        }
    }
}

impl LineChartViewState {
    pub fn new() -> Self {
        Self::default()
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

    pub fn select_next_category(&mut self, categories_count: usize) {
        if categories_count == 0 {
            return;
        }
        let i = match self.category_list_state.selected() {
            Some(i) => {
                if i >= categories_count - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.category_list_state.select(Some(i));
    }

    pub fn select_previous_category(&mut self, categories_count: usize) {
        if categories_count == 0 {
            return;
        }
        let i = match self.category_list_state.selected() {
            Some(i) => {
                if i == 0 {
                    categories_count - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.category_list_state.select(Some(i));
    }

    pub fn toggle_selected_category(&mut self, categories: &[Category]) {
        if let Some(selected) = self.category_list_state.selected() {
            if let Some(category) = categories.get(selected) {
                if self.selected_categories.contains(&category.id) {
                    self.selected_categories.remove(&category.id);
                } else {
                    self.selected_categories.insert(category.id.clone());
                }
            }
        }
    }

    pub fn select_all_categories(&mut self, categories: &[Category]) {
        self.selected_categories = categories.iter().map(|c| c.id.clone()).collect();
    }

    pub fn deselect_all_categories(&mut self) {
        self.selected_categories.clear();
    }

    pub fn get_month_title(&self) -> String {
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

    /// Get the number of days in the selected month
    fn days_in_month(&self) -> u32 {
        NaiveDate::from_ymd_opt(self.selected_year, self.selected_month + 1, 1)
            .unwrap_or_else(|| NaiveDate::from_ymd_opt(self.selected_year + 1, 1, 1).unwrap())
            .pred_opt()
            .unwrap()
            .day()
    }

    /// Calculate cumulative daily expenses for the selected month and categories
    pub fn get_cumulative_daily_expenses(&self, expenses: &[Expense]) -> Vec<(u32, f64)> {
        let days = self.days_in_month();
        let mut daily_totals: HashMap<u32, f64> = HashMap::new();

        // Filter expenses by month and selected categories
        for expense in expenses {
            if expense.date.year() == self.selected_year
                && expense.date.month() == self.selected_month
                && (self.selected_categories.is_empty()
                    || self.selected_categories.contains(&expense.category_id))
            {
                *daily_totals.entry(expense.date.day()).or_insert(0.0) += expense.amount;
            }
        }

        // Build cumulative sum
        let mut cumulative = 0.0;
        let mut result = Vec::new();
        for day in 1..=days {
            cumulative += daily_totals.get(&day).unwrap_or(&0.0);
            result.push((day, cumulative));
        }

        result
    }
}

impl super::expenses_widget::ViewState for LineChartViewState {
    fn handle_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::KeyCode;
        use super::expenses_widget::ViewInputResult;

        match key {
            KeyCode::Left => {
                self.previous_month();
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                self.next_month();
                ViewInputResult::Consumed
            }
            // Arrow keys for category navigation
            KeyCode::Up => {
                self.pending_category_action = Some(CategoryAction::Previous);
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.pending_category_action = Some(CategoryAction::Next);
                ViewInputResult::Consumed
            }
            KeyCode::Char(' ') => {
                self.pending_category_action = Some(CategoryAction::Toggle);
                ViewInputResult::Consumed
            }
            KeyCode::Char('a') | KeyCode::Char('A') => {
                self.pending_category_action = Some(CategoryAction::SelectAll);
                ViewInputResult::Consumed
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.deselect_all_categories();
                ViewInputResult::Consumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }

    fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: ratatui::layout::Rect) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::{MouseEventKind, MouseButton};
        use super::expenses_widget::ViewInputResult;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.pending_category_action = Some(CategoryAction::Previous);
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.pending_category_action = Some(CategoryAction::Next);
                ViewInputResult::Consumed
            }
            // Click in category list area (left 25 columns) to select/toggle
            MouseEventKind::Down(MouseButton::Left) => {
                let x = mouse.column;
                let y = mouse.row;

                // Category list is in the left 25 columns (after border)
                if x >= area.x && x < area.x + 25 && y > area.y + 1 {
                    // Calculate which category was clicked
                    // Header is row 0, so categories start at row 2 (after title and border)
                    let category_row = (y - area.y - 2) as usize;

                    // Select the category that was clicked
                    self.category_list_state.select(Some(category_row));
                    // Then toggle it
                    self.pending_category_action = Some(CategoryAction::Toggle);
                    return ViewInputResult::Consumed;
                }
                ViewInputResult::NotConsumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }
}

/// Widget for rendering the line chart
pub struct LineChartView<'a> {
    expenses: &'a [Expense],
    categories: &'a [Category],
}

impl<'a> LineChartView<'a> {
    pub fn new(expenses: &'a [Expense], categories: &'a [Category]) -> Self {
        Self { expenses, categories }
    }
}

impl<'a> StatefulWidget for LineChartView<'a> {
    type State = LineChartViewState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        // Process any pending category actions
        if let Some(action) = state.pending_category_action.take() {
            match action {
                CategoryAction::Next => state.select_next_category(self.categories.len()),
                CategoryAction::Previous => state.select_previous_category(self.categories.len()),
                CategoryAction::Toggle => state.toggle_selected_category(self.categories),
                CategoryAction::SelectAll => state.select_all_categories(self.categories),
            }
        }

        // Split into category selector on left and chart on right
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(25), Constraint::Min(40)])
            .split(area);

        // Render category selector
        render_category_selector(chunks[0], buf, state, self.categories);

        // Render line chart
        render_line_chart(chunks[1], buf, state, self.expenses, self.categories);
    }
}

fn render_category_selector(area: Rect, buf: &mut Buffer, state: &mut LineChartViewState, categories: &[Category]) {
    let items: Vec<ListItem> = categories
        .iter()
        .map(|cat| {
            let is_selected = state.selected_categories.contains(&cat.id);
            let checkbox = if is_selected { "[✓] " } else { "[ ] " };
            let color = color_from_name(&cat.name);
            ListItem::new(Line::from(vec![
                Span::styled(checkbox, Style::default().fg(if is_selected { Color::Green } else { Color::Gray })),
                Span::styled(&cat.name, Style::default().fg(color)),
            ]))
        })
        .collect();

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Categories (↑↓:nav Space:toggle)")
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    StatefulWidget::render(list, area, buf, &mut state.category_list_state);
}

fn render_line_chart(area: Rect, buf: &mut Buffer, state: &mut LineChartViewState, expenses: &[Expense], categories: &[Category]) {
    let cumulative_data = state.get_cumulative_daily_expenses(expenses);
    let max_value = cumulative_data.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max);
    let total = cumulative_data.last().map(|(_, v)| *v).unwrap_or(0.0);

    // Calculate line color from selected categories
    let line_color = calculate_mixed_color(state, categories);

    if cumulative_data.is_empty() || max_value == 0.0 {
        let paragraph = Paragraph::new("No expense data for selected categories")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Cumulative Daily Expenses - {}", state.get_month_title()))
            );
        Widget::render(paragraph, area, buf);
        return;
    }

    // Create text-based line chart
    let chart_height = area.height.saturating_sub(4) as usize;
    let chart_width = area.width.saturating_sub(12) as usize;

    if chart_height < 3 || chart_width < 10 {
        let paragraph = Paragraph::new("Area too small for chart")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title("Line Chart"));
        Widget::render(paragraph, area, buf);
        return;
    }

    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:.2}", total), Style::default().fg(line_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];

    // Build ASCII chart
    let days = cumulative_data.len();
    let scale_x = days as f64 / chart_width as f64;

    // Create chart grid
    let mut chart_grid: Vec<Vec<char>> = vec![vec![' '; chart_width]; chart_height];

    // Plot the line
    for x in 0..chart_width {
        let day_idx = (x as f64 * scale_x).floor() as usize;
        if day_idx < cumulative_data.len() {
            let value = cumulative_data[day_idx].1;
            let y = ((value / max_value) * (chart_height - 1) as f64).floor() as usize;
            let y_inverted = chart_height - 1 - y;
            if y_inverted < chart_height {
                chart_grid[y_inverted][x] = '●';
            }
        }
    }

    // Connect points with lines
    for x in 1..chart_width {
        let day_idx_prev = ((x - 1) as f64 * scale_x).floor() as usize;
        let day_idx_curr = (x as f64 * scale_x).floor() as usize;

        if day_idx_prev < cumulative_data.len() && day_idx_curr < cumulative_data.len() {
            let value_prev = cumulative_data[day_idx_prev].1;
            let value_curr = cumulative_data[day_idx_curr].1;

            let y_prev = ((value_prev / max_value) * (chart_height - 1) as f64).floor() as i32;
            let y_curr = ((value_curr / max_value) * (chart_height - 1) as f64).floor() as i32;

            // Fill in intermediate points for steep lines
            let y_min = y_prev.min(y_curr);
            let y_max = y_prev.max(y_curr);
            for y in y_min..=y_max {
                let y_inverted = (chart_height as i32 - 1 - y) as usize;
                if y_inverted < chart_height && chart_grid[y_inverted][x] == ' ' {
                    chart_grid[y_inverted][x] = '│';
                }
            }
        }
    }

    // Add Y-axis labels and chart content
    for (i, row) in chart_grid.iter().enumerate() {
        let value_at_row = max_value * (chart_height - 1 - i) as f64 / (chart_height - 1) as f64;
        let y_label = if i == 0 || i == chart_height - 1 || i == chart_height / 2 {
            format!("{:>8.0} │", value_at_row)
        } else {
            "         │".to_string()
        };

        let row_str: String = row.iter().collect();
        lines.push(Line::from(vec![
            Span::styled(y_label, Style::default().fg(Color::DarkGray)),
            Span::styled(row_str, Style::default().fg(line_color)),
        ]));
    }

    // Add X-axis
    let x_axis_line = "─".repeat(chart_width);
    lines.push(Line::from(vec![
        Span::raw("         └"),
        Span::styled(x_axis_line, Style::default().fg(Color::DarkGray)),
    ]));

    // Add day labels
    let mut day_labels = String::new();
    let step = (days as f64 / 6.0).ceil() as usize;
    for i in 0..chart_width {
        let day_idx = (i as f64 * scale_x).floor() as usize;
        if day_idx < days && (day_idx == 0 || day_idx % step == 0) {
            let day = cumulative_data[day_idx].0;
            day_labels.push_str(&format!("{:<5}", day));
        } else if day_labels.len() < i {
            day_labels.push(' ');
        }
    }
    lines.push(Line::from(vec![
        Span::raw("          "),
        Span::styled(day_labels.trim_end().to_string(), Style::default().fg(Color::DarkGray)),
    ]));

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!("Cumulative Daily Expenses - {} (←/→: Month)", state.get_month_title()))
        )
        .scroll((state.scroll_offset as u16, 0));

    Widget::render(paragraph, area, buf);
}

/// Calculate a mixed color from all selected categories
fn calculate_mixed_color(state: &LineChartViewState, categories: &[Category]) -> Color {
    if state.selected_categories.is_empty() {
        // Default color when no categories selected (shows all)
        return Color::Cyan;
    }

    // Get colors of all selected categories
    let selected_cats: Vec<&Category> = categories
        .iter()
        .filter(|c| state.selected_categories.contains(&c.id))
        .collect();

    if selected_cats.is_empty() {
        return Color::Cyan;
    }

    if selected_cats.len() == 1 {
        // Single category - use its color directly
        return color_from_name(&selected_cats[0].name);
    }

    // Multiple categories - average the RGB values
    let mut total_r: u32 = 0;
    let mut total_g: u32 = 0;
    let mut total_b: u32 = 0;

    for cat in &selected_cats {
        let color = color_from_name(&cat.name);
        if let Color::Rgb(r, g, b) = color {
            total_r += r as u32;
            total_g += g as u32;
            total_b += b as u32;
        }
    }

    let count = selected_cats.len() as u32;
    Color::Rgb(
        (total_r / count) as u8,
        (total_g / count) as u8,
        (total_b / count) as u8,
    )
}

