//! Password and passphrase generation using unbiased sampling from an injected CSPRNG.
use crate::{Error, Result};
use rand_core::CryptoRng;
use zeroize::Zeroizing;

const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &str = "0123456789";
const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.?/";
const AMBIGUOUS: &str = "O0Il1|";

#[derive(Debug, Clone)]
pub struct PasswordSpec {
    pub length: usize,
    pub lower: bool,
    pub upper: bool,
    pub digits: bool,
    pub symbols: bool,
    pub avoid_ambiguous: bool,
}

impl Default for PasswordSpec {
    fn default() -> Self {
        Self {
            length: 20,
            lower: true,
            upper: true,
            digits: true,
            symbols: true,
            avoid_ambiguous: false,
        }
    }
}

/// Uniform integer in `0..n` by rejection sampling (no modulo bias).
pub fn uniform(rng: &mut impl CryptoRng, n: usize) -> usize {
    assert!(n > 0 && n <= u32::MAX as usize);
    let n = n as u32;
    let zone = u32::MAX - (u32::MAX % n + 1) % n; // largest multiple of n minus 1
    loop {
        let x = rng.next_u32();
        if x <= zone {
            return (x % n) as usize;
        }
    }
}

pub fn generate_password(
    spec: &PasswordSpec,
    rng: &mut impl CryptoRng,
) -> Result<Zeroizing<String>> {
    let classes: Vec<Vec<char>> = [
        (spec.lower, LOWER),
        (spec.upper, UPPER),
        (spec.digits, DIGITS),
        (spec.symbols, SYMBOLS),
    ]
    .into_iter()
    .filter(|(on, _)| *on)
    .map(|(_, s)| {
        s.chars()
            .filter(|c| !(spec.avoid_ambiguous && AMBIGUOUS.contains(*c)))
            .collect()
    })
    .collect();

    if classes.is_empty() {
        return Err(Error::InvalidArgument(
            "select at least one character class".into(),
        ));
    }
    if spec.length < classes.len() || spec.length > 1024 {
        return Err(Error::InvalidArgument("length out of range".into()));
    }
    let all: Vec<char> = classes.iter().flatten().copied().collect();

    // One guaranteed char per class, rest from the union, then Fisher-Yates shuffle.
    let mut out: Zeroizing<Vec<char>> =
        Zeroizing::new(classes.iter().map(|c| c[uniform(rng, c.len())]).collect());
    while out.len() < spec.length {
        out.push(all[uniform(rng, all.len())]);
    }
    for i in (1..out.len()).rev() {
        out.swap(i, uniform(rng, i + 1));
    }
    Ok(Zeroizing::new(out.iter().collect()))
}

/// Passphrase of `words` words drawn from the caller-supplied list (e.g. EFF large list).
pub fn generate_passphrase(
    wordlist: &[&str],
    words: usize,
    separator: &str,
    capitalize: bool,
    rng: &mut impl CryptoRng,
) -> Result<Zeroizing<String>> {
    if wordlist.len() < 2 || !(3..=32).contains(&words) {
        return Err(Error::InvalidArgument(
            "need a wordlist and 3..=32 words".into(),
        ));
    }
    let picked: Vec<String> = (0..words)
        .map(|_| {
            let w = wordlist[uniform(rng, wordlist.len())];
            if capitalize {
                let mut c = w.chars();
                c.next()
                    .map(|f| f.to_uppercase().chain(c).collect())
                    .unwrap_or_default()
            } else {
                w.to_string()
            }
        })
        .collect();
    Ok(Zeroizing::new(picked.join(separator)))
}

/// Entropy in bits of a passphrase of `words` words drawn uniformly from a list of `list_len` words.
pub fn passphrase_entropy_bits(list_len: usize, words: usize) -> f64 {
    words as f64 * (list_len as f64).log2()
}

/// Rough entropy estimate in bits for a random password of this spec (upper bound for UI hints).
pub fn entropy_bits(spec: &PasswordSpec) -> f64 {
    let mut pool = 0usize;
    for (on, s) in [
        (spec.lower, LOWER),
        (spec.upper, UPPER),
        (spec.digits, DIGITS),
        (spec.symbols, SYMBOLS),
    ] {
        if on {
            pool += s
                .chars()
                .filter(|c| !(spec.avoid_ambiguous && AMBIGUOUS.contains(*c)))
                .count();
        }
    }
    if pool == 0 {
        0.0
    } else {
        spec.length as f64 * (pool as f64).log2()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

    #[test]
    fn password_has_each_class_and_length() {
        let mut r = ChaCha20Rng::seed_from_u64(1);
        for _ in 0..200 {
            let p = generate_password(
                &PasswordSpec {
                    length: 8,
                    ..Default::default()
                },
                &mut r,
            )
            .unwrap();
            assert_eq!(p.chars().count(), 8);
            assert!(p.chars().any(|c| c.is_ascii_lowercase()));
            assert!(p.chars().any(|c| c.is_ascii_uppercase()));
            assert!(p.chars().any(|c| c.is_ascii_digit()));
            assert!(p.chars().any(|c| SYMBOLS.contains(c)));
        }
    }

    #[test]
    fn ambiguous_excluded() {
        let mut r = ChaCha20Rng::seed_from_u64(2);
        let spec = PasswordSpec {
            length: 500,
            avoid_ambiguous: true,
            ..Default::default()
        };
        let p = generate_password(&spec, &mut r).unwrap();
        assert!(!p.chars().any(|c| AMBIGUOUS.contains(c)));
    }

    #[test]
    fn uniform_is_roughly_flat() {
        let mut r = ChaCha20Rng::seed_from_u64(3);
        let mut counts = [0u32; 6];
        for _ in 0..60_000 {
            counts[uniform(&mut r, 6)] += 1;
        }
        assert!(
            counts.iter().all(|&c| (9_000..11_000).contains(&c)),
            "{counts:?}"
        );
    }

    #[test]
    fn rejects_bad_specs() {
        let mut r = ChaCha20Rng::seed_from_u64(4);
        let none = PasswordSpec {
            lower: false,
            upper: false,
            digits: false,
            symbols: false,
            ..Default::default()
        };
        assert!(generate_password(&none, &mut r).is_err());
        assert!(generate_password(
            &PasswordSpec {
                length: 2,
                ..Default::default()
            },
            &mut r
        )
        .is_err());
    }

    #[test]
    fn eff_passphrase_entropy() {
        let bits = passphrase_entropy_bits(crate::wordlist::EFF_LARGE_LEN, 6);
        assert!((bits - 77.55).abs() < 0.01, "{bits}");
    }

    #[test]
    fn passphrase_from_eff_list() {
        let mut r = ChaCha20Rng::seed_from_u64(6);
        let list = crate::wordlist::eff_large();
        let p = generate_passphrase(&list, 6, " ", false, &mut r).unwrap();
        let picked: Vec<&str> = p.split(' ').collect();
        assert_eq!(picked.len(), 6);
        assert!(picked.iter().all(|w| list.contains(w)));
    }

    #[test]
    fn passphrase_format() {
        let mut r = ChaCha20Rng::seed_from_u64(5);
        let p = generate_passphrase(&["alpha", "bravo", "charlie"], 4, "-", true, &mut r).unwrap();
        assert_eq!(p.split('-').count(), 4);
        assert!(p
            .split('-')
            .all(|w| w.chars().next().unwrap().is_uppercase()));
    }
}
