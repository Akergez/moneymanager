//! Utility functions for TUI components

use ratatui::style::Color;

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

