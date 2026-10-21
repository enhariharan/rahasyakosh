//! Bundled word lists for passphrase generation.

/// EFF "large" wordlist: 7,776 words (6^5, one per five dice rolls), one per line, in dice order.
/// Source: <https://www.eff.org/files/2016/07/18/eff_large_wordlist.txt> with the dice numbers removed.
const EFF_LARGE: &str = include_str!("wordlists/eff_large.txt");

/// Number of words in [`eff_large`].
pub const EFF_LARGE_LEN: usize = 7776;

/// The EFF large wordlist.
pub fn eff_large() -> Vec<&'static str> {
    EFF_LARGE.lines().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;

    #[test]
    fn eff_large_is_unmodified() {
        let digest = Sha256::digest(EFF_LARGE.as_bytes());
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            hex,
            "6d557f0693958fb5e650b68b5bee585eb82cf4da32965505c789e924743bc522"
        );
    }

    #[test]
    fn eff_large_has_7776_unique_lowercase_words() {
        let words = eff_large();
        assert_eq!(words.len(), EFF_LARGE_LEN);
        assert_eq!(words.iter().collect::<HashSet<_>>().len(), EFF_LARGE_LEN);
        assert!(words
            .iter()
            .all(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c == '-')));
    }

    #[test]
    fn eff_large_is_prefix_free() {
        let words = eff_large();
        for a in &words {
            assert!(!words.iter().any(|b| a != b && b.starts_with(a)), "{a}");
        }
    }
}
