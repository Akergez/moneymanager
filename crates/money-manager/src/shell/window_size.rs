use gpui_kit::{App, Pixels, Size, size};

/// The window a desktop opens, in rems: wide enough for the sidebar with its
/// labels and the table with its comments, at whatever size the interface is
/// drawn.
const INITIAL: (f32, f32) = (78.0, 50.0);

/// The smallest the window can be made, in rems: a phone held upright.
const MINIMUM: (f32, f32) = (21.0, 30.0);

/// `MONEY_MANAGER_SIZE=400x800` opens at a phone-sized window, which is the
/// only practical way to look at the collapsed layout without a phone. It is
/// in the platform's own units, as a screenshot of it will be.
pub(super) fn initial_size(cx: &App) -> Size<Pixels> {
    std::env::var("MONEY_MANAGER_SIZE")
        .ok()
        .and_then(|size| parse_size(&size))
        .map(|(width, height)| size(Pixels::from(width), Pixels::from(height)))
        .unwrap_or_else(|| in_rems(INITIAL, cx))
}

pub(super) fn minimum_size(cx: &App) -> Size<Pixels> {
    in_rems(MINIMUM, cx)
}

fn in_rems((width, height): (f32, f32), cx: &App) -> Size<Pixels> {
    let rem = crate::appearance::rem(cx);
    size(rem * width, rem * height)
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

    #[test]
    fn the_window_a_desktop_opens_has_room_for_the_widest_layout() {
        assert!(INITIAL.0 >= crate::shell::layout::DESKTOP_FROM);
        assert!(MINIMUM.0 < crate::shell::layout::PHONE_BELOW);
    }
}
