//! How a category is drawn: its colour and its icon.
//!
//! Both are part of the category's record in the ledger (see
//! [`CategoryStyle`]) and sync with it; both are optional there. A category
//! nobody has styled — every one made by the terminal version — still gets a
//! colour, a hue derived from its name and so the same on every device, and
//! a neutral icon.
//!
//! The colours here are data, not theme: a category's colour is what a person
//! picked for it and has to stay that colour in either theme, on the chart
//! and in the list alike. That is the one place raw colour values belong.

use gpui_kit::{Hsla, hsla, rgb};
use money_core::format::hue_from_name;
use money_core::ledger::TRANSFER_CATEGORY_ID;
use money_core::models::CategoryStyle;

use crate::ui::Lucide;

/// The colours offered for a category, as stored (`#rrggbb`) and as named to
/// assistive technology. Mid-lightness, so each reads on a light and on a
/// dark surface.
pub const PALETTE: [(&str, &str); 8] = [
    ("#e8853b", "Orange"),
    ("#d9a514", "Yellow"),
    ("#2fb39a", "Teal"),
    ("#4c8df6", "Blue"),
    ("#9a6cf0", "Purple"),
    ("#e0569a", "Pink"),
    ("#e5604d", "Red"),
    ("#7f93a8", "Slate"),
];

/// The built-in icons a category can wear, by the key that is stored.
pub const ICONS: [(&str, Lucide); 16] = [
    ("tag", Lucide::Tag),
    ("utensils", Lucide::Utensils),
    ("coffee", Lucide::Coffee),
    ("shopping-bag", Lucide::ShoppingBag),
    ("car", Lucide::Car),
    ("house", Lucide::House),
    ("zap", Lucide::Zap),
    ("heart", Lucide::Heart),
    ("film", Lucide::Film),
    ("book", Lucide::Book),
    ("smartphone", Lucide::Smartphone),
    ("gift", Lucide::Gift),
    ("briefcase", Lucide::Briefcase),
    ("laptop", Lucide::Laptop),
    ("trending-up", Lucide::TrendingUp),
    ("wallet", Lucide::Wallet),
];

/// A category as it is drawn.
#[derive(Debug, Clone, Copy)]
pub struct CategoryLook {
    pub color: Hsla,
    pub icon: Lucide,
}

/// Parses `#rrggbb`. Anything else is not a colour this application stored.
fn parse_color(stored: &str) -> Option<Hsla> {
    let digits = stored.strip_prefix('#')?;
    if digits.len() != 6 {
        return None;
    }
    u32::from_str_radix(digits, 16)
        .ok()
        .map(|value| rgb(value).into())
}

/// The colour a name gets before anyone picks one.
fn derived_color(name: &str) -> Hsla {
    hsla(f32::from(hue_from_name(name)) / 360.0, 0.62, 0.52, 1.0)
}

/// The colour of a category. `muted` is what a transfer leg is drawn in: it
/// is not a category and should not compete with one.
pub fn color_of(style: &CategoryStyle, category_id: &[u8], name: &str, muted: Hsla) -> Hsla {
    if category_id == TRANSFER_CATEGORY_ID {
        return muted;
    }
    // A colour another client wrote in a form this one does not read is
    // treated as none, not as an error.
    style
        .color
        .as_deref()
        .and_then(parse_color)
        .unwrap_or_else(|| derived_color(name))
}

/// The icon of a category. A key this version does not have — a later
/// version's icon — is drawn as the default one.
pub fn icon_of(style: &CategoryStyle, category_id: &[u8]) -> Lucide {
    if category_id == TRANSFER_CATEGORY_ID {
        return Lucide::ArrowLeftRight;
    }
    style
        .icon
        .as_deref()
        .and_then(|key| ICONS.iter().find(|(name, _)| *name == key))
        .map_or(Lucide::Tag, |(_, icon)| *icon)
}

impl CategoryLook {
    pub fn of(style: &CategoryStyle, category_id: &[u8], name: &str, muted: Hsla) -> Self {
        CategoryLook {
            color: color_of(style, category_id, name, muted),
            icon: icon_of(style, category_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_colour_is_six_hex_digits_after_a_hash() {
        assert!(parse_color("#e8853b").is_some());
        assert!(parse_color("e8853b").is_none());
        assert!(parse_color("#e88").is_none());
        assert!(parse_color("#zzzzzz").is_none());
    }

    #[test]
    fn every_palette_colour_parses_and_every_icon_key_is_unique() {
        for (stored, _) in PALETTE {
            assert!(parse_color(stored).is_some(), "{stored}");
        }
        let mut keys: Vec<&str> = ICONS.iter().map(|(key, _)| *key).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ICONS.len());
    }

    #[test]
    fn an_unstyled_category_is_coloured_by_its_name() {
        assert_eq!(derived_color("Food"), derived_color("Food"));
        assert_ne!(derived_color("Food"), derived_color("Transport"));
    }

    #[test]
    fn a_style_this_version_cannot_read_falls_back_instead_of_failing() {
        let muted = hsla(0.0, 0.0, 0.5, 1.0);
        let style = CategoryStyle {
            color: Some("teal".into()),
            icon: Some("an-icon-from-the-future".into()),
        };
        assert_eq!(color_of(&style, &[1], "Food", muted), derived_color("Food"));
        assert_eq!(icon_of(&style, &[1]), Lucide::Tag);

        let style = CategoryStyle {
            color: Some("#4c8df6".into()),
            icon: Some("car".into()),
        };
        assert_eq!(color_of(&style, &[1], "Food", muted), rgb(0x4c8df6).into());
        assert_eq!(icon_of(&style, &[1]), Lucide::Car);
        // A transfer leg is never drawn as a category, whatever is passed.
        assert_eq!(color_of(&style, &TRANSFER_CATEGORY_ID, "x", muted), muted);
    }
}
