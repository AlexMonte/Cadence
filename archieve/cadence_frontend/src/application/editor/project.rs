pub fn slugify_identifier(value: &str) -> String {
    let normalized = value
        .trim()
        .to_lowercase()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '$' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    let trimmed = normalized.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "trick".to_string()
    } else {
        trimmed
    }
}
