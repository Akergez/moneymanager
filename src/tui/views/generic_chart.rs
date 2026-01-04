//! Generic chart components shared between expense and top-up charts

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Bar, BarChart, BarGroup, Block, Borders, ListState, Paragraph, Widget},
};
use chrono::{Datelike, NaiveDate};
use std::collections::{HashMap, HashSet};
use crate::tui::utils::color_from_name;

/// Chart type for different color schemes
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ChartType {
    #[default]
    Expense,
    TopUp,
}

impl ChartType {
    pub fn total_color(&self) -> Color {
        match self {
            ChartType::Expense => Color::Rgb(178, 34, 34), // Brick red for expenses
            ChartType::TopUp => Color::Rgb(65, 105, 225),  // Royal blue for top-ups
        }
    }
}

/// Pending category action to be handled during render
#[derive(Debug, Clone, Copy)]
pub enum CategoryAction {
    Next,
    Previous,
    Toggle,
    SelectAll,
    ClickAt { x: u16, y: u16 },
}

/// Button action for navigation in pie charts
#[derive(Debug, Clone, Copy)]
pub enum ButtonAction {
    PrevMonth,
    NextMonth,
    ToggleMode,
}

/// Display mode for pie charts
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum ChartMode {
    #[default]
    SelectedMonth,
    AllTime,
}

impl ChartMode {
    pub fn toggle(&self) -> Self {
        match self {
            ChartMode::SelectedMonth => ChartMode::AllTime,
            ChartMode::AllTime => ChartMode::SelectedMonth,
        }
    }
}

/// Trait for items that can be charted (expenses, top-ups, etc.)
pub trait ChartableItem {
    fn amount(&self) -> f64;
    fn date(&self) -> NaiveDate;
    fn category_id(&self) -> &[u8];
}

/// Trait for categories
pub trait ChartableCategory {
    fn id(&self) -> &[u8];
    fn name(&self) -> &str;
}

// Implement for Expense
impl ChartableItem for crate::models::Expense {
    fn amount(&self) -> f64 { self.amount }
    fn date(&self) -> NaiveDate { self.date }
    fn category_id(&self) -> &[u8] { &self.category_id }
}

// Implement for TopUp
impl ChartableItem for crate::models::TopUp {
    fn amount(&self) -> f64 { self.amount }
    fn date(&self) -> NaiveDate { self.date }
    fn category_id(&self) -> &[u8] { &self.category_id }
}

// Implement for Category
impl ChartableCategory for crate::models::Category {
    fn id(&self) -> &[u8] { &self.id }
    fn name(&self) -> &str { &self.name }
}

// Implement for TopUpCategory
impl ChartableCategory for crate::models::TopUpCategory {
    fn id(&self) -> &[u8] { &self.id }
    fn name(&self) -> &str { &self.name }
}

/// Shared state for pie chart views
#[derive(Debug, Clone)]
pub struct GenericPieChartState {
    pub mode: ChartMode,
    pub scroll_offset: usize,
    pub selected_year: i32,
    pub selected_month: u32,
    pub pending_button_action: Option<ButtonAction>,
}

impl Default for GenericPieChartState {
    fn default() -> Self {
        let now = chrono::Local::now();
        Self {
            mode: ChartMode::default(),
            scroll_offset: 0,
            selected_year: now.year(),
            selected_month: now.month(),
            pending_button_action: None,
        }
    }
}

