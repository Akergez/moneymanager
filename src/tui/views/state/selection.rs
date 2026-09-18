//! Row selection and scrolling shared by the table views.

/// Rows above the first data row of a bordered table with a header:
/// top border, header, header bottom margin.
pub const TABLE_DATA_TOP: u16 = 3;
/// Rows of a bordered table with a header that are not data rows.
const TABLE_CHROME_ROWS: u16 = 4;

/// Selected row + scroll offset of a table. The table reports its row count and
/// viewport height on every render via [`RowSelection::update_layout`], which
/// keeps both values in range and scrolls to follow keyboard selection.
#[derive(Debug, Clone, Default)]
pub struct RowSelection {
    pub selected: Option<usize>,
    pub scroll_offset: usize,
    total: usize,
    visible: usize,
    /// Scroll to keep `selected` visible on the next render. Cleared by the
    /// mouse wheel so free scrolling isn't snapped back to the selection.
    follow: bool,
}

impl RowSelection {
    /// Move the selection by `delta` rows (no selection yet → the first
    /// visible row).
    pub fn move_by(&mut self, delta: isize) {
        if self.total == 0 {
            self.selected = None;
            return;
        }
        let last = self.total - 1;
        let next = match self.selected {
            None => self.scroll_offset.min(last),
            Some(i) => i.saturating_add_signed(delta).min(last),
        };
        self.selected = Some(next);
        self.follow = true;
    }

    /// Select `index` directly (clamped to the row count).
    pub fn select(&mut self, index: usize) {
        if self.total == 0 {
            self.selected = None;
        } else {
            self.selected = Some(index.min(self.total - 1));
            self.follow = true;
        }
    }

    /// Select the row under a click at `row_in_viewport` (0 = first data row).
    pub fn click(&mut self, row_in_viewport: usize) {
        let index = self.scroll_offset + row_in_viewport;
        if index < self.total {
            self.selected = Some(index);
        }
    }

    /// Mouse-wheel scrolling: moves the viewport, not the selection.
    pub fn scroll_by(&mut self, delta: isize) {
        self.scroll_offset = self
            .scroll_offset
            .saturating_add_signed(delta)
            .min(self.total.saturating_sub(self.visible));
        self.follow = false;
    }

    pub fn reset(&mut self) {
        self.selected = None;
        self.scroll_offset = 0;
    }

    /// Record the current row count and the height of the table widget, then
    /// clamp the selection/scroll and scroll to the selection if needed.
    pub fn update_layout(&mut self, total: usize, table_height: u16) {
        self.total = total;
        self.visible = table_height.saturating_sub(TABLE_CHROME_ROWS).max(1) as usize;
        if let Some(i) = self.selected {
            self.selected = (total > 0).then(|| i.min(total - 1));
        }
        if self.follow
            && let Some(i) = self.selected
        {
            if i < self.scroll_offset {
                self.scroll_offset = i;
            } else if i >= self.scroll_offset + self.visible {
                self.scroll_offset = i + 1 - self.visible;
            }
        }
        self.scroll_offset = self.scroll_offset.min(total.saturating_sub(self.visible));
    }

    /// Handle a left click at screen row `y` for a table whose widget starts at
    /// `table_top` and whose data area ends before `data_bottom`. Returns true
    /// if the click landed on a data row.
    pub fn click_at(&mut self, y: u16, table_top: u16, data_bottom: u16) -> bool {
        let first = table_top + TABLE_DATA_TOP;
        if y >= first && y < data_bottom {
            self.click((y - first) as usize);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_selection_scrolls_into_view_and_clamps() {
        let mut s = RowSelection::default();
        s.update_layout(20, 9); // 5 visible data rows
        s.move_by(1);
        assert_eq!(s.selected, Some(0));
        s.move_by(6);
        s.update_layout(20, 9);
        assert_eq!(s.selected, Some(6));
        assert_eq!(s.scroll_offset, 2); // rows 2..=6 visible
        s.move_by(100);
        s.update_layout(20, 9);
        assert_eq!(s.selected, Some(19));
        assert_eq!(s.scroll_offset, 15);
        s.move_by(-100);
        s.update_layout(20, 9);
        assert_eq!((s.selected, s.scroll_offset), (Some(0), 0));
    }

    #[test]
    fn click_maps_screen_rows_to_data_rows() {
        let mut s = RowSelection::default();
        s.update_layout(10, 9);
        s.scroll_by(3);
        // Table at y=10: border 10, header 11, margin 12, first data row 13.
        assert!(!s.click_at(12, 10, 18));
        assert!(s.click_at(13, 10, 18));
        assert_eq!(s.selected, Some(3));
        assert!(s.click_at(14, 10, 18));
        assert_eq!(s.selected, Some(4));
    }

    #[test]
    fn shrinking_rows_clamps_selection() {
        let mut s = RowSelection::default();
        s.update_layout(5, 20);
        s.select(4);
        s.update_layout(2, 20);
        assert_eq!(s.selected, Some(1));
        s.update_layout(0, 20);
        assert_eq!(s.selected, None);
    }
}
