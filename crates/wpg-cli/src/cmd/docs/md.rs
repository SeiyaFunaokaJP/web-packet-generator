//! Markdown and Fluent helpers of the documentation generator.

use std::cell::RefCell;
use std::collections::BTreeSet;

use wpg_i18n::{try_t, Locale};

use crate::cmd::send::tr;

thread_local! {
    /// `(locale, key)` pairs looked up but missing (checked by a test).
    static MISSING: RefCell<BTreeSet<String>> = const { RefCell::new(BTreeSet::new()) };
}

fn record(locale: Locale, key: &str) -> bool {
    let found = try_t(locale, key).is_some();
    if !found {
        MISSING.with(|m| m.borrow_mut().insert(format!("{locale}:{key}")));
    }
    found
}

/// Takes the set of missing messages recorded so far.
#[cfg(test)]
pub(crate) fn take_missing() -> BTreeSet<String> {
    MISSING.with(|m| std::mem::take(&mut *m.borrow_mut()))
}

/// Translated fixed text (the key itself, recorded as missing, when absent).
pub(crate) fn tx(locale: Locale, key: &str) -> String {
    try_t(locale, key).unwrap_or_else(|| {
        record(locale, key);
        key.to_owned()
    })
}

/// Translated fixed text with arguments.
pub(crate) fn txa(locale: Locale, key: &str, args: &[(&str, String)]) -> String {
    record(locale, key);
    tr(locale, key, args)
}

/// `` `s` `` (with a longer fence when `s` contains backticks).
pub(crate) fn code(s: &str) -> String {
    if s.contains('`') {
        format!("`` {s} ``")
    } else {
        format!("`{s}`")
    }
}

/// Plain text (Fluent messages, doc comments) as Markdown: `<`, `>`, `*`, `_`
/// and `[` outside code spans are escaped so that they show as written.
pub(crate) fn plain(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_code = false;
    for c in s.chars() {
        match c {
            '`' => {
                in_code = !in_code;
                out.push(c);
            }
            '<' if !in_code => out.push_str("&lt;"),
            '>' if !in_code => out.push_str("&gt;"),
            '*' | '_' | '[' if !in_code => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// Markdown from doc comments: kept as written except for `<` and `>` outside code
/// spans (`<data_dir>` would read as an HTML tag).
pub(crate) fn doc_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_code = false;
    for c in s.chars() {
        match c {
            '`' => {
                in_code = !in_code;
                out.push(c);
            }
            '<' if !in_code => out.push_str("&lt;"),
            '>' if !in_code => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Text safe inside a table cell: one line, `|` escaped.
pub(crate) fn cell(s: &str) -> String {
    let one_line = s.split_whitespace().collect::<Vec<_>>().join(" ");
    one_line.replace('|', "\\|")
}

/// A Markdown table.
pub(crate) fn table(out: &mut String, head: &[String], rows: &[Vec<String>]) {
    out.push_str("| ");
    out.push_str(&head.join(" | "));
    out.push_str(" |\n|");
    for _ in head {
        out.push_str("---|");
    }
    out.push('\n');
    for r in rows {
        out.push_str("| ");
        out.push_str(
            &r.iter()
                .map(|c| {
                    if c.is_empty() {
                        " ".to_owned()
                    } else {
                        cell(c)
                    }
                })
                .collect::<Vec<_>>()
                .join(" | "),
        );
        out.push_str(" |\n");
    }
    out.push('\n');
}

/// A heading of `level` (clamped to 1..=6; deeper ones become bold paragraphs).
pub(crate) fn heading(out: &mut String, level: usize, text: &str) {
    if level <= 6 {
        out.push_str(&"#".repeat(level.max(1)));
        out.push(' ');
        out.push_str(text);
    } else {
        out.push_str("**");
        out.push_str(text);
        out.push_str("**");
    }
    out.push_str("\n\n");
}

/// A paragraph (doc-comment text keeps its line breaks as soft breaks).
pub(crate) fn para(out: &mut String, text: &str) {
    let text = text.trim();
    if !text.is_empty() {
        out.push_str(text);
        out.push_str("\n\n");
    }
}

/// `2048 (0x0800)` for fields of 8 bits or more, plain decimal below.
pub(crate) fn num(v: u64, bits: Option<u8>) -> String {
    match bits {
        Some(b) if b >= 8 => {
            let digits = usize::from(b).div_ceil(4);
            format!("{v} (0x{v:0digits$x})")
        }
        _ => v.to_string(),
    }
}

/// Ends the page with exactly one newline.
pub(crate) fn finish(mut s: String) -> String {
    let len = s.trim_end().len();
    s.truncate(len);
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn helpers() {
        assert_eq!(code("a`b"), "`` a`b ``");
        assert_eq!(cell("a |\n b"), "a \\| b");
        assert_eq!(plain("<PRI> `a_b<c>` x_y"), "&lt;PRI&gt; `a_b<c>` x\\_y");
        assert_eq!(num(2048, Some(16)), "2048 (0x0800)");
        assert_eq!(num(6, Some(4)), "6");
        let mut t = String::new();
        table(
            &mut t,
            &["A".into(), "B".into()],
            &[vec!["1".into(), String::new()]],
        );
        assert_eq!(t, "| A | B |\n|---|---|\n| 1 |   |\n\n");
    }
}
