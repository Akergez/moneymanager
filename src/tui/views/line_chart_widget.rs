//! Line chart view as a StatefulWidget - showing cumulative daily expenses for current month

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, ListState, Paragraph, StatefulWidget, Widget},
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
    ClickAt { x: u16, y: u16 },
}

/// Button action for navigation
#[derive(Debug, Clone, Copy)]
pub enum ButtonAction {
    PrevMonth,
    NextMonth,
}

/// Focus area for the line chart view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineChartFocus {
    #[default]
    Chart,
    CategorySelector,
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
    pub pending_button_action: Option<ButtonAction>,
    pub focus: LineChartFocus,
    /// Actual rendered height of the category selector (set each frame, used by mouse handler)
    pub category_selector_height: u16,
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
            pending_button_action: None,
            focus: LineChartFocus::default(),
            category_selector_height: 5,
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
            // Up/Down to switch focus between chart and category selector
            KeyCode::Up => {
                self.focus = LineChartFocus::Chart;
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.focus = LineChartFocus::CategorySelector;
                ViewInputResult::Consumed
            }
            // Left/Right behavior depends on focus
            KeyCode::Left => {
                match self.focus {
                    LineChartFocus::Chart => self.previous_month(),
                    LineChartFocus::CategorySelector => {
                        self.pending_category_action = Some(CategoryAction::Previous);
                    }
                }
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                match self.focus {
                    LineChartFocus::Chart => self.next_month(),
                    LineChartFocus::CategorySelector => {
                        self.pending_category_action = Some(CategoryAction::Next);
                    }
                }
                ViewInputResult::Consumed
            }
            KeyCode::Char(' ') => {
                if self.focus == LineChartFocus::CategorySelector {
                    self.pending_category_action = Some(CategoryAction::Toggle);
                }
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

        let category_area_height = self.category_selector_height;
        let button_bar_height = 3u16;
        let category_area_start = area.y + area.height.saturating_sub(category_area_height);
        let button_bar_start = category_area_start.saturating_sub(button_bar_height);

        let is_in_category_area = mouse.row >= category_area_start;
        let is_in_button_bar = mouse.row >= button_bar_start && mouse.row < category_area_start;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                if is_in_category_area {
                    self.pending_category_action = Some(CategoryAction::Previous);
                } else {
                    self.previous_month();
                }
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                if is_in_category_area {
                    self.pending_category_action = Some(CategoryAction::Next);
                } else {
                    self.next_month();
                }
                ViewInputResult::Consumed
            }
            // Click to set focus and optionally toggle category or button
            MouseEventKind::Down(MouseButton::Left) => {
                if is_in_category_area {
                    self.focus = LineChartFocus::CategorySelector;
                    // Pass click position to be resolved during render
                    self.pending_category_action = Some(CategoryAction::ClickAt {
                        x: mouse.column,
                        y: mouse.row
                    });
                } else if is_in_button_bar {
                    // Buttons are right-aligned: [◀ Prev]  [Next ▶]
                    // Calculate button positions from right edge
                    let content_end = area.x + area.width - 1; // -1 for border
                    // [Next ▶] is at the right end (8 chars)
                    let next_start = content_end.saturating_sub(8);
                    // [◀ Prev] is before Next with 2 char gap (8 chars)
                    let prev_end = next_start.saturating_sub(2);
                    let prev_start = prev_end.saturating_sub(8);

                    if mouse.column >= next_start && mouse.column < content_end {
                        self.pending_button_action = Some(ButtonAction::NextMonth);
                    } else if mouse.column >= prev_start && mouse.column < prev_end {
                        self.pending_button_action = Some(ButtonAction::PrevMonth);
                    }
                } else {
                    self.focus = LineChartFocus::Chart;
                }
                ViewInputResult::Consumed
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
        let selector_height = line_category_selector_height(self.categories, area.width);
        state.category_selector_height = selector_height;

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(3),
                Constraint::Length(selector_height),
            ])
            .split(area);

        // Process any pending button actions
        if let Some(action) = state.pending_button_action.take() {
            match action {
                ButtonAction::PrevMonth => state.previous_month(),
                ButtonAction::NextMonth => state.next_month(),
            }
        }

        // Process any pending category actions (need chunks[2] for click detection)
        if let Some(action) = state.pending_category_action.take() {
            match action {
                CategoryAction::Next => state.select_next_category(self.categories.len()),
                CategoryAction::Previous => state.select_previous_category(self.categories.len()),
                CategoryAction::Toggle => state.toggle_selected_category(self.categories),
                CategoryAction::SelectAll => state.select_all_categories(self.categories),
                CategoryAction::ClickAt { x, y } => {
                    // Calculate which category was clicked based on position
                    if let Some(idx) = find_category_at_position(x, y, chunks[2], self.categories) {
                        state.category_list_state.select(Some(idx));
                        state.toggle_selected_category(self.categories);
                    }
                }
            }
        }

        // Render line chart
        render_line_chart(chunks[0], buf, state, self.expenses, self.categories);

        // Render button bar
        render_button_bar(chunks[1], buf);

        // Render category selector at bottom
        render_category_selector(chunks[2], buf, state, self.categories);
    }
}

fn line_category_selector_height(categories: &[Category], area_width: u16) -> u16 {
    if categories.is_empty() {
        return 3;
    }
    let content_width = area_width.saturating_sub(2) as usize;
    if content_width == 0 {
        return (categories.len() as u16) + 2;
    }
    let mut current_len = 0usize;
    let mut lines = 1usize;
    for cat in categories {
        let cat_len = 4 + cat.name.chars().count();
        let sep = if current_len > 0 { 2 } else { 0 };
        if current_len > 0 && current_len + sep + cat_len > content_width {
            lines += 1;
            current_len = cat_len;
        } else {
            current_len += sep + cat_len;
        }
    }
    (lines as u16 + 2).max(3)
}

