#[cfg(test)]
pub(crate) mod dir;
mod server;

pub use server::ServerTresseClient;

use tresse_lib::VersionLabel;

/// Parse a user-supplied version spec into a [`VersionLabel`].
///
/// Accepts the canonical `llllllll-hhhhhhhh` form or a raw hex `source` value.
pub fn parse_version_label(input: &str) -> Result<VersionLabel, String> {
    let trimmed = input.trim();
    if let Some((lamport, hash)) = trimmed.split_once('-') {
        let lamport = u32::from_str_radix(lamport, 16)
            .map_err(|_| format!("invalid version id '{input}'"))?;
        let hash =
            u32::from_str_radix(hash, 16).map_err(|_| format!("invalid version id '{input}'"))?;
        return Ok(VersionLabel { lamport, hash });
    }
    let raw = u64::from_str_radix(
        trimmed.trim_start_matches("0x").trim_start_matches("0X"),
        16,
    )
    .map_err(|_| format!("invalid version id '{input}' (expected llllllll-hhhhhhhh)"))?;
    Ok(VersionLabel::from_source(raw))
}
