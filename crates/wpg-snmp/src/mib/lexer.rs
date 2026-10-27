//! Tokenizer for MIB modules (the ASN.1 subset of the SMI).
//!
//! Tolerant by design: comments run from `--` to the end of the line (as in
//! net-snmp; real MIBs are full of `-------` banner lines), identifiers may
//! contain `_` (common in vendor MIBs), stray characters become
//! [`Tok::Other`] instead of errors, and an unterminated string ends at the
//! end of the input.

/// A token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// Identifier or keyword (`ifDescr`, `OBJECT-TYPE`, `INTEGER`).
    Ident(String),
    /// Decimal number (possibly negative).
    Number(String),
    /// Quoted string (quotes removed).
    Str(String),
    /// `'0A'H` / `'0101'B` (content and the radix letter).
    Quoted(String, char),
    /// `::=`
    Assign,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `,`
    Comma,
    /// `;`
    Semi,
    /// `..`
    Range,
    /// `|`
    Pipe,
    /// Anything else (`.`, `:`, `<`, ...).
    Other(char),
}

/// A token with its 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The token.
    pub tok: Tok,
    /// Line number.
    pub line: u32,
}

impl Tok {
    /// The identifier text, if this is one.
    #[must_use]
    pub fn ident(&self) -> Option<&str> {
        match self {
            Tok::Ident(s) => Some(s),
            _ => None,
        }
    }

    /// Whether this is the identifier `kw`.
    #[must_use]
    pub fn is(&self, kw: &str) -> bool {
        matches!(self, Tok::Ident(s) if s == kw)
    }

    /// Source-like text (for constraint display).
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Tok::Ident(s) | Tok::Number(s) => s.clone(),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Quoted(s, r) => format!("'{s}'{r}"),
            Tok::Assign => "::=".into(),
            Tok::LBrace => "{".into(),
            Tok::RBrace => "}".into(),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::LBracket => "[".into(),
            Tok::RBracket => "]".into(),
            Tok::Comma => ",".into(),
            Tok::Semi => ";".into(),
            Tok::Range => "..".into(),
            Tok::Pipe => "|".into(),
            Tok::Other(c) => c.to_string(),
        }
    }
}

/// Splits `src` into tokens.
#[must_use]
pub fn tokenize(src: &str) -> Vec<Token> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    let mut line = 1u32;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() || c == '\u{feff}' {
            i += 1;
            continue;
        }
        let start_line = line;
        let push = |out: &mut Vec<Token>, tok| {
            out.push(Token {
                tok,
                line: start_line,
            });
        };
        // Comment to the end of the line.
        if c == '-' && chars.get(i + 1) == Some(&'-') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() {
            let s = i;
            while i < chars.len() {
                let d = chars[i];
                let hyphen = d == '-' && chars.get(i + 1) != Some(&'-');
                if d.is_ascii_alphanumeric() || d == '_' || hyphen {
                    i += 1;
                } else {
                    break;
                }
            }
            let mut word: String = chars[s..i].iter().collect();
            // `foo-` before a comment / delimiter: drop the dangling hyphen.
            while word.ends_with('-') {
                word.pop();
            }
            push(&mut out, Tok::Ident(word));
            continue;
        }
        if c.is_ascii_digit() || (c == '-' && chars.get(i + 1).is_some_and(char::is_ascii_digit)) {
            let s = i;
            i += 1;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            push(&mut out, Tok::Number(chars[s..i].iter().collect()));
            continue;
        }
        match c {
            '"' => {
                i += 1;
                let s = i;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    i += 1;
                }
                let text: String = chars[s..i.min(chars.len())].iter().collect();
                i += 1;
                push(&mut out, Tok::Str(text));
            }
            '\'' => {
                i += 1;
                let s = i;
                while i < chars.len() && chars[i] != '\'' && chars[i] != '\n' {
                    i += 1;
                }
                let text: String = chars[s..i.min(chars.len())].iter().collect();
                i += 1;
                let radix = chars.get(i).copied().unwrap_or('H').to_ascii_uppercase();
                if matches!(radix, 'H' | 'B') {
                    i += 1;
                }
                push(&mut out, Tok::Quoted(text, radix));
            }
            ':' if chars.get(i + 1) == Some(&':') && chars.get(i + 2) == Some(&'=') => {
                i += 3;
                push(&mut out, Tok::Assign);
            }
            '.' if chars.get(i + 1) == Some(&'.') => {
                i += 2;
                push(&mut out, Tok::Range);
            }
            _ => {
                i += 1;
                let tok = match c {
                    '{' => Tok::LBrace,
                    '}' => Tok::RBrace,
                    '(' => Tok::LParen,
                    ')' => Tok::RParen,
                    '[' => Tok::LBracket,
                    ']' => Tok::RBracket,
                    ',' => Tok::Comma,
                    ';' => Tok::Semi,
                    '|' => Tok::Pipe,
                    other => Tok::Other(other),
                };
                push(&mut out, tok);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        tokenize(s).into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn basics() {
        assert_eq!(
            toks(
                "ifIndex OBJECT-TYPE -- comment\n SYNTAX INTEGER (1..2147483647) ::= { ifEntry 1 }"
            ),
            vec![
                Tok::Ident("ifIndex".into()),
                Tok::Ident("OBJECT-TYPE".into()),
                Tok::Ident("SYNTAX".into()),
                Tok::Ident("INTEGER".into()),
                Tok::LParen,
                Tok::Number("1".into()),
                Tok::Range,
                Tok::Number("2147483647".into()),
                Tok::RParen,
                Tok::Assign,
                Tok::LBrace,
                Tok::Ident("ifEntry".into()),
                Tok::Number("1".into()),
                Tok::RBrace,
            ]
        );
        let t = tokenize("a\n\"multi\nline\" b ----------\n'0A'H vendor_name -5");
        assert_eq!(t[1].tok, Tok::Str("multi\nline".into()));
        assert_eq!(t[2].line, 3);
        assert_eq!(t[3].tok, Tok::Quoted("0A".into(), 'H'));
        assert_eq!(t[4].tok, Tok::Ident("vendor_name".into()));
        assert_eq!(t[5].tok, Tok::Number("-5".into()));
        // A comment glued to an identifier.
        assert_eq!(
            toks("foo--bar\nbaz"),
            vec![Tok::Ident("foo".into()), Tok::Ident("baz".into())]
        );
    }
}
