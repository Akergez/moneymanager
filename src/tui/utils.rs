//! Utility functions for TUI components

use ratatui::style::Color;

/// Format a number with space thousands separators and 2 decimal places,
/// e.g. `1234567.891 -> "1 234 567.89"`.
pub fn format_amount(value: f64) -> String {
    let cents = (value.abs() * 100.0).round() as u64;
    let sign = if value < 0.0 && cents > 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", int_with_spaces(&(cents / 100).to_string()), cents % 100)
}

/// Like [`format_amount`] but rounded to whole units: `"12 346"`.
pub fn format_amount_short(value: f64) -> String {
    let units = value.abs().round() as u64;
    let sign = if value < 0.0 && units > 0 { "-" } else { "" };
    format!("{sign}{}", int_with_spaces(&units.to_string()))
}

/// Display symbol for common currencies, otherwise the ISO code itself.
pub fn currency_symbol(currency: &str) -> &str {
    match currency {
        "RUB" => "₽",
        "USD" => "$",
        "EUR" => "€",
        other => other,
    }
}

/// Money with its currency: `"12 345.67 ₽"`, `"100.00 $"`, `"5.00 GBP"`.
pub fn format_money(value: f64, currency: &str) -> String {
    let amount = format_amount(value);
    if currency.is_empty() {
        amount
    } else {
        format!("{amount} {}", currency_symbol(currency))
    }
}

/// Insert space separators every 3 digits from the right.
fn int_with_spaces(s: &str) -> String {
    let len = s.len();
    if len <= 3 {
        return s.to_string();
    }
    let mut out = String::with_capacity(len + (len / 4) + 1);
    let first = if len.is_multiple_of(3) { 3 } else { len % 3 };
    out.push_str(&s[..first]);
    let mut i = first;
    while i + 3 <= len {
        out.push(' ');
        out.push_str(&s[i..i + 3]);
        i += 3;
    }
    out
}

/// Generate a deterministic color from a name using a simple hash.
/// This ensures the same name always produces the same color across the application.
pub fn color_from_name(name: &str) -> Color {
    // Simple hash function to get a deterministic value from the name
    let hash: u32 = name.bytes().fold(0u32, |acc, b| {
        acc.wrapping_mul(31).wrapping_add(b as u32)
    });
    
    // Generate HSL-like color with good saturation and lightness
    // Use the hash to determine hue (0-360)
    let hue = (hash % 360) as f64;
    
    // Convert HSL to RGB (fixed saturation=0.7, lightness=0.5 for vibrant colors)
    let saturation: f64 = 0.7;
    let lightness: f64 = 0.5;
    
    let c = (1.0_f64 - (2.0_f64 * lightness - 1.0_f64).abs()) * saturation;
    let x = c * (1.0_f64 - ((hue / 60.0_f64) % 2.0_f64 - 1.0_f64).abs());
    let m = lightness - c / 2.0_f64;
    
    let (r_prime, g_prime, b_prime) = match (hue as u32) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    
    let r = ((r_prime + m) * 255.0) as u8;
    let g = ((g_prime + m) * 255.0) as u8;
    let b = ((b_prime + m) * 255.0) as u8;
    
    Color::Rgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_amounts_with_separators_and_rounding() {
        assert_eq!(format_amount(0.0), "0.00");
        assert_eq!(format_amount(1234.5), "1 234.50");
        assert_eq!(format_amount(1234567.891), "1 234 567.89");
        assert_eq!(format_amount(0.999), "1.00");
        assert_eq!(format_amount(999.996), "1 000.00");
        assert_eq!(format_amount(-5.0), "-5.00");
        assert_eq!(format_amount(-0.001), "0.00");
        assert_eq!(format_amount_short(12345.67), "12 346");
    }

    #[test]
    fn formats_money_with_currency() {
        assert_eq!(format_money(12345.67, "RUB"), "12 345.67 ₽");
        assert_eq!(format_money(100.0, "USD"), "100.00 $");
        assert_eq!(format_money(-1.5, "EUR"), "-1.50 €");
        assert_eq!(format_money(5.0, "GBP"), "5.00 GBP");
    }
}

