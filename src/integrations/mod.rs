pub(crate) mod sketchybar;
pub(crate) mod yasb;
pub(crate) mod zebar;

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

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slug_matches_scripts() {
        assert_eq!(slug("DELL SE2416H"), "dell-se2416h");
        assert_eq!(slug("AW2725DM"), "aw2725dm");
        assert_eq!(slug("  DELL   U2720Q #1  "), "dell-u2720q-1");
        assert_eq!(slug("---a---b---"), "a-b");
    }
}
