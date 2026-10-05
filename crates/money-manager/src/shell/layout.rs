//! Which of the three layouts a window is wide enough for.
//!
//! The layout is read off the window's width on every frame, which is the
//! only thing that is true when a window is tiled, maximized or is a phone's
//! whole screen. Nothing about it is stored.

/// Below this the navigation is a bar along the bottom and the list of
/// records is a list rather than a table.
pub const PHONE_BELOW: f32 = 700.0;

/// From this the sidebar has room for its labels.
pub const DESKTOP_FROM: f32 = 1100.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    /// One column, navigation along the bottom.
    Phone,
    /// The sidebar collapsed to its icons.
    Tablet,
    /// The sidebar with its labels.
    Desktop,
}

impl Layout {
    pub fn for_width(width: f32) -> Self {
        if width < PHONE_BELOW {
            Layout::Phone
        } else if width < DESKTOP_FROM {
            Layout::Tablet
        } else {
            Layout::Desktop
        }
    }

    pub fn is_phone(self) -> bool {
        self == Layout::Phone
    }

    /// What a UI scenario calls it.
    pub fn name(self) -> &'static str {
        match self {
            Layout::Phone => "phone",
            Layout::Tablet => "tablet",
            Layout::Desktop => "desktop",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layout_follows_the_width_and_changes_exactly_at_its_bounds() {
        assert_eq!(Layout::for_width(390.0), Layout::Phone);
        assert_eq!(Layout::for_width(699.9), Layout::Phone);
        assert_eq!(Layout::for_width(700.0), Layout::Tablet);
        assert_eq!(Layout::for_width(834.0), Layout::Tablet);
        assert_eq!(Layout::for_width(1099.9), Layout::Tablet);
        assert_eq!(Layout::for_width(1100.0), Layout::Desktop);
        assert_eq!(Layout::for_width(2560.0), Layout::Desktop);
    }
}