impl GenericPieChartState {
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
            ChartMode::SelectedMonth => {
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
            ChartMode::AllTime => "All Time".to_string(),
        }
    }

    /// Calculate totals by category for any chartable items
    pub fn get_by_category<I, C>(
        &self,
        items: &[I],
        categories: &[C],
    ) -> Vec<(String, f64)>
    where
        I: ChartableItem,
        C: ChartableCategory,
    {
        let mut category_map: HashMap<Vec<u8>, (String, f64)> = categories
            .iter()
            .map(|c| (c.id().to_vec(), (c.name().to_string(), 0.0)))
            .collect();

        let filtered = self.filter_items(items);

        for item in filtered {
            if let Some(entry) = category_map.get_mut(item.category_id()) {
                entry.1 += item.amount();
            }
        }

        let mut data: Vec<(String, f64)> = category_map
            .into_values()
            .filter(|(_, amount)| *amount > 0.0)
            .collect();

        data.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        data
    }

    fn filter_items<'a, I: ChartableItem>(&self, items: &'a [I]) -> Vec<&'a I> {
        match self.mode {
            ChartMode::SelectedMonth => {
                items
                    .iter()
                    .filter(|i| i.date().month() == self.selected_month && i.date().year() == self.selected_year)
                    .collect()
            }
            ChartMode::AllTime => items.iter().collect(),
        }
    }

    /// Handle keyboard input for pie chart
    pub fn handle_pie_chart_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
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

    /// Handle mouse input for pie chart
    pub fn handle_pie_chart_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::{MouseEventKind, MouseButton};
        use super::expenses_widget::ViewInputResult;

        let button_bar_height = 3u16;
        let button_bar_start = area.y + area.height.saturating_sub(button_bar_height);
        let is_in_button_bar = mouse.row >= button_bar_start;

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_up();
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown => {
                self.scroll_down();
                ViewInputResult::Consumed
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if is_in_button_bar {
                    let mode_text_len = match self.mode {
                        ChartMode::SelectedMonth => 9,
                        ChartMode::AllTime => 10,
                    };
                    let content_end = area.x + area.width - 1;

                    let next_start = content_end.saturating_sub(8);
                    let mode_end = next_start.saturating_sub(2);
                    let mode_start = mode_end.saturating_sub(mode_text_len as u16);
                    let prev_end = mode_start.saturating_sub(2);
                    let prev_start = prev_end.saturating_sub(8);

                    if mouse.column >= next_start && mouse.column < content_end {
                        self.pending_button_action = Some(ButtonAction::NextMonth);
                        return ViewInputResult::Consumed;
                    }
                    if mouse.column >= mode_start && mouse.column < mode_end {
                        self.pending_button_action = Some(ButtonAction::ToggleMode);
                        return ViewInputResult::Consumed;
                    }
                    if mouse.column >= prev_start && mouse.column < prev_end {
                        self.pending_button_action = Some(ButtonAction::PrevMonth);
                        return ViewInputResult::Consumed;
                    }
                }
                ViewInputResult::NotConsumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }

    /// Process pending button actions
    pub fn process_pending_actions(&mut self) {
        if let Some(action) = self.pending_button_action.take() {
            match action {
                ButtonAction::PrevMonth => self.previous_month(),
                ButtonAction::NextMonth => self.next_month(),
                ButtonAction::ToggleMode => self.toggle_mode(),
            }
        }
    }
}

