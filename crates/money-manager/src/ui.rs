//! Small pieces every screen draws with.
//!
//! The window is drawn with the design system (`gpui-adaptive`): its intents
//! for what a person does, its primitives for the parts of this application
//! no intent is. What is here holds no state and knows nothing of the ledger
//! beyond how a category looks.

use std::sync::{Arc, OnceLock};

use gpui_adaptive::{Tile, TileSize};
use gpui_kit::{Image, ImageFormat};

use crate::book::CategoryLook;

/// The full icon catalogue: what a category can wear goes well past the
/// system's set.
pub use gpui_adaptive::Lucide;

/// The application's own icon — the same file the desktop shows in its
/// launcher.
pub fn app_icon() -> Arc<Image> {
    static ICON: OnceLock<Arc<Image>> = OnceLock::new();
    ICON.get_or_init(|| {
        Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            include_bytes!(
                "../../../data/icons/hicolor/scalable/apps/app.akergez.MoneyManager.svg"
            )
            .to_vec(),
        ))
    })
    .clone()
}

/// A category's icon on a tint of its colour: what identifies it wherever it
/// appears.
pub fn category_tile(look: CategoryLook, size: TileSize) -> Tile {
    Tile::new(look.icon, look.color).size(size)
}
