//! Password strength estimation (US-GEN-03), a thin wrapper over `zxcvbn`.
//!
//! The estimate is pattern-aware (dictionary words, keyboard walks, dates, repeats), unlike the naive
//! "pool size ^ length" figure from [`crate::generator::entropy_bits`]. It is a guide, not a guarantee.
use zxcvbn::zxcvbn;

/// Result of [`estimate`]. Holds no copy of the password, only derived text.
#[derive(Debug, Clone, PartialEq)]
pub struct Strength {
    /// 0 (trivially guessable) to 4 (very strong), as in zxcvbn.
    pub score: u8,
    /// log10 of the estimated number of guesses needed.
    pub guesses_log10: f64,
    /// Time to crack in an offline attack against a slow hash (10^4 guesses/second), e.g. "3 years".
    pub crack_time: String,
    /// Why the password is weak, if zxcvbn can say.
    pub warning: Option<String>,
    pub suggestions: Vec<String>,
}

impl Strength {
    /// Minimum score US-VLT-01 accepts for a new master password without an explicit override.
    pub const MIN_MASTER_SCORE: u8 = 2;

    pub fn label(&self) -> &'static str {
        match self.score {
            0 => "very weak",
            1 => "weak",
            2 => "fair",
            3 => "strong",
            _ => "very strong",
        }
    }
}

/// Estimate the strength of `password`. `user_inputs` are words an attacker could guess from context
/// (username, site title, email); passwords that contain them score lower.
pub fn estimate(password: &str, user_inputs: &[&str]) -> Strength {
    let e = zxcvbn(password, user_inputs);
    let (warning, suggestions) = match e.feedback() {
        Some(f) => (
            f.warning().map(|w| w.to_string()),
            f.suggestions().iter().map(|s| s.to_string()).collect(),
        ),
        None => (None, Vec::new()),
    };
    Strength {
        score: e.score() as u8,
        guesses_log10: e.guesses_log10(),
        crack_time: e
            .crack_times()
            .offline_slow_hashing_1e4_per_second()
            .to_string(),
        warning,
        suggestions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{generator, wordlist};
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

    #[test]
    fn common_passwords_score_low() {
        for p in ["", "password", "123456", "qwerty123", "letmein"] {
            assert!(estimate(p, &[]).score <= 1, "{p}");
        }
    }

    #[test]
    fn random_password_scores_high() {
        let mut r = ChaCha20Rng::seed_from_u64(7);
        let p = generator::generate_password(&Default::default(), &mut r).unwrap();
        assert_eq!(estimate(&p, &[]).score, 4);
    }

    #[test]
    fn six_word_eff_passphrase_scores_high() {
        let mut r = ChaCha20Rng::seed_from_u64(8);
        let p =
            generator::generate_passphrase(&wordlist::eff_large(), 6, "-", false, &mut r).unwrap();
        assert!(estimate(&p, &[]).score >= 3);
    }

    #[test]
    fn context_words_lower_the_score() {
        let plain = estimate("alice-github-2024", &[]);
        let aware = estimate("alice-github-2024", &["alice", "github"]);
        assert!(aware.guesses_log10 < plain.guesses_log10);
    }

    #[test]
    fn weak_password_has_feedback() {
        let s = estimate("password", &[]);
        assert!(s.warning.is_some() || !s.suggestions.is_empty());
        assert_eq!(s.label(), "very weak");
    }
}
