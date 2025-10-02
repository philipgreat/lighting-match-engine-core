/// Parses a human-readable string containing an optional size unit (k, M, G)
/// into a u32 integer.
///
/// Supported unit suffixes (case-insensitive):
/// - 'k' or 'K': Kilo (1,000)
/// - 'm' or 'M': Mega (1,000,000)
/// - 'g' or 'G': Giga (1,000,000,000)
///
/// # Arguments
/// * `s`: The string to parse, e.g., "10", "500k", "2m", "1G".
///
/// # Returns
/// Returns a `Result<u32, &'static str>`:
/// - `Ok(u32)` on success, containing the parsed value.
/// - `Err(&'static str)` on failure, with an error message.
pub fn parse_human_readable_u32(s: &str) -> Result<u32, &'static str> {
    // Trim whitespace and convert the string to lowercase for case-insensitive unit handling.
    let s_trimmed_lower = s.trim().to_lowercase();
    let s_bytes = s_trimmed_lower.as_bytes();

    // Check for empty input string.
    if s_bytes.is_empty() {
        return Err("Input string cannot be empty");
    }

    // Determine the number part and the potential unit character.
    let (number_str, unit_char) = match s_bytes.last() {
        Some(last_byte) if last_byte.is_ascii_alphabetic() => {
            // The last character is a letter, assume it's the unit
            let unit = *last_byte as char;
            let number = &s_trimmed_lower[..s_trimmed_lower.len() - 1];
            (number, Some(unit))
        }
        _ => {
            // The last character is not a letter, or the string is empty (handled above), no unit.
            (s_trimmed_lower.as_str(), None)
        }
    };

    // Parse the numerical part. Use u64 to prevent multiplication overflow against u32::MAX.
    let base_value: u64 = match number_str.parse() {
        Ok(v) => v,
        Err(_) => return Err("Failed to parse the number part"),
    };

    // Determine the multiplier based on the unit character.
    let multiplier: u64 = match unit_char {
        Some('k') => 1_000,
        Some('m') => 1_000_000,
        Some('g') => 1_000_000_000,
        Some(_) => return Err("Unsupported unit character"),
        None => 1, // No unit
    };

    // Calculate the final value.
    let final_value: u64 = base_value.saturating_mul(multiplier);

    // Check if the result safely fits into a u32.
    if final_value > u32::MAX as u64 {
        Err("Result value exceeds the maximum value for u32")
    } else {
        // Cast the value down to u32, which is safe due to the check above.
        Ok(final_value as u32)
    }
}
