pub fn short_hex(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() <= 10 {
        return trimmed.to_string();
    }
    let prefix_len = 6.min(trimmed.len());
    let suffix_len = 4.min(trimmed.len().saturating_sub(prefix_len));
    let prefix = &trimmed[..prefix_len];
    let suffix = &trimmed[trimmed.len() - suffix_len..];
    format!("{}...{}", prefix, suffix)
}