fn render_button_bar(area: Rect, buf: &mut Buffer) {
    let available_width = area.width.saturating_sub(2) as usize;
    let buttons_width = 18; // "[◀ Prev]" + "  " + "[Next ▶]"
    let left_padding = available_width.saturating_sub(buttons_width);

    let line = Line::from(vec![
        Span::raw(" ".repeat(left_padding)),
        Span::styled("[◀ Prev]", Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled("[Next ▶]", Style::default().fg(Color::Cyan)),
    ]);

    let paragraph = Paragraph::new(vec![line])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
        );

    Widget::render(paragraph, area, buf);
}

fn render_category_selector(area: Rect, buf: &mut Buffer, state: &mut LineChartViewState, categories: &[Category]) {
    let is_focused = state.focus == LineChartFocus::CategorySelector;
    let content_width = area.width.saturating_sub(2) as usize; // Account for borders

    // Build lines with category spans, tracking positions for click detection
    let mut lines: Vec<Line> = vec![];
    let mut current_line: Vec<Span> = vec![];
    let mut current_len = 0usize;

    for (i, cat) in categories.iter().enumerate() {
        let is_selected = state.selected_categories.contains(&cat.id);
        let is_highlighted = is_focused && state.category_list_state.selected() == Some(i);
        let checkbox = if is_selected { "✓" } else { " " };
        let color = color_from_name(&cat.name);

        // Build the category text: [X] Name
        let cat_text = format!("[{}] {}", checkbox, cat.name);
        let cat_len = cat_text.chars().count();

        // Separator before item (except first on a line)
        let separator = if current_len > 0 { "  " } else { "" };
        let separator_len = separator.len();

        // Check if we need to wrap
        if current_len > 0 && current_len + separator_len + cat_len > content_width {
            lines.push(Line::from(std::mem::take(&mut current_line)));
            current_len = 0;
        }

        // Add separator if not at start of line
        if current_len > 0 {
            current_line.push(Span::raw("  "));
            current_len += 2;
        }

        // Style for this category
        let style = if is_highlighted {
            Style::default().fg(color).add_modifier(Modifier::REVERSED)
        } else if is_selected {
            Style::default().fg(color).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(color)
        };

        current_line.push(Span::styled(cat_text, style));
        current_len += cat_len;
    }

    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if is_focused {
        "Categories (←/→:nav Space:toggle a:All c:Clear)"
    } else {
        "Categories (↓:focus)"
    };

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title)
        );

    Widget::render(paragraph, area, buf);
}

/// Find which category index was clicked based on the mouse position
fn find_category_at_position(x: u16, y: u16, area: Rect, categories: &[Category]) -> Option<usize> {
    // Check if click is within the category area (accounting for border)
    if x < area.x + 1 || x >= area.x + area.width - 1 {
        return None;
    }
    if y < area.y + 1 || y >= area.y + area.height - 1 {
        return None;
    }

    let content_width = area.width.saturating_sub(2) as usize;
    let content_start_x = (area.x + 1) as usize;
    let content_start_y = (area.y + 1) as usize;

    let click_x = x as usize;
    let click_y = y as usize;

    // Relative position within content area
    let rel_x = click_x - content_start_x;
    let rel_y = click_y - content_start_y;

    // Simulate the same layout logic as render_category_selector
    let mut current_x = 0usize;
    let mut current_y = 0usize;

    for (i, cat) in categories.iter().enumerate() {
        // Category text: "[X] Name" - use chars().count() to match render logic
        // Checkbox is either "✓" (1 char) or " " (1 char)
        let cat_len = 4 + cat.name.chars().count(); // "[" + checkbox + "] " = 4 chars + name chars

        // Separator before item (except first on a line)
        let separator_len = if current_x > 0 { 2 } else { 0 };

        // Check if we need to wrap
        if current_x > 0 && current_x + separator_len + cat_len > content_width {
            current_y += 1;
            current_x = 0;
        }

        // Add separator if not at start of line
        let item_start = if current_x > 0 {
            current_x + 2
        } else {
            current_x
        };
        let item_end = item_start + cat_len;

        // Check if click is within this category
        if rel_y == current_y && rel_x >= item_start && rel_x < item_end {
            return Some(i);
        }

        current_x = item_end;
    }

    None
}

fn render_line_chart(area: Rect, buf: &mut Buffer, state: &mut LineChartViewState, expenses: &[Expense], categories: &[Category]) {
    let cumulative_data = state.get_cumulative_daily_expenses(expenses);
    let max_value = cumulative_data.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max);
    let total = cumulative_data.last().map(|(_, v)| *v).unwrap_or(0.0);

    // Calculate line color from selected categories
    let line_color = calculate_mixed_color(state, categories);

    let is_focused = state.focus == LineChartFocus::Chart;
    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let title = if is_focused {
        format!("Cumulative Daily Expenses - {} (←/→: Month)", state.get_month_title())
    } else {
        format!("Cumulative Daily Expenses - {} (↑:focus)", state.get_month_title())
    };

    if cumulative_data.is_empty() || max_value == 0.0 {
        let paragraph = Paragraph::new("No expense data for selected categories")
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title)
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

    // Total line at top
    let total_text = format!("{:.2}", total);
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(total_text, Style::default().fg(line_color).add_modifier(Modifier::BOLD)),
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
                .border_style(border_style)
                .title(title)
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

