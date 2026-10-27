//! Text from a vault or an import file is not trusted: a title can hold `ESC ] 52 ; c ; ... BEL` (write the
//! terminal's clipboard) or `ESC [ 2 J` (clear the screen), so what `rk` prints is shown with such characters
//! spelled out instead of sent to the terminal (audit finding P5-2).
use std::borrow::Cow;

/// Characters that must not reach a terminal as themselves: control characters (C0, DEL and the C1 range, which
/// includes the one-byte forms of the escape introducers), line and paragraph separators, and the bidirectional
/// overrides that make text read as something it is not.
fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{2028}' | '\u{2029}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

fn spell(c: char, out: &mut String) {
    match c {
        '\n' => out.push_str("\\n"),
        '\r' => out.push_str("\\r"),
        '\t' => out.push_str("\\t"),
        c if (c as u32) < 0x100 => out.push_str(&format!("\\x{:02x}", c as u32)),
        c => out.push_str(&format!("\\u{{{:x}}}", c as u32)),
    }
}

/// For one line: every unsafe character, newline and tab included, is replaced by a visible escape such as `\x1b`.
pub fn line(s: &str) -> Cow<'_, str> {
    if !s.chars().any(is_unsafe) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if is_unsafe(c) {
            spell(c, &mut out);
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// For text that may span lines (notes): newlines and tabs stay, everything else unsafe is escaped.
pub fn block(s: &str) -> Cow<'_, str> {
    if !s.chars().any(|c| is_unsafe(c) && !matches!(c, '\n' | '\t')) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if is_unsafe(c) && !matches!(c, '\n' | '\t') {
            spell(c, &mut out);
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_sequences_are_spelled_out() {
        let evil = "\x1b]52;c;aGk=\x07\x1b[2J";
        assert_eq!(line(evil), "\\x1b]52;c;aGk=\\x07\\x1b[2J");
        assert_eq!(line("a\u{9b}31m"), "a\\x9b31m");
        assert_eq!(line("x\u{202e}y"), "x\\u{202e}y");
        assert_eq!(line("two\nlines\there"), "two\\nlines\\there");
    }

    #[test]
    fn plain_text_in_any_script_is_untouched() {
        for s in [
            "GitHub",
            "नमस्ते",
            "வணக்கம்",
            "مرحبا",
            "日本語",
            "a b-c_d.e/f?g=h",
        ] {
            assert!(matches!(line(s), Cow::Borrowed(_)), "{s}");
            assert!(matches!(block(s), Cow::Borrowed(_)), "{s}");
        }
    }

    #[test]
    fn a_block_keeps_its_lines() {
        assert_eq!(block("one\ntwo\tthree\x1b[0m"), "one\ntwo\tthree\\x1b[0m");
    }
}
