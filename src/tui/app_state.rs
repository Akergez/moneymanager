//! Main application state using StatefulWidgets

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyModifiers, MouseEvent, MouseEventKind, MouseButton};
use ratatui::layout::Rect;
use crate::config::Config;
use crate::ledger::{self, AccountSummary};
use crate::models::{Account, Category, Expense, Transfer, TopUpCategory, TopUp};
use crate::store::{Store, DEFAULT_ACCOUNT_ID, hex_decode, hex_encode};

use chrono::NaiveDate;

use super::types::Tab;
use super::ui::{tab_titles_for_width, compute_tab_rows};
use super::utils::format_money;
use super::views::{
    ExpensesViewState, TopUpsViewState,
    ExpenseCategoriesViewState, TopUpCategoriesViewState,
    PieChartViewState, BarChartViewState, LineChartViewState,
    TopUpPieChartViewState, TopUpBarChartViewState,
    AccountsViewState,
    TransfersViewState,
    ConfirmDialogState, RecordType,
    ViewInputResult, ViewState,
};
use super::forms::{
    CategoryFormState, ExpenseFormState, TopUpCategoryFormState, TopUpFormState,
    AccountFormState, TransferFormState, FormInputResult,
};

/// Main application state
pub struct AppState {
    pub running: bool,
    pub current_tab: Tab,

    // Data
    pub categories: Vec<Category>,
    pub expenses: Vec<Expense>,
    pub top_up_categories: Vec<TopUpCategory>,
    pub top_ups: Vec<TopUp>,
    pub accounts: Vec<Account>,
    pub transfers: Vec<Transfer>,

    /// The account currently shown; every list/chart is filtered to it.
    pub current_account: Vec<u8>,
    /// Per-account totals (`id -> balance/in/out`), recomputed on reload.
    pub summaries: HashMap<Vec<u8>, AccountSummary>,
    /// Current account's ledger entries (real + derived transfer legs).
    pub account_expenses: Vec<Expense>,
    pub account_top_ups: Vec<TopUp>,
    /// Real categories + the virtual transfer category (for lists/charts).
    pub expense_categories_ext: Vec<Category>,
    pub top_up_categories_ext: Vec<TopUpCategory>,
    /// Leave transfer legs out of the pie charts (toggled with `x`).
    pub hide_transfers_on_pie: bool,

    // View states
    pub expenses_view: ExpensesViewState,
    pub top_ups_view: TopUpsViewState,
    pub expense_categories_view: ExpenseCategoriesViewState,
    pub top_up_categories_view: TopUpCategoriesViewState,
    pub pie_chart_view: PieChartViewState,
    pub bar_chart_view: BarChartViewState,
    pub line_chart_view: LineChartViewState,
    pub top_up_pie_chart_view: TopUpPieChartViewState,
    pub top_up_bar_chart_view: TopUpBarChartViewState,
    pub accounts_view: AccountsViewState,
    pub transfers_view: TransfersViewState,

    /// Height of the tab bar (updated each frame by ui::draw, used by mouse handler)
    pub tab_bar_height: u16,

    // Form states
    pub category_form: CategoryFormState,
    pub expense_form: ExpenseFormState,
    pub top_up_category_form: TopUpCategoryFormState,
    pub top_up_form: TopUpFormState,
    pub account_form: AccountFormState,
    pub transfer_form: TransferFormState,

    // Delete-confirmation dialog
    pub confirm_dialog: ConfirmDialogState,

    /// Local config, which carries the S3 remote (if any) plus the multi-account
    /// fields default_currency / last_account.
    pub config: Config,
    /// Path of the local money_manager.toml.
    pub config_path: PathBuf,
    /// Transient status line shown in the footer until the next key press:
    /// `(message, is_error)`.
    pub status: Option<(String, bool)>,
}

