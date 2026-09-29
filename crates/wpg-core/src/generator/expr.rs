//! Tiny integer expression language for the `expr` generator.
//!
//! Grammar (C-like precedence, low to high): `|`, `^`, `&`, `<< >>`, `+ -`,
//! `* / %`, unary `- + ~`. Literals are decimal / `0x` / `0b`. `$field` and
//! `$layer.field` read other fields through a [`FieldLookup`]; `idx` is the packet
//! index. Functions: `hash(x)` (FNV-1a 64 over the 8 big-endian bytes of `x`),
//! `crc(x)` (CRC-32/IEEE over the same 8 bytes), `rand(min, max)` (deterministic per
//! packet index), `min(..)`, `max(..)`. Arithmetic is wrapping `i128`.

use crate::generator::rng::Rng;
use crate::generator::GeneratorError;
use crate::notation::parse_uint;
use crate::packet::FieldValue;

/// Resolves `$field` references to integers.
pub trait FieldLookup {
    /// Value of `name` (`field` or `layer.field`), if known.
    fn lookup(&self, name: &str) -> Option<i128>;
}

impl<F: Fn(&str) -> Option<i128>> FieldLookup for F {
    fn lookup(&self, name: &str) -> Option<i128> {
        self(name)
    }
}

/// A lookup that knows no fields.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoFields;

impl FieldLookup for NoFields {
    fn lookup(&self, _name: &str) -> Option<i128> {
        None
    }
}

