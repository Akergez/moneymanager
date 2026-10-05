use gpui_kit::{Pixels, px, size};

/// `MONEY_MANAGER_SIZE=400x800` opens at a phone-sized window, which is the
/// only practical way to look at the collapsed layout without a phone.
///
/// These are window dimensions handed to the platform, not layout: the one
/// place a pixel count is the thing itself.
pub(super) fn initial_size() -> gpui_kit::Size<Pixels> {
    let (width, height) = std::env::var("MONEY_MANAGER_SIZE")
        .ok()
        .and_then(|size| parse_size(&size))
        .unwrap_or((1240.0, 800.0));
    size(px(width), px(height))
}

/// The smallest the window can be made: a phone held upright.
pub(super) fn minimum_size() -> gpui_kit::Size<Pixels> {
    size(px(340.), px(480.))
}

fn parse_size(text: &str) -> Option<(f32, f32)> {
    let (width, height) = text.split_once('x')?;
    let (width, height): (f32, f32) = (width.trim().parse().ok()?, height.trim().parse().ok()?);
    (width > 0.0 && height > 0.0).then_some((width, height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_size_is_two_positive_numbers() {
        assert_eq!(parse_size("400x800"), Some((400.0, 800.0)));
        assert_eq!(parse_size(" 1320 x 840 "), Some((1320.0, 840.0)));
        assert_eq!(parse_size("400"), None);
        assert_eq!(parse_size("wide x tall"), None);
        assert_eq!(parse_size("0x800"), None);
    }
}