impl AppState {
    pub fn new(
        conn: &mut Store,
        config: Config,
        config_path: PathBuf,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Restore the last shown account; if the stored id is missing
        // (deleted or pre-sync) fall back to the first known account.
        let accounts = Account::read_all(conn)?;
        let candidate = config
            .last_account
            .as_deref()
            .map(hex_decode)
            .unwrap_or_else(|| DEFAULT_ACCOUNT_ID.to_vec());
        let current_account = accounts
            .iter()
            .find(|a| a.id == candidate)
            .or_else(|| accounts.first())
            .map(|a| a.id.clone())
            .unwrap_or_else(|| DEFAULT_ACCOUNT_ID.to_vec());

        let mut state = Self {
            running: true,
            current_tab: Tab::ExpenseCategories,
            categories: Vec::new(),
            expenses: Vec::new(),
            top_up_categories: Vec::new(),
            top_ups: Vec::new(),
            accounts,
            transfers: Vec::new(),
            current_account,
            summaries: HashMap::new(),
            account_expenses: Vec::new(),
            account_top_ups: Vec::new(),
            expense_categories_ext: Vec::new(),
            top_up_categories_ext: Vec::new(),
            hide_transfers_on_pie: false,
            expenses_view: ExpensesViewState::new(),
            top_ups_view: TopUpsViewState::new(),
            expense_categories_view: ExpenseCategoriesViewState::new(),
            top_up_categories_view: TopUpCategoriesViewState::new(),
            pie_chart_view: PieChartViewState::new(),
            bar_chart_view: BarChartViewState::new(),
            line_chart_view: LineChartViewState::new(),
            top_up_pie_chart_view: TopUpPieChartViewState::new(),
            top_up_bar_chart_view: TopUpBarChartViewState::new(),
            accounts_view: AccountsViewState::new(),
            transfers_view: TransfersViewState::new(),
            tab_bar_height: 3,
            category_form: CategoryFormState::new(),
            expense_form: ExpenseFormState::new(),
            top_up_category_form: TopUpCategoryFormState::new(),
            top_up_form: TopUpFormState::new(),
            account_form: AccountFormState::new(),
            transfer_form: TransferFormState::new(),
            confirm_dialog: ConfirmDialogState::new(),
            config,
            config_path,
            status: None,
        };
        state.reload_data(conn)?;
        Ok(state)
    }

    /// Bidirectional sync with the configured S3 remote, then reload the views.
    ///
    /// Delegates the chunk dance (stage local chunk → fetch remote → push local
    /// → merge → persist) to [`Store::sync`]; on success we reload the in-memory
    /// data so the merged state shows up in the app immediately.
    pub fn sync(&mut self, conn: &mut Store) {
        let Some(remote) = self.config.remote.clone() else {
            self.status = Some(("No [remote] configured in money_manager.toml".to_string(), true));
            return;
        };
        match conn.sync(&remote) {
            Ok(report) => {
                self.status = Some((
                    format!("Synced — pulled {}, pushed {}", report.pulled.len(), report.pushed.len()),
                    false,
                ));
                self.reload(conn);
            }
            Err(e) => self.status = Some((format!("Sync failed: {e}"), true)),
        }
    }

    pub fn reload_data(&mut self, conn: &mut Store) -> Result<(), Box<dyn std::error::Error>> {
        self.categories = Category::read_all(conn)?;
        self.expenses = Expense::read_all(conn)?;
        self.top_up_categories = TopUpCategory::read_all(conn)?;
        self.top_ups = TopUp::read_all(conn)?;
        self.accounts = Account::read_all(conn)?;
        self.transfers = Transfer::read_all(conn)?;
        self.recompute_view();
        Ok(())
    }

    /// [`AppState::reload_data`], reporting a failure in the status line.
    fn reload(&mut self, conn: &mut Store) {
        if let Err(e) = self.reload_data(conn) {
            self.status = Some((format!("Reload failed: {e}"), true));
        }
    }

    /// Recompute the per-account caches after the underlying data changed.
    /// Table selections are kept (the tables clamp them to the new row count).
    pub fn recompute_view(&mut self) {
        match self.current_account_ref() {
            Some(acct) => {
                let ledger = ledger::ledger_for(
                    acct,
                    &self.accounts,
                    &self.expenses,
                    &self.top_ups,
                    &self.transfers,
                );
                self.account_expenses = ledger.expenses;
                self.account_top_ups = ledger.top_ups;
            }
            None => {
                // Current account unknown (e.g. not synced yet): empty ledger.
                self.account_expenses.clear();
                self.account_top_ups.clear();
            }
        }

        self.summaries = ledger::summaries(&self.accounts, &self.expenses, &self.top_ups, &self.transfers);
        self.expense_categories_ext = ledger::with_transfer_category(&self.categories);
        self.top_up_categories_ext = ledger::with_transfer_top_up_category(&self.top_up_categories);
    }