/// Render the button bar for pie charts
pub fn render_pie_chart_button_bar(area: Rect, buf: &mut Buffer, mode: ChartMode) {
    let mode_text = match mode {
        ChartMode::SelectedMonth => "[Monthly]",
        ChartMode::AllTime => "[All Time]",
    };
    let available_width = area.width.saturating_sub(2) as usize;
    let buttons_width = 8 + 2 + mode_text.len() + 2 + 8;
    let left_padding = available_width.saturating_sub(buttons_width);

    let line = Line::from(vec![
        Span::raw(" ".repeat(left_padding)),
        Span::styled("[◀ Prev]", Style::default().fg(Color::Cyan)),
        Span::raw("  "),
        Span::styled(mode_text, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
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

/// Render pie chart content for any chartable data
pub fn render_pie_chart_content(
    area: Rect,
    buf: &mut Buffer,
    data: &[(String, f64)],
    scroll_offset: usize,
    title: &str,
    empty_message: &str,
    chart_type: ChartType,
) {
    let total: f64 = data.iter().map(|(_, amt)| amt).sum();

    let name_width = if area.width < 60 {
        10
    } else if area.width < 80 {
        15
    } else {
        20
    };

    let max_bar_width = if area.width < 60 {
        15
    } else if area.width < 80 {
        30
    } else {
        50
    };

    if data.is_empty() {
        let paragraph = Paragraph::new(empty_message)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(title));
        Widget::render(paragraph, area, buf);
        return;
    }

    let total_text = format!("{:.2}", total);
    let total_color = chart_type.total_color();
    let mut lines: Vec<Line> = vec![
        Line::from(vec![
            Span::styled("Total: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(total_text, Style::default().fg(total_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
    ];

    for (name, amount) in data.iter() {
        let percentage = (amount / total) * 100.0;
        let bar_width = ((percentage / 100.0) * max_bar_width as f64) as usize;
        let bar = "█".repeat(bar_width.min(max_bar_width));

        let color = color_from_name(name);

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

    let paragraph = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title(title))
        .scroll((scroll_offset as u16, 0));

    Widget::render(paragraph, area, buf);
}

/// Focus area for bar chart view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarChartFocus {
    #[default]
    Chart,
    CategorySelector,
}

/// Shared state for bar chart views with category selection
#[derive(Debug, Clone, Default)]
pub struct GenericBarChartState {
    pub selected_categories: HashSet<Vec<u8>>,
    pub category_list_state: ListState,
    pub pending_category_action: Option<CategoryAction>,
    pub focus: BarChartFocus,
    pub scroll_offset: usize,
}

impl GenericBarChartState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scroll_left(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    pub fn scroll_right(&mut self, max_scroll: usize) {
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
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

    pub fn toggle_selected_category<C: ChartableCategory>(&mut self, categories: &[C]) {
        if let Some(selected) = self.category_list_state.selected() {
            if let Some(category) = categories.get(selected) {
                let id = category.id().to_vec();
                if self.selected_categories.contains(&id) {
                    self.selected_categories.remove(&id);
                } else {
                    self.selected_categories.insert(id);
                }
            }
        }
    }

    pub fn select_all_categories<C: ChartableCategory>(&mut self, categories: &[C]) {
        self.selected_categories = categories.iter().map(|c| c.id().to_vec()).collect();
    }

    pub fn deselect_all_categories(&mut self) {
        self.selected_categories.clear();
    }

    /// Calculate monthly totals for any chartable items, filtered by selected categories
    /// Returns all months that have any data (across all categories), with zero for filtered months
    pub fn get_monthly_totals<I: ChartableItem>(&self, items: &[I]) -> Vec<(String, f64)> {
        // First, get all unique months from ALL items (not filtered)
        let mut all_months: HashSet<String> = HashSet::new();
        for item in items {
            let month_key = format!("{}-{:02}", item.date().year(), item.date().month());
            all_months.insert(month_key);
        }

        // Initialize all months with zero
        let mut monthly_map: HashMap<String, f64> = all_months
            .into_iter()
            .map(|m| (m, 0.0))
            .collect();

        // Add amounts only for selected categories (or all if none selected)
        for item in items {
            if !self.selected_categories.is_empty() && !self.selected_categories.contains(&item.category_id().to_vec()) {
                continue;
            }
            let month_key = format!("{}-{:02}", item.date().year(), item.date().month());
            *monthly_map.entry(month_key).or_insert(0.0) += item.amount();
        }

        let mut monthly_vec: Vec<_> = monthly_map.into_iter().collect();
        monthly_vec.sort_by(|a, b| a.0.cmp(&b.0));

        monthly_vec
    }

    /// Handle keyboard input for bar chart
    pub fn handle_bar_chart_input(&mut self, key: crossterm::event::KeyCode) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::KeyCode;
        use super::expenses_widget::ViewInputResult;

        match key {
            // Up/Down to switch focus between chart and category selector
            KeyCode::Up => {
                self.focus = BarChartFocus::Chart;
                ViewInputResult::Consumed
            }
            KeyCode::Down => {
                self.focus = BarChartFocus::CategorySelector;
                ViewInputResult::Consumed
            }
            // Left/Right behavior depends on focus
            KeyCode::Left => {
                if self.focus == BarChartFocus::CategorySelector {
                    self.pending_category_action = Some(CategoryAction::Previous);
                } else {
                    // Scroll chart left when focused on chart
                    self.scroll_left();
                }
                ViewInputResult::Consumed
            }
            KeyCode::Right => {
                if self.focus == BarChartFocus::CategorySelector {
                    self.pending_category_action = Some(CategoryAction::Next);
                } else {
                    // Scroll chart right when focused on chart (max_scroll will be clamped in render)
                    self.scroll_right(usize::MAX);
                }
                ViewInputResult::Consumed
            }
            KeyCode::Char(' ') => {
                if self.focus == BarChartFocus::CategorySelector {
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

    /// Handle mouse input for bar chart
    pub fn handle_bar_chart_mouse(&mut self, mouse: crossterm::event::MouseEvent, area: Rect) -> super::expenses_widget::ViewInputResult {
        use crossterm::event::{MouseEventKind, MouseButton};
        use super::expenses_widget::ViewInputResult;

        // Calculate areas: chart (min), category selector (5)
        let category_area_height = 5u16;
        let category_area_start = area.y + area.height.saturating_sub(category_area_height);

        let is_in_category_area = mouse.row >= category_area_start;

        match mouse.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollLeft => {
                if is_in_category_area {
                    self.pending_category_action = Some(CategoryAction::Previous);
                } else {
                    // Scroll chart left
                    self.scroll_left();
                }
                ViewInputResult::Consumed
            }
            MouseEventKind::ScrollDown | MouseEventKind::ScrollRight => {
                if is_in_category_area {
                    self.pending_category_action = Some(CategoryAction::Next);
                } else {
                    // Scroll chart right (max_scroll will be clamped in render)
                    self.scroll_right(usize::MAX);
                }
                ViewInputResult::Consumed
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if is_in_category_area {
                    self.focus = BarChartFocus::CategorySelector;
                    // Pass click position to be resolved during render
                    self.pending_category_action = Some(CategoryAction::ClickAt {
                        x: mouse.column,
                        y: mouse.row
                    });
                } else {
                    self.focus = BarChartFocus::Chart;
                }
                ViewInputResult::Consumed
            }
            _ => ViewInputResult::NotConsumed,
        }
    }
}

/// Calculate mixed color from selected categories, or default color if none selected
pub fn calculate_bar_color_from_categories<C: ChartableCategory>(
    state: &GenericBarChartState,
    categories: &[C],
    default_color: Color,
) -> Color {
    if state.selected_categories.is_empty() {
        return default_color;
    }

    // Get colors of selected categories
    let selected_colors: Vec<(u8, u8, u8)> = categories
        .iter()
        .filter(|c| state.selected_categories.contains(&c.id().to_vec()))
        .map(|c| {
            let color = color_from_name(c.name());
            match color {
                Color::Rgb(r, g, b) => (r, g, b),
                Color::Red => (255, 0, 0),
                Color::Green => (0, 255, 0),
                Color::Blue => (0, 0, 255),
                Color::Yellow => (255, 255, 0),
                Color::Magenta => (255, 0, 255),
                Color::Cyan => (0, 255, 255),
                Color::White => (255, 255, 255),
                Color::LightRed => (255, 100, 100),
                Color::LightGreen => (100, 255, 100),
                Color::LightBlue => (100, 100, 255),
                Color::LightYellow => (255, 255, 100),
                Color::LightMagenta => (255, 100, 255),
                Color::LightCyan => (100, 255, 255),
                _ => (200, 200, 200),
            }
        })
        .collect();

    if selected_colors.is_empty() {
        return default_color;
    }

    if selected_colors.len() == 1 {
        let (r, g, b) = selected_colors[0];
        return Color::Rgb(r, g, b);
    }

    // Mix colors by averaging RGB values
    let total = selected_colors.len() as u32;
    let (r_sum, g_sum, b_sum) = selected_colors.iter().fold((0u32, 0u32, 0u32), |acc, (r, g, b)| {
        (acc.0 + *r as u32, acc.1 + *g as u32, acc.2 + *b as u32)
    });

    Color::Rgb(
        (r_sum / total) as u8,
        (g_sum / total) as u8,
        (b_sum / total) as u8,
    )
}

/// Render bar chart for any monthly data with horizontal scrolling support
pub fn render_bar_chart(
    area: Rect,
    buf: &mut Buffer,
    monthly_data: &[(String, f64)],
    scroll_offset: usize,
    title_short: &str,
    title_long: &str,
    empty_message: &str,
    bar_color: Color,
) -> usize {
    // Returns max_scroll for state management
    let title = if area.width < 60 { title_short } else { title_long };

    if monthly_data.is_empty() {
        let paragraph = Paragraph::new(empty_message)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::ALL).title(title));
        Widget::render(paragraph, area, buf);
        return 0;
    }

    let (bar_width, bar_gap) = if area.width < 60 {
        (3u16, 0u16)
    } else if area.width < 80 {
        (4u16, 1u16)
    } else {
        (5u16, 1u16)
    };

    // Calculate how many bars can fit in the chart area
    let chart_inner_width = area.width.saturating_sub(2) as usize; // Account for borders
    let bar_total_width = (bar_width + bar_gap) as usize;
    let visible_bars = if bar_total_width > 0 {
        (chart_inner_width / bar_total_width).max(1)
    } else {
        monthly_data.len()
    };

    // Calculate max scroll offset
    let max_scroll = monthly_data.len().saturating_sub(visible_bars);
    let actual_scroll = scroll_offset.min(max_scroll);

    // Slice data for visible range
    let end_idx = (actual_scroll + visible_bars).min(monthly_data.len());
    let visible_data = &monthly_data[actual_scroll..end_idx];

    // Use max from ALL data (not just visible) to keep scale consistent
    let max_amount = monthly_data.iter().map(|(_, amt)| *amt as u64).max().unwrap_or(1);
    // Ensure max is at least 1 to avoid division issues
    let max_amount = max_amount.max(1);

    let bars: Vec<Bar> = visible_data
        .iter()
        .map(|(month, amount)| {
            let label = month.split('-').nth(1).unwrap_or(month);
            Bar::default()
                .value(*amount as u64)
                .label(Line::from(label))
                .style(Style::default().fg(bar_color))
                .value_style(
                    Style::default()
                        .fg(Color::Black)
                        .bg(bar_color)
                        .add_modifier(Modifier::BOLD)
                )
        })
        .collect();

    // Build title with scroll indicators if needed
    let scroll_title = if max_scroll > 0 {
        let left_indicator = if actual_scroll > 0 { "◀ " } else { "" };
        let right_indicator = if actual_scroll < max_scroll { " ▶" } else { "" };
        format!("{}{}{}", left_indicator, title, right_indicator)
    } else {
        title.to_string()
    };

    let chart = BarChart::default()
        .block(Block::default().borders(Borders::ALL).title(scroll_title))
        .data(BarGroup::default().bars(&bars))
        .bar_width(bar_width)
        .bar_gap(bar_gap)
        .max(max_amount)
        .style(Style::default().fg(Color::White));

    Widget::render(chart, area, buf);

    max_scroll
}

/// Find which category index was clicked based on the mouse position
pub fn find_category_at_position<C: ChartableCategory>(x: u16, y: u16, area: Rect, categories: &[C]) -> Option<usize> {
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
        let cat_len = 4 + cat.name().chars().count(); // "[" + checkbox + "] " = 4 chars + name chars

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

/// Render category selector for bar chart views
pub fn render_bar_chart_category_selector<C: ChartableCategory>(
    area: Rect,
    buf: &mut Buffer,
    state: &GenericBarChartState,
    categories: &[C],
) {
    let is_focused = state.focus == BarChartFocus::CategorySelector;
    let content_width = area.width.saturating_sub(2) as usize; // Account for borders

    // Build lines with category spans
    let mut lines: Vec<Line> = vec![];
    let mut current_line: Vec<Span> = vec![];
    let mut current_len = 0usize;

    for (i, cat) in categories.iter().enumerate() {
        let is_selected = state.selected_categories.contains(&cat.id().to_vec());
        let is_highlighted = is_focused && state.category_list_state.selected() == Some(i);
        let checkbox = if is_selected { "✓" } else { " " };
        let color = color_from_name(cat.name());

        // Build the category text: [X] Name
        let cat_text = format!("[{}] {}", checkbox, cat.name());
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


