//! The commands that have a keyboard form.
//!
//! Each is one action whether it comes from a key, a button or a menu, so the
//! entry points cannot disagree about what it does.

gpui_kit::actions!(
    money_manager,
    [
        Quit,
        ShowTransactions,
        ShowCategories,
        ShowCharts,
        ToggleMode,
        NewRecord,
        EditRecord,
        NewTransfer,
        SyncNow,
        OpenSettings
    ]
);

/// The key context the workspace's shortcuts live in.
pub const CONTEXT: &str = "Workspace";
