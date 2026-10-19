pub(crate) mod sketchybar;

/// Lowercase, collapse each run of non-alphanumeric characters to a single `-`,
/// and trim leading and trailing `-`.
pub(crate) fn slug(name: &str) -> String {
    let mut out = String::new();
    let mut in_sep = false;
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            in_sep = false;
        } else if !in_sep {
            out.push('-');
            in_sep = true;
        }
    }
    out.trim_matches('-').to_string()
}
