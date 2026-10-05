//! How money is written out, and the colour a name gets when nobody chose one.

/// Format a number with space thousands separators and 2 decimal places,
/// e.g. `1234567.891 -> "1 234 567.89"`.
pub fn format_amount(value: f64) -> String {
    let cents = (value.abs() * 100.0).round() as u64;
    let sign = if value < 0.0 && cents > 0 { "-" } else { "" };
    format!(
        "{sign}{}.{:02}",
        int_with_spaces(&(cents / 100).to_string()),
        cents % 100
    )
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

/// Parses what a person types into an amount field: a comma is a decimal
/// point, spaces group thousands. Nothing else is forgiven.
pub fn parse_amount(text: &str) -> Option<f64> {
    let cleaned: String = text
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| if c == ',' { '.' } else { c })
        .collect();
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// A deterministic hue, 0–359, for a name: the same name is the same colour
/// on every device, which is what makes it usable before anyone picks one.
pub fn hue_from_name(name: &str) -> u16 {
    let hash: u32 = name
        .bytes()
        .fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    (hash % 360) as u16
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

    #[test]
    fn an_amount_is_typed_with_a_comma_or_a_point() {
        assert_eq!(parse_amount("12,5"), Some(12.5));
        assert_eq!(parse_amount(" 1 234.50 "), Some(1234.5));
        assert_eq!(parse_amount(""), None);
        assert_eq!(parse_amount("twelve"), None);
        assert_eq!(parse_amount("inf"), None);
    }

    #[test]
    fn a_name_always_gets_the_same_hue() {
        assert_eq!(hue_from_name("Food"), hue_from_name("Food"));
        assert!(hue_from_name("Food") < 360);
        assert_ne!(hue_from_name("Food"), hue_from_name("Transport"));
    }
}