    /// The account currently shown, if it exists.
    pub fn current_account_ref(&self) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == self.current_account)
    }

    /// Name and currency of the current account (empty if unknown).
    pub fn current_account_label(&self) -> (&str, &str) {
        self.current_account_ref()
            .map(|a| (a.name.as_str(), a.currency.as_str()))
            .unwrap_or(("", ""))
    }

    /// Move to the next/previous account in the list and persist the selection.
    fn step_account(&mut self, forward: bool) {
        let len = self.accounts.len();
        if len < 2 {
            return;
        }
        let idx = self
            .accounts
            .iter()
            .position(|a| a.id == self.current_account)
            .unwrap_or(0);
        let new_idx = if forward { (idx + 1) % len } else { (idx + len - 1) % len };
        self.set_current_account(self.accounts[new_idx].id.clone());
    }

    pub fn next_account(&mut self) {
        self.step_account(true);
    }

    pub fn previous_account(&mut self) {
        self.step_account(false);
    }

    /// Switch the displayed account to `account_id` (must exist) and persist it.
    pub fn set_current_account(&mut self, account_id: Vec<u8>) {
        if account_id == self.current_account || !self.accounts.iter().any(|a| a.id == account_id) {
            return;
        }
        self.current_account = account_id;
        self.recompute_view();
        // The row lists are now a different account's: stale scroll/selection
        // would point at unrelated records.
        self.expenses_view.reset_selection();
        self.top_ups_view.reset_selection();

        let hex = hex_encode(&self.current_account);
        if let Err(e) = self.config.save_last_account(&self.config_path, &hex) {
            self.status = Some((format!("Could not remember the account: {e}"), true));
        }
    }

    fn last_expense_date(&self) -> Option<NaiveDate> {
        self.account_expenses.iter().filter(|e| e.transfer.is_none()).map(|e| e.date).max()
    }

    fn last_top_up_date(&self) -> Option<NaiveDate> {
        self.account_top_ups.iter().filter(|t| t.transfer.is_none()).map(|t| t.date).max()
    }

    fn open_transfer_form(&mut self) {
        if self.accounts.len() < 2 {
            self.status = Some(("A transfer needs at least two accounts — create one on the Accounts tab (0)".to_string(), true));
            return;
        }
        let last_transfer_date = self.transfers.iter().map(|t| t.date).max();
        self.transfer_form.open(&self.accounts, &self.current_account, last_transfer_date);
    }

    /// Title and description of the confirmation for deleting transfer `tr`.
    fn describe_transfer_delete(&self, tr: &Transfer) -> (String, String) {
        let from_name = ledger::account_name(&self.accounts, &tr.from_account_id);
        let to_name = ledger::account_name(&self.accounts, &tr.to_account_id);
        let from_cur = ledger::currency_of(&self.accounts, &tr.from_account_id);
        let to_cur = ledger::currency_of(&self.accounts, &tr.to_account_id);
        let title = format!(
            "Delete transfer {} → {}?",
            format_money(tr.amount_from, from_cur),
            format_money(tr.amount_to, to_cur)
        );
        let mut desc = format!(
            "{} · {} → {}. This removes both the expense on \"{}\" and the top-up on \"{}\".",
            tr.date, from_name, to_name, from_name, to_name
        );
        if let Some(c) = &tr.comment {
            desc.push_str(&format!(" Comment: {c}"));
        }
        (title, desc)
    }

    /// Open the delete-confirmation dialog for the currently selected record.
    /// A transfer leg (on the expenses/top-ups tabs) deletes the whole transfer.
    fn open_confirm_delete(&mut self) {
        let (_, currency) = self.current_account_label();
        let currency = currency.to_string();

        // (kind, id, amount, category name, date, comment, transfer leg id)
        let target = match self.current_tab {
            Tab::Expenses => self.expenses_view.rows.selected.and_then(|idx| {
                let exp = self.expenses_view.sort_expenses(&self.account_expenses).into_iter().nth(idx)?;
                let category = self
                    .expense_categories_ext
                    .iter()
                    .find(|c| c.id == exp.category_id)
                    .map(|c| c.name.clone());
                Some(("expense", RecordType::Expense, exp.id, exp.amount, category, exp.date, exp.comment, exp.transfer))
            }),
            Tab::TopUps => self.top_ups_view.rows.selected.and_then(|idx| {
                let t = self.top_ups_view.sort_top_ups(&self.account_top_ups).into_iter().nth(idx)?;
                let category = self
                    .top_up_categories_ext
                    .iter()
                    .find(|c| c.id == t.category_id)
                    .map(|c| c.name.clone());
                Some(("top-up", RecordType::TopUp, t.id, t.amount, category, t.date, t.comment, t.transfer))
            }),
            Tab::Transfers => {
                let tr = self.transfers_view.rows.selected.and_then(|idx| {
                    self.transfers_view.sorted(&self.transfers, &self.accounts).get(idx).map(|t| (*t).clone())
                });
                if let Some(tr) = tr {
                    let (title, desc) = self.describe_transfer_delete(&tr);
                    self.confirm_dialog.open(RecordType::Transfer, tr.id, title, desc);
                }
                return;
            }
            _ => None,
        };
        let Some((kind, record_type, id, amount, category, date, comment, leg)) = target else {
            return;
        };

        if let Some(leg) = leg {
            // A leg is derived from its transfer: deleting it deletes the transfer.
            if let Some(tr) = self.transfers.iter().find(|t| t.id == leg.transfer_id).cloned() {
                let (title, desc) = self.describe_transfer_delete(&tr);
                self.confirm_dialog.open(RecordType::Transfer, tr.id, title, desc);
            }
            return;
        }

        let title = format!(
            "Delete {kind} {} · {} · {date}?",
            format_money(amount, &currency),
            category.as_deref().unwrap_or("Unknown")
        );
        let desc = comment.map(|c| format!("Comment: {c}")).unwrap_or_default();
        self.confirm_dialog.open(record_type, id, title, desc);
    }

    /// Perform the delete confirmed in the dialog, then reload.
    fn delete_record(&mut self, record_type: RecordType, id: &[u8], conn: &mut Store) {
        let (result, what) = match record_type {
            RecordType::Expense => (Expense::delete(conn, id), "Expense"),
            RecordType::TopUp => (TopUp::delete(conn, id), "Top-up"),
            RecordType::Transfer => (Transfer::delete(conn, id), "Transfer"),
        };
        match result {
            Ok(_) => {
                self.status = Some((format!("{what} deleted"), false));
                self.reload(conn);
            }
            Err(e) => self.status = Some((format!("Delete failed: {e}"), true)),
        }
    }

    /// Act on the confirm dialog's result (the dialog clears its state when it
    /// closes, so the target is captured by the caller beforehand).
    fn finish_confirm(&mut self, result: FormInputResult, record_type: RecordType, id: Vec<u8>, conn: &mut Store) {
        if result == FormInputResult::SubmittedNeedsReload {
            self.delete_record(record_type, &id, conn);
        }
    }

    fn finish_form(&mut self, result: FormInputResult, conn: &mut Store) {
        if result == FormInputResult::SubmittedNeedsReload {
            self.reload(conn);
        }
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
            Tab::Accounts => &mut self.accounts_view,
            Tab::ExpenseCategories => &mut self.expense_categories_view,
            Tab::Expenses => &mut self.expenses_view,
            Tab::TopUpCategories => &mut self.top_up_categories_view,
            Tab::TopUps => &mut self.top_ups_view,
            Tab::ExpensePieChart => &mut self.pie_chart_view,
            Tab::ExpenseBarChart => &mut self.bar_chart_view,
            Tab::ExpenseLineChart => &mut self.line_chart_view,
            Tab::TopUpPieChart => &mut self.top_up_pie_chart_view,
            Tab::TopUpBarChart => &mut self.top_up_bar_chart_view,
            Tab::Transfers => &mut self.transfers_view,
        }
    }

    /// Act on a view's request (shared by keyboard and mouse handling).
    /// Returns false if the view didn't handle the input.
    fn apply_view_result(&mut self, result: ViewInputResult) -> bool {
        match result {
            ViewInputResult::OpenCategoryForm => self.category_form.open(),
            ViewInputResult::OpenExpenseForm => {
                self.expense_form.open(&self.categories, self.last_expense_date(), &self.current_account)
            }
            ViewInputResult::OpenTopUpCategoryForm => self.top_up_category_form.open(),
            ViewInputResult::OpenTopUpForm => {
                self.top_up_form.open(&self.top_up_categories, self.last_top_up_date(), &self.current_account)
            }
            ViewInputResult::OpenAccountForm => self.account_form.open_create(),
            ViewInputResult::OpenAccountEdit => {
                if let Some(acct) = self.accounts_view.selected().and_then(|i| self.accounts.get(i)) {
                    self.account_form.open_edit(&acct.id, &acct.name, &acct.currency, acct.opening_balance);
                }
            }
            ViewInputResult::MakeCurrent => {
                if let Some(id) = self.accounts_view.selected().and_then(|i| self.accounts.get(i)).map(|a| a.id.clone()) {
                    self.set_current_account(id);
                }
            }
            ViewInputResult::OpenTransferForm => self.open_transfer_form(),
            ViewInputResult::OpenConfirmDelete => self.open_confirm_delete(),
            ViewInputResult::Consumed => {}
            ViewInputResult::NotConsumed => return false,
        }
        true
    }

    /// Handle all input - delegates to forms or views as appropriate
    pub fn handle_input(&mut self, key: KeyCode, modifiers: KeyModifiers, conn: &mut Store) {
        // The status line is transient: any key dismisses it.
        self.status = None;

        // Handle form input first if a form is active
        if self.category_form.is_active {
            let result = self.category_form.handle_input(key, conn);
            return self.finish_form(result, conn);
        }
        if self.expense_form.is_active {
            let result = self.expense_form.handle_input(key, conn);
            return self.finish_form(result, conn);
        }
        if self.top_up_category_form.is_active {
            let result = self.top_up_category_form.handle_input(key, conn);
            return self.finish_form(result, conn);
        }
        if self.top_up_form.is_active {
            let result = self.top_up_form.handle_input(key, conn);
            return self.finish_form(result, conn);
        }
        if self.account_form.is_active {
            let result = self.account_form.handle_input(key, conn);
            return self.finish_form(result, conn);
        }
        if self.transfer_form.is_active {
            let result = self.transfer_form.handle_input(key, conn);
            return self.finish_form(result, conn);
        }
        if self.confirm_dialog.is_active {
            let record_type = self.confirm_dialog.record_type;
            let record_id = self.confirm_dialog.record_id.clone();
            let result = self.confirm_dialog.handle_input(key);
            return self.finish_confirm(result, record_type, record_id, conn);
        }

        // Delegate to current view
        let view_result = self.current_view_mut().handle_input(key);
        if self.apply_view_result(view_result) {
            return;
        }

        // Global input handling
        match key {
            // Quit
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit(),
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => self.quit(),

            // Tab navigation
            KeyCode::Tab => self.next_tab(),
            KeyCode::BackTab => self.previous_tab(),

            // Account switching (previous/next in the accounts list)
            KeyCode::Char('[') => self.previous_account(),
            KeyCode::Char(']') => self.next_account(),

            // Direct tab selection
            KeyCode::Char(c @ '0'..='9') => self.current_tab = Tab::from_index(c as usize - '0' as usize),

            // Reload
            KeyCode::Char('r') | KeyCode::Char('R') => self.reload(conn),

            // Sync with the S3 remote
            KeyCode::Char('s') | KeyCode::Char('S') => self.sync(conn),

            // Open transfer form from any tab
            KeyCode::Char('t') | KeyCode::Char('T') => self.open_transfer_form(),

            // Show/hide transfer legs on the pie charts
            KeyCode::Char('x') | KeyCode::Char('X')
                if matches!(self.current_tab, Tab::ExpensePieChart | Tab::TopUpPieChart) =>
            {
                self.hide_transfers_on_pie = !self.hide_transfers_on_pie;
            }

            _ => {}
        }
    }

    /// Handle mouse input for tab switching and view interactions
    pub fn handle_mouse(&mut self, mouse: MouseEvent, area: Rect, conn: &mut Store) {
        // Handle mouse in active forms first; a `true` from `handle_mouse` means
        // a button was clicked and is processed by `handle_input(Null)`.
        if self.category_form.is_active {
            if self.category_form.handle_mouse(mouse) {
                let result = self.category_form.handle_input(KeyCode::Null, conn);
                self.finish_form(result, conn);
            }
            return;
        }
        if self.expense_form.is_active {
            if self.expense_form.handle_mouse(mouse) {
                let result = self.expense_form.handle_input(KeyCode::Null, conn);
                self.finish_form(result, conn);
            }
            return;
        }
        if self.top_up_category_form.is_active {
            if self.top_up_category_form.handle_mouse(mouse) {
                let result = self.top_up_category_form.handle_input(KeyCode::Null, conn);
                self.finish_form(result, conn);
            }
            return;
        }
        if self.top_up_form.is_active {
            if self.top_up_form.handle_mouse(mouse) {
                let result = self.top_up_form.handle_input(KeyCode::Null, conn);
                self.finish_form(result, conn);
            }
            return;
        }
        if self.account_form.is_active {
            if self.account_form.handle_mouse(mouse) {
                let result = self.account_form.handle_input(KeyCode::Null, conn);
                self.finish_form(result, conn);
            }
            return;
        }
        if self.transfer_form.is_active {
            if self.transfer_form.handle_mouse(mouse) {
                let result = self.transfer_form.handle_input(KeyCode::Null, conn);
                self.finish_form(result, conn);
            }
            return;
        }
        if self.confirm_dialog.is_active {
            let record_type = self.confirm_dialog.record_type;
            let record_id = self.confirm_dialog.record_id.clone();
            if self.confirm_dialog.handle_mouse(mouse) {
                let result = self.confirm_dialog.handle_input(KeyCode::Null);
                self.finish_confirm(result, record_type, record_id, conn);
            }
            return;
        }

        // Only handle left button clicks for tabs
        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
            let x = mouse.column;
            let y = mouse.row;

            // Tab bar occupies the last tab_bar_height rows
            let tab_bar_top = area.y + area.height.saturating_sub(self.tab_bar_height);
            if y >= tab_bar_top {
                // Row within the tab bar (0 = top border)
                let bar_row = y - tab_bar_top;
                // Ignore border rows
                if bar_row == 0 || bar_row >= self.tab_bar_height.saturating_sub(1) {
                    return;
                }
                // Content row index (0-based, inside borders)
                let content_row = (bar_row - 1) as usize;

                let titles = tab_titles_for_width(area.width);
                let content_width = area.width.saturating_sub(2) as usize;
                let rows = compute_tab_rows(&titles, content_width);

                if let Some(row_tabs) = rows.get(content_row) {
                    // x position inside the border
                    let click_x = x.saturating_sub(area.x + 1) as usize;
                    let mut current_pos = 0usize;
                    for (i, &tab_idx) in row_tabs.iter().enumerate() {
                        let sep = if i > 0 { 3 } else { 0 };
                        let tab_start = current_pos + sep;
                        let tab_end = tab_start + titles[tab_idx].chars().count();
                        if click_x >= tab_start && click_x < tab_end {
                            self.current_tab = Tab::from_index(tab_idx);
                            return;
                        }
                        current_pos = tab_end;
                    }
                }
                return;
            }
        }

        // Calculate content area (between header and tab bar)
        let content_area = Rect {
            x: area.x,
            y: area.y + 3,
            width: area.width,
            height: area.height.saturating_sub(3 + self.tab_bar_height),
        };

        // Delegate to current view for scroll and other interactions
        let view_result = match self.current_tab {
            Tab::Accounts => self.accounts_view.handle_mouse(mouse, content_area),
            Tab::ExpenseCategories => self.expense_categories_view.handle_mouse(mouse, content_area),
            Tab::Expenses => self.expenses_view.handle_mouse(mouse, content_area),
            Tab::TopUpCategories => self.top_up_categories_view.handle_mouse(mouse, content_area),
            Tab::TopUps => self.top_ups_view.handle_mouse(mouse, content_area),
            Tab::ExpensePieChart => self.pie_chart_view.handle_mouse(mouse, content_area),
            Tab::ExpenseBarChart => self.bar_chart_view.handle_mouse(mouse, content_area),
            Tab::ExpenseLineChart => self.line_chart_view.handle_mouse(mouse, content_area),
            Tab::TopUpPieChart => self.top_up_pie_chart_view.handle_mouse(mouse, content_area),
            Tab::TopUpBarChart => self.top_up_bar_chart_view.handle_mouse(mouse, content_area),
            Tab::Transfers => self.transfers_view.handle_mouse(mouse, content_area),
        };
        self.apply_view_result(view_result);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::Config;
    use crate::models::Account;
    use crate::store::Store;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(tag: &str) -> PathBuf {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("mm_app_{tag}_{nanos}"));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Reproduce what the TUI does on first launch: open the store, ensure the
    /// default account, then build the full `AppState`.
    fn build_state(dir: &std::path::Path) -> (Store, AppState) {
        let mut store = Store::open(&dir.join("data"), 7).unwrap();
        Account::ensure_default(&mut store, "RUB").unwrap();
        let state = AppState::new(&mut store, Config::default(), dir.join("money_manager.toml")).unwrap();
        (store, state)
    }

    fn key(state: &mut AppState, store: &mut Store, code: KeyCode) {
        state.handle_input(code, KeyModifiers::NONE, store);
    }

    #[test]
    fn new_starts_on_the_default_account() {
        let dir = temp_dir("new");
        let (_store, s) = build_state(&dir);
        assert!(s.running);
        assert_eq!(s.accounts.len(), 1);
        assert_eq!(s.current_account, DEFAULT_ACCOUNT_ID.to_vec());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn deleting_a_transfer_leg_from_expenses_deletes_the_transfer() {
        let dir = temp_dir("leg");
        let (mut store, mut s) = build_state(&dir);
        let usd = Account::create(&mut store, "Card USD", "USD", 0.0).unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        Transfer::create(&mut store, &DEFAULT_ACCOUNT_ID, &usd.id, 9235.0, 100.0, None, date).unwrap();
        s.reload_data(&mut store).unwrap();
        assert_eq!(s.account_expenses.len(), 1, "outbound leg on the default account");

        // Select the leg with the keyboard, press d, confirm with y.
        s.current_tab = Tab::Expenses;
        s.expenses_view.rows.update_layout(s.account_expenses.len(), 20);
        key(&mut s, &mut store, KeyCode::Down);
        assert_eq!(s.expenses_view.rows.selected, Some(0));
        key(&mut s, &mut store, KeyCode::Char('d'));
        assert!(s.confirm_dialog.is_active);
        assert_eq!(s.confirm_dialog.record_type, RecordType::Transfer);
        assert!(s.confirm_dialog.description.contains("Card USD"));
        key(&mut s, &mut store, KeyCode::Char('y'));

        assert!(Transfer::read_all(&store).unwrap().is_empty());
        assert!(s.account_expenses.is_empty());
        assert_eq!(s.summaries[&usd.id].balance, 0.0);
        assert_eq!(s.summaries[&DEFAULT_ACCOUNT_ID.to_vec()].balance, 0.0);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn switching_accounts_filters_views_and_persists_the_choice() {
        let dir = temp_dir("switch");
        let (mut store, mut s) = build_state(&dir);
        let usd = Account::create(&mut store, "Card USD", "USD", 50.0).unwrap();
        let cat = Category::create(&mut store, "Food").unwrap();
        let date = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        Expense::create(&mut store, &cat.id, 10.0, None, date, &DEFAULT_ACCOUNT_ID).unwrap();
        s.reload_data(&mut store).unwrap();
        assert_eq!(s.account_expenses.len(), 1);

        let expected = if s.accounts[0].id == usd.id { KeyCode::Char('[') } else { KeyCode::Char(']') };
        key(&mut s, &mut store, expected);
        assert_eq!(s.current_account, usd.id);
        assert!(s.account_expenses.is_empty());
        assert_eq!(s.summaries[&usd.id].balance, 50.0);

        let saved = Config::load(&dir.join("money_manager.toml")).unwrap();
        assert_eq!(saved.last_account, Some(hex_encode(&usd.id)));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn transfer_form_needs_two_accounts() {
        let dir = temp_dir("tform");
        let (mut store, mut s) = build_state(&dir);
        key(&mut s, &mut store, KeyCode::Char('t'));
        assert!(!s.transfer_form.is_active);
        assert!(s.status.as_ref().is_some_and(|(_, err)| *err));
        std::fs::remove_dir_all(dir).ok();
    }
}