/// Numeric view of a [`FieldValue`] for use in expressions (`None` for values wider
/// than 16 octets or non-numeric).
#[must_use]
pub fn field_value_to_i128(v: &FieldValue) -> Option<i128> {
    match v {
        FieldValue::UInt(x) => Some(i128::from(*x)),
        FieldValue::Int(x) => Some(i128::from(*x)),
        FieldValue::Ipv4(a) => Some(i128::from(u32::from(*a))),
        FieldValue::Ipv6(a) => i128::try_from(u128::from(*a)).ok(),
        FieldValue::Mac(m) => Some(m.iter().fold(0i128, |a, b| (a << 8) | i128::from(*b))),
        FieldValue::Bytes(b) | FieldValue::Nsap(b) if b.len() <= 15 => {
            Some(b.iter().fold(0i128, |a, x| (a << 8) | i128::from(*x)))
        }
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BinOp {
    Or,
    Xor,
    And,
    Shl,
    Shr,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

impl BinOp {
    fn prec(self) -> u8 {
        match self {
            BinOp::Or => 1,
            BinOp::Xor => 2,
            BinOp::And => 3,
            BinOp::Shl | BinOp::Shr => 4,
            BinOp::Add | BinOp::Sub => 5,
            BinOp::Mul | BinOp::Div | BinOp::Rem => 6,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Func {
    Hash,
    Crc,
    Min,
    Max,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Lit(i128),
    Idx,
    Field(String),
    Neg(Box<Node>),
    Not(Box<Node>),
    Bin(BinOp, Box<Node>, Box<Node>),
    Call(Func, Vec<Node>),
    Rand(u64, Box<Node>, Box<Node>),
}

/// A parsed expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    root: Node,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Num(i128),
    Ident(String),
    Field(String),
    Op(BinOp),
    Tilde,
    LParen,
    RParen,
    Comma,
}

fn err(msg: impl Into<String>) -> GeneratorError {
    GeneratorError::Expr(msg.into())
}

fn tokenize(src: &str) -> Result<Vec<Tok>, GeneratorError> {
    let cs: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < cs.len() {
        let c = cs[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '~' => {
                out.push(Tok::Tilde);
                i += 1;
            }
            '+' | '-' | '*' | '/' | '%' | '&' | '|' | '^' => {
                out.push(Tok::Op(match c {
                    '+' => BinOp::Add,
                    '-' => BinOp::Sub,
                    '*' => BinOp::Mul,
                    '/' => BinOp::Div,
                    '%' => BinOp::Rem,
                    '&' => BinOp::And,
                    '|' => BinOp::Or,
                    _ => BinOp::Xor,
                }));
                i += 1;
            }
            '<' | '>' => {
                if cs.get(i + 1) != Some(&c) {
                    return Err(err(format!("unexpected `{c}`")));
                }
                out.push(Tok::Op(if c == '<' { BinOp::Shl } else { BinOp::Shr }));
                i += 2;
            }
            '$' => {
                let start = i + 1;
                i = start;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_' || cs[i] == '.') {
                    i += 1;
                }
                if i == start {
                    return Err(err("`$` must be followed by a field name"));
                }
                out.push(Tok::Field(cs[start..i].iter().collect()));
            }
            c if c.is_ascii_digit() => {
                let start = i;
                while i < cs.len() && (cs[i].is_ascii_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                let text: String = cs[start..i].iter().collect();
                let v = parse_uint(&text)
                    .ok()
                    .and_then(|v| i128::try_from(v).ok())
                    .ok_or_else(|| err(format!("bad number `{text}`")))?;
                out.push(Tok::Num(v));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < cs.len() && (cs[i].is_alphanumeric() || cs[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(cs[start..i].iter().collect()));
            }
            other => return Err(err(format!("unexpected character `{other}`"))),
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    rand_id: u64,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn binary(&mut self, min_prec: u8) -> Result<Node, GeneratorError> {
        let mut lhs = self.unary()?;
        while let Some(Tok::Op(op)) = self.peek().cloned() {
            if op.prec() < min_prec {
                break;
            }
            self.pos += 1;
            let rhs = self.binary(op.prec() + 1)?;
            lhs = Node::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Node, GeneratorError> {
        match self.peek() {
            Some(Tok::Op(BinOp::Sub)) => {
                self.pos += 1;
                Ok(Node::Neg(Box::new(self.unary()?)))
            }
            Some(Tok::Op(BinOp::Add)) => {
                self.pos += 1;
                self.unary()
            }
            Some(Tok::Tilde) => {
                self.pos += 1;
                Ok(Node::Not(Box::new(self.unary()?)))
            }
            _ => self.primary(),
        }
    }

    fn primary(&mut self) -> Result<Node, GeneratorError> {
        match self.bump() {
            Some(Tok::Num(n)) => Ok(Node::Lit(n)),
            Some(Tok::Field(f)) => Ok(Node::Field(f)),
            Some(Tok::LParen) => {
                let n = self.binary(1)?;
                match self.bump() {
                    Some(Tok::RParen) => Ok(n),
                    _ => Err(err("missing `)`")),
                }
            }
            Some(Tok::Ident(name)) => {
                if self.peek() != Some(&Tok::LParen) {
                    return if name == "idx" {
                        Ok(Node::Idx)
                    } else {
                        Err(err(format!("unknown identifier `{name}`")))
                    };
                }
                self.pos += 1;
                let mut args = Vec::new();
                if self.peek() == Some(&Tok::RParen) {
                    self.pos += 1;
                } else {
                    loop {
                        args.push(self.binary(1)?);
                        match self.bump() {
                            Some(Tok::Comma) => {}
                            Some(Tok::RParen) => break,
                            _ => return Err(err("expected `,` or `)` in call")),
                        }
                    }
                }
                self.call(&name, args)
            }
            Some(t) => Err(err(format!("unexpected token {t:?}"))),
            None => Err(err("unexpected end of expression")),
        }
    }

    fn call(&mut self, name: &str, mut args: Vec<Node>) -> Result<Node, GeneratorError> {
        let arity = |n: usize, args: &Vec<Node>| {
            if args.len() == n {
                Ok(())
            } else {
                Err(err(format!("`{name}` takes {n} argument(s)")))
            }
        };
        match name {
            "hash" => {
                arity(1, &args)?;
                Ok(Node::Call(Func::Hash, args))
            }
            "crc" => {
                arity(1, &args)?;
                Ok(Node::Call(Func::Crc, args))
            }
            "rand" => {
                arity(2, &args)?;
                let hi = args.pop().unwrap_or(Node::Lit(0));
                let lo = args.pop().unwrap_or(Node::Lit(0));
                self.rand_id += 1;
                Ok(Node::Rand(self.rand_id, Box::new(lo), Box::new(hi)))
            }
            "min" | "max" => {
                if args.is_empty() {
                    return Err(err(format!("`{name}` needs at least one argument")));
                }
                Ok(Node::Call(
                    if name == "min" { Func::Min } else { Func::Max },
                    args,
                ))
            }
            _ => Err(err(format!("unknown function `{name}`"))),
        }
    }
}

const CRC_TABLE: [u32; 256] = {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        #[allow(clippy::cast_possible_truncation)]
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
};

/// CRC-32 (IEEE 802.3, as used by Ethernet FCS / zlib) of `data`.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for b in data {
        c = CRC_TABLE[usize::from((c as u8) ^ *b)] ^ (c >> 8);
    }
    !c
}

/// FNV-1a 64-bit hash of `data`.
#[must_use]
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for b in data {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn word_bytes(x: i128) -> [u8; 8] {
    #[allow(clippy::cast_possible_truncation)]
    (x as u64).to_be_bytes()
}

/// Evaluation environment.
#[derive(Clone, Copy)]
pub(crate) struct Env<'a> {
    pub(crate) index: u64,
    pub(crate) seed: u64,
    pub(crate) fields: &'a dyn FieldLookup,
}

impl Expr {
    /// Parses `src`.
    ///
    /// # Errors
    /// [`GeneratorError::Expr`] on syntax errors.
    pub fn parse(src: &str) -> Result<Expr, GeneratorError> {
        let toks = tokenize(src)?;
        let mut p = Parser {
            toks,
            pos: 0,
            rand_id: 0,
        };
        let root = p.binary(1)?;
        if p.pos < p.toks.len() {
            return Err(err("trailing input after expression"));
        }
        Ok(Expr { root })
    }

    /// Field names (`field` / `layer.field`) this expression reads.
    #[must_use]
    pub fn references(&self) -> Vec<String> {
        fn walk(n: &Node, out: &mut Vec<String>) {
            match n {
                Node::Field(f) => {
                    if !out.contains(f) {
                        out.push(f.clone());
                    }
                }
                Node::Neg(a) | Node::Not(a) => walk(a, out),
                Node::Bin(_, a, b) | Node::Rand(_, a, b) => {
                    walk(a, out);
                    walk(b, out);
                }
                Node::Call(_, args) => args.iter().for_each(|a| walk(a, out)),
                Node::Lit(_) | Node::Idx => {}
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }

    /// Evaluates with packet index `index`, a `seed` for `rand` and `fields`.
    ///
    /// # Errors
    /// [`GeneratorError::Expr`] for division by zero, bad shifts or unknown fields.
    pub fn eval(
        &self,
        index: u64,
        seed: u64,
        fields: &dyn FieldLookup,
    ) -> Result<i128, GeneratorError> {
        self.eval_env(&Env {
            index,
            seed,
            fields,
        })
    }

    pub(crate) fn eval_env(&self, env: &Env<'_>) -> Result<i128, GeneratorError> {
        eval(&self.root, env)
    }
}

fn eval(n: &Node, env: &Env<'_>) -> Result<i128, GeneratorError> {
    Ok(match n {
        Node::Lit(v) => *v,
        Node::Idx => i128::from(env.index),
        Node::Field(f) => env
            .fields
            .lookup(f)
            .ok_or_else(|| err(format!("unknown field `${f}`")))?,
        Node::Neg(a) => eval(a, env)?.wrapping_neg(),
        Node::Not(a) => !eval(a, env)?,
        Node::Bin(op, a, b) => {
            let (x, y) = (eval(a, env)?, eval(b, env)?);
            match op {
                BinOp::Add => x.wrapping_add(y),
                BinOp::Sub => x.wrapping_sub(y),
                BinOp::Mul => x.wrapping_mul(y),
                BinOp::Div | BinOp::Rem if y == 0 => return Err(err("division by zero")),
                BinOp::Div => x.wrapping_div(y),
                BinOp::Rem => x.wrapping_rem(y),
                BinOp::And => x & y,
                BinOp::Or => x | y,
                BinOp::Xor => x ^ y,
                BinOp::Shl | BinOp::Shr => {
                    let s = u32::try_from(y)
                        .ok()
                        .filter(|s| *s < 128)
                        .ok_or_else(|| err("shift amount out of range"))?;
                    if *op == BinOp::Shl {
                        x << s
                    } else {
                        x >> s
                    }
                }
            }
        }
        Node::Call(f, args) => {
            let vals = args
                .iter()
                .map(|a| eval(a, env))
                .collect::<Result<Vec<_>, _>>()?;
            match f {
                Func::Hash => i128::from(fnv1a64(&word_bytes(vals[0]))),
                Func::Crc => i128::from(crc32(&word_bytes(vals[0]))),
                Func::Min => vals.into_iter().min().unwrap_or(0),
                Func::Max => vals.into_iter().max().unwrap_or(0),
            }
        }
        Node::Rand(id, lo, hi) => {
            let (lo, hi) = (eval(lo, env)?, eval(hi, env)?);
            let (lo, hi) = if lo <= hi { (lo, hi) } else { (hi, lo) };
            let mut rng = Rng::for_index(env.seed ^ id.wrapping_mul(0x9E37_79B9), env.index);
            // Offset within the (possibly huge) span, computed in u128.
            let span = hi.wrapping_sub(lo) as u128;
            lo.wrapping_add(rng.range_u128(0, span) as i128)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(s: &str) -> i128 {
        Expr::parse(s).unwrap().eval(0, 0, &NoFields).unwrap()
    }

    #[test]
    fn precedence_and_literals() {
        assert_eq!(ev("1 + 2 * 3"), 7);
        assert_eq!(ev("(1 + 2) * 3"), 9);
        assert_eq!(ev("10 - 2 - 3"), 5);
        assert_eq!(ev("2 * 3 % 4"), 2);
        assert_eq!(ev("1 << 4 | 1"), 17);
        assert_eq!(ev("0xff & 0b1010"), 10);
        assert_eq!(ev("6 ^ 3"), 5);
        assert_eq!(ev("1 | 2 ^ 3 & 4"), 3);
        assert_eq!(ev("-2 + 5"), 3);
        assert_eq!(ev("~0"), -1);
        assert_eq!(ev("256 >> 4 + 0"), 16);
        assert_eq!(ev("7 / 2"), 3);
        assert_eq!(ev("- -3"), 3);
    }

    #[test]
    fn functions() {
        assert_eq!(ev("min(3, 1, 2)"), 1);
        assert_eq!(ev("max(3, 1, 2)"), 3);
        // FNV-1a of eight zero bytes.
        assert_eq!(ev("hash(0)"), i128::from(fnv1a64(&[0; 8])));
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(ev("crc(1)"), i128::from(crc32(&1u64.to_be_bytes())));
        for i in 0..50 {
            let v = Expr::parse("rand(5, 8)")
                .unwrap()
                .eval(i, 9, &NoFields)
                .unwrap();
            assert!((5..=8).contains(&v));
        }
        let a = Expr::parse("rand(0, 1000000)").unwrap();
        assert_eq!(
            a.eval(4, 1, &NoFields).unwrap(),
            a.eval(4, 1, &NoFields).unwrap()
        );
    }

    #[test]
    fn idx_and_fields() {
        let e = Expr::parse("$ipv4.src + idx * 2 - $ttl").unwrap();
        assert_eq!(
            e.references(),
            vec!["ipv4.src".to_owned(), "ttl".to_owned()]
        );
        let lk = |n: &str| match n {
            "ipv4.src" => Some(100),
            "ttl" => Some(4),
            _ => None,
        };
        assert_eq!(e.eval(3, 0, &lk).unwrap(), 102);
        assert!(e.eval(0, 0, &NoFields).is_err());
    }

    #[test]
    fn errors() {
        for bad in [
            "", "1 +", "(1", "1 2", "foo", "foo(1)", "hash()", "1 < 2", "$", "3 $",
        ] {
            assert!(Expr::parse(bad).is_err(), "{bad}");
        }
        assert!(Expr::parse("1 / 0").unwrap().eval(0, 0, &NoFields).is_err());
        assert!(Expr::parse("1 % 0").unwrap().eval(0, 0, &NoFields).is_err());
        assert!(Expr::parse("1 << 200")
            .unwrap()
            .eval(0, 0, &NoFields)
            .is_err());
    }

    #[test]
    fn value_conversion() {
        assert_eq!(field_value_to_i128(&FieldValue::UInt(5)), Some(5));
        assert_eq!(
            field_value_to_i128(&FieldValue::Mac([0, 0, 0, 0, 1, 0])),
            Some(256)
        );
        assert_eq!(field_value_to_i128(&FieldValue::Bytes(vec![0; 20])), None);
    }
}
