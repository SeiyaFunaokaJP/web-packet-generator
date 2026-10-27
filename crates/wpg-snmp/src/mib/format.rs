//! Readable rendering of variable bindings with MIB knowledge: names
//! (`IF-MIB::ifDescr.3`), enumeration labels (`up(1)`), DISPLAY-HINTs
//! (RFC 2579 §3.1: `1x:`, `255a`, `2d-1d-1d,1d:1d:1d.1d,1a1d:1d`, `d-2`),
//! humanised TimeTicks, BITS names, UNITS, OID values as names, decoded
//! table indexes and tables assembled from walk results.

use std::collections::BTreeMap;
use std::net::Ipv4Addr;

use super::registry::{BaseType, Node, Registry, ResolvedSyntax};
use crate::oid::Oid;
use crate::value::{hex_spaced, Value, VarBind};

/// A rendered binding.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Formatted {
    /// OID (dotted).
    pub oid: String,
    /// `IF-MIB::ifDescr.3` (numeric when unknown).
    pub name: String,
    /// Defining module.
    pub module: Option<String>,
    /// Object descriptor (`ifDescr`).
    pub object: Option<String>,
    /// Instance sub-identifiers (`3`).
    pub instance: String,
    /// Wire type (`Counter32`, `OCTET STRING`, ...).
    pub value_type: String,
    /// Textual convention / named type of the object, if any.
    pub syntax: Option<String>,
    /// Readable value.
    pub value: String,
    /// Plain value without MIB knowledge.
    pub raw: String,
    /// Numeric value of integer types (rates, charts).
    pub numeric: Option<u64>,
    /// UNITS.
    pub units: Option<String>,
}

/// Renders one binding.
#[must_use]
pub fn format_varbind(reg: &Registry, vb: &VarBind) -> Formatted {
    let found = reg.lookup(&vb.oid);
    let (module, object, instance, syntax, units) = match found {
        Some((n, rest)) => (
            Some(n.module.clone()),
            Some(n.name.clone()),
            rest.iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join("."),
            n.syntax.clone(),
            n.units.clone(),
        ),
        None => (None, None, String::new(), None, None),
    };
    let value = format_value(reg, syntax.as_ref(), units.as_deref(), &vb.value);
    Formatted {
        oid: vb.oid.to_string(),
        name: reg.name_of(&vb.oid),
        module,
        object,
        instance,
        value_type: vb.value.type_name().to_owned(),
        syntax: syntax.as_ref().and_then(|s| s.type_name.clone()),
        value,
        raw: vb.value.to_string(),
        numeric: vb.value.as_u64(),
        units,
    }
}

/// Renders a value with an (optional) object syntax.
#[must_use]
pub fn format_value(
    reg: &Registry,
    syntax: Option<&ResolvedSyntax>,
    units: Option<&str>,
    v: &Value,
) -> String {
    let hint = syntax.and_then(|s| s.display_hint.as_deref());
    let with_units = |s: String| match units {
        Some(u) if !u.is_empty() => format!("{s} {u}"),
        _ => s,
    };
    match v {
        Value::Integer(i) => {
            if let Some(label) = syntax.and_then(|s| s.enum_label(*i)) {
                return format!("{label}({i})");
            }
            with_units(
                hint.and_then(|h| integer_hint(h, i128::from(*i)))
                    .unwrap_or_else(|| i.to_string()),
            )
        }
        Value::Counter32(x) | Value::Gauge32(x) => with_units(
            hint.and_then(|h| integer_hint(h, i128::from(*x)))
                .unwrap_or_else(|| x.to_string()),
        ),
        Value::Counter64(x) => with_units(
            hint.and_then(|h| integer_hint(h, i128::from(*x)))
                .unwrap_or_else(|| x.to_string()),
        ),
        Value::TimeTicks(t) => format!("{} ({t})", timeticks(*t)),
        Value::IpAddress(a) => Ipv4Addr::from(*a).to_string(),
        Value::Oid(o) => reg.name_of(o),
        Value::OctetString(b) => {
            if let Some(s) = syntax.filter(|s| s.base == BaseType::Bits) {
                return bits(b, &s.named);
            }
            if let Some(h) = hint {
                if let Some(s) = octet_hint(h, b) {
                    return with_units(s);
                }
            }
            with_units(auto_octets(b))
        }
        Value::Opaque(b) => hex_spaced(b),
        Value::Null => "NULL".into(),
        Value::NoSuchObject | Value::NoSuchInstance | Value::EndOfMibView => v.type_name().into(),
    }
}

/// Text when the octets are printable UTF-8, else hex.
#[must_use]
pub fn auto_octets(b: &[u8]) -> String {
    if b.is_empty() {
        return String::new();
    }
    match std::str::from_utf8(b) {
        Ok(s)
            if s.chars()
                .all(|c| !c.is_control() || matches!(c, '\n' | '\r' | '\t')) =>
        {
            s.trim_end_matches('\0').to_owned()
        }
        // A C string with a trailing NUL.
        _ if b.last() == Some(&0)
            && std::str::from_utf8(&b[..b.len() - 1])
                .is_ok_and(|s| s.chars().all(|c| !c.is_control())) =>
        {
            String::from_utf8_lossy(&b[..b.len() - 1]).into_owned()
        }
        _ => hex_spaced(b),
    }
}

/// `1d 02:03:04.56` for hundredths of a second.
#[must_use]
pub fn timeticks(t: u32) -> String {
    let cs = t % 100;
    let s = t / 100;
    let (d, h, m, sec) = (s / 86_400, (s / 3600) % 24, (s / 60) % 60, s % 60);
    if d > 0 {
        format!("{d}d {h:02}:{m:02}:{sec:02}.{cs:02}")
    } else {
        format!("{h:02}:{m:02}:{sec:02}.{cs:02}")
    }
}

/// Names of the set bits (`up(0) testing(2)`); bit 0 is the most significant
/// bit of the first octet.
#[must_use]
pub fn bits(b: &[u8], named: &[(i64, String)]) -> String {
    let mut out = Vec::new();
    for (i, byte) in b.iter().enumerate() {
        for bit in 0..8 {
            if byte & (0x80 >> bit) != 0 {
                let n = i64::try_from(i * 8 + bit).unwrap_or(i64::MAX);
                match named.iter().find(|(x, _)| *x == n) {
                    Some((_, name)) => out.push(format!("{name}({n})")),
                    None => out.push(format!("({n})")),
                }
            }
        }
    }
    if out.is_empty() {
        format!("[{}]", hex_spaced(b))
    } else {
        out.join(" ")
    }
}

/// INTEGER DISPLAY-HINT: `d`, `d-N` (N implied decimals), `x`, `o`, `b`.
#[must_use]
pub fn integer_hint(hint: &str, v: i128) -> Option<String> {
    let h = hint.trim();
    let (kind, rest) = h.split_at(h.char_indices().nth(1).map_or(h.len(), |(i, _)| i));
    match kind {
        "d" => {
            let decimals: usize = match rest.strip_prefix('-') {
                Some(n) => n.parse().ok()?,
                None if rest.is_empty() => 0,
                None => return None,
            };
            if decimals == 0 {
                return Some(v.to_string());
            }
            let neg = v < 0;
            let digits = v.unsigned_abs().to_string();
            let padded = format!("{digits:0>width$}", width = decimals + 1);
            let (int, frac) = padded.split_at(padded.len() - decimals);
            Some(format!("{}{int}.{frac}", if neg { "-" } else { "" }))
        }
        "x" => Some(format!("{v:x}")),
        "o" => Some(format!("{v:o}")),
        "b" => Some(format!("{v:b}")),
        _ => None,
    }
}

struct OctetSpec {
    repeat: bool,
    len: usize,
    fmt: char,
    sep: Option<char>,
    term: Option<char>,
}

fn parse_octet_hint(h: &str) -> Option<Vec<OctetSpec>> {
    let c: Vec<char> = h.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < c.len() {
        let repeat = c[i] == '*';
        if repeat {
            i += 1;
        }
        let s = i;
        while i < c.len() && c[i].is_ascii_digit() {
            i += 1;
        }
        let len: usize = c[s..i].iter().collect::<String>().parse().ok()?;
        let fmt = *c.get(i)?;
        if !matches!(fmt, 'a' | 't' | 'x' | 'd' | 'o') {
            return None;
        }
        i += 1;
        let mut sep = None;
        let mut term = None;
        if let Some(&ch) = c.get(i) {
            if !ch.is_ascii_digit() && ch != '*' {
                sep = Some(ch);
                i += 1;
                if repeat {
                    if let Some(&t) = c.get(i) {
                        if !t.is_ascii_digit() && t != '*' {
                            term = Some(t);
                            i += 1;
                        }
                    }
                }
            }
        }
        out.push(OctetSpec {
            repeat,
            len,
            fmt,
            sep,
            term,
        });
    }
    (!out.is_empty()).then_some(out)
}

/// OCTET STRING DISPLAY-HINT (RFC 2579 §3.1). The last specification is
/// reused until the data is consumed. `None` for an unusable hint.
#[must_use]
pub fn octet_hint(hint: &str, b: &[u8]) -> Option<String> {
    let specs = parse_octet_hint(hint)?;
    let mut out = String::new();
    let mut pos = 0;
    let mut si = 0;
    while pos < b.len() {
        let spec = &specs[si.min(specs.len() - 1)];
        si += 1;
        let reps = if spec.repeat {
            let r = usize::from(b[pos]);
            pos += 1;
            r
        } else {
            1
        };
        for r in 0..reps {
            if pos >= b.len() {
                break;
            }
            let take = spec.len.min(b.len() - pos);
            let chunk = &b[pos..pos + take];
            pos += take;
            match spec.fmt {
                'a' | 't' => out.push_str(&String::from_utf8_lossy(chunk)),
                'x' => {
                    for x in chunk {
                        out.push_str(&format!("{x:02x}"));
                    }
                }
                'd' | 'o' => {
                    let n = chunk.iter().fold(0u128, |a, x| (a << 8) | u128::from(*x));
                    out.push_str(&if spec.fmt == 'd' {
                        n.to_string()
                    } else {
                        format!("{n:o}")
                    });
                }
                _ => return None,
            }
            let more = pos < b.len();
            if more {
                if spec.repeat && r + 1 == reps {
                    if let Some(t) = spec.term {
                        out.push(t);
                        continue;
                    }
                }
                if let Some(s) = spec.sep {
                    out.push(s);
                }
            }
        }
        if spec.len == 0 && !spec.repeat {
            // A zero-length spec would loop forever.
            return None;
        }
    }
    Some(out)
}

/// Decodes the instance sub-identifiers of a row into readable index values
/// (one per INDEX object). `None` when the INDEX cannot be decoded.
#[must_use]
pub fn decode_index(reg: &Registry, row: &Node, suffix: &[u32]) -> Option<Vec<String>> {
    let items = reg.index_of(row);
    if items.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(items.len());
    let mut pos = 0;
    for (i, item) in items.iter().enumerate() {
        let node = reg.index_node(row, &item.name)?;
        let syn = node.syntax.as_ref()?;
        let last = i + 1 == items.len();
        let rest = suffix.get(pos..)?;
        let (used, text) = match syn.base {
            BaseType::Integer
            | BaseType::Unsigned
            | BaseType::Counter32
            | BaseType::TimeTicks
            | BaseType::Counter64 => {
                let x = *rest.first()?;
                let t = match syn.enum_label(i64::from(x)) {
                    Some(l) => format!("{l}({x})"),
                    None => x.to_string(),
                };
                (1, t)
            }
            BaseType::IpAddress => {
                let a = rest.get(..4)?;
                let o: Vec<u8> = a
                    .iter()
                    .map(|x| u8::try_from(*x).ok())
                    .collect::<Option<_>>()?;
                (4, Ipv4Addr::new(o[0], o[1], o[2], o[3]).to_string())
            }
            BaseType::OctetString | BaseType::Bits | BaseType::Opaque => {
                let (start, n) = if let Some(n) = syn.fixed_size {
                    (0, n)
                } else if item.implied && last {
                    (0, rest.len())
                } else {
                    (1, usize::try_from(*rest.first()?).ok()?)
                };
                let arcs = rest.get(start..start + n)?;
                let bytes: Vec<u8> = arcs
                    .iter()
                    .map(|x| u8::try_from(*x).ok())
                    .collect::<Option<_>>()?;
                let v = Value::OctetString(bytes);
                (start + n, format_value(reg, Some(syn), None, &v))
            }
            BaseType::ObjectIdentifier => {
                let (start, n) = if item.implied && last {
                    (0, rest.len())
                } else {
                    (1, usize::try_from(*rest.first()?).ok()?)
                };
                let arcs = rest.get(start..start + n)?;
                (start + n, reg.name_of(&Oid::from(arcs)))
            }
            _ => return None,
        };
        pos += used;
        out.push(text);
    }
    (pos == suffix.len()).then_some(out)
}

/// A conceptual table assembled from bindings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Table {
    /// `IF-MIB::ifEntry`.
    pub entry: String,
    /// `IF-MIB::ifTable` (the entry's parent), if defined.
    pub table: Option<String>,
    /// INDEX object names.
    pub index_names: Vec<String>,
    /// Columns that occur, in OID order.
    pub columns: Vec<TableColumn>,
    /// Rows in instance order.
    pub rows: Vec<TableRow>,
}

/// A column.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TableColumn {
    /// Descriptor.
    pub name: String,
    /// Column OID.
    pub oid: String,
    /// Type / TC.
    pub syntax: String,
    /// The column's values are counters (rates make sense).
    pub counter: bool,
}

/// A row.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TableRow {
    /// Instance sub-identifiers (`1`, `4.10.0.0.1`).
    pub instance: String,
    /// Decoded index values (empty when not decodable).
    pub index: Vec<String>,
    /// Readable cells, one per column.
    pub cells: Vec<Option<String>>,
    /// Numeric cells (counters / gauges), one per column.
    pub numeric: Vec<Option<u64>>,
}

/// Groups the bindings that are columns of a conceptual row into tables
/// (one per row type, most populated first).
#[must_use]
pub fn build_tables(reg: &Registry, vbs: &[VarBind]) -> Vec<Table> {
    // entry oid → (column oid → node), instance → column oid → value
    struct Acc<'a> {
        entry: &'a Node,
        columns: BTreeMap<Oid, &'a Node>,
        rows: BTreeMap<Oid, BTreeMap<Oid, &'a Value>>,
        count: usize,
    }
    let mut acc: BTreeMap<Oid, Acc<'_>> = BTreeMap::new();
    for vb in vbs {
        let Some((col, rest)) = reg.lookup(&vb.oid) else {
            continue;
        };
        if rest.is_empty() {
            continue;
        }
        let Some(entry_oid) = col.oid.parent() else {
            continue;
        };
        let Some(entry) = reg.node(&entry_oid).filter(|e| e.is_row()) else {
            continue;
        };
        let a = acc.entry(entry_oid).or_insert_with(|| Acc {
            entry,
            columns: BTreeMap::new(),
            rows: BTreeMap::new(),
            count: 0,
        });
        a.columns.insert(col.oid.clone(), col);
        a.rows
            .entry(Oid::from(rest))
            .or_default()
            .insert(col.oid.clone(), &vb.value);
        a.count += 1;
    }
    let mut tables: Vec<(usize, Table)> = acc
        .into_values()
        .map(|a| {
            let cols: Vec<(&Oid, &&Node)> = a.columns.iter().collect();
            let rows = a
                .rows
                .iter()
                .map(|(inst, cells)| TableRow {
                    instance: inst.to_string(),
                    index: decode_index(reg, a.entry, inst.arcs()).unwrap_or_default(),
                    cells: cols
                        .iter()
                        .map(|(o, n)| {
                            cells.get(*o).map(|v| {
                                format_value(reg, n.syntax.as_ref(), n.units.as_deref(), v)
                            })
                        })
                        .collect(),
                    numeric: cols
                        .iter()
                        .map(|(o, _)| cells.get(*o).and_then(|v| v.as_u64()))
                        .collect(),
                })
                .collect();
            let table = a
                .entry
                .oid
                .parent()
                .and_then(|p| reg.node(&p))
                .map(Node::qualified);
            (
                a.count,
                Table {
                    entry: a.entry.qualified(),
                    table,
                    index_names: reg
                        .index_of(a.entry)
                        .iter()
                        .map(|i| i.name.clone())
                        .collect(),
                    columns: cols
                        .iter()
                        .map(|(o, n)| TableColumn {
                            name: n.name.clone(),
                            oid: o.to_string(),
                            syntax: n
                                .syntax
                                .as_ref()
                                .map(ResolvedSyntax::describe)
                                .unwrap_or_default(),
                            counter: n.syntax.as_ref().is_some_and(|s| {
                                matches!(s.base, BaseType::Counter32 | BaseType::Counter64)
                            }),
                        })
                        .collect(),
                    rows,
                },
            )
        })
        .collect();
    tables.sort_by_key(|a| std::cmp::Reverse(a.0));
    tables.into_iter().map(|(_, t)| t).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hints() {
        assert_eq!(
            octet_hint("1x:", &[0, 0x1a, 0x2b, 0xff]).unwrap(),
            "00:1a:2b:ff"
        );
        assert_eq!(octet_hint("255a", b"eth0").unwrap(), "eth0");
        // DateAndTime 2026-10-27,13:05:09.4,+9:0
        let dt = [0x07, 0xea, 10, 27, 13, 5, 9, 4, b'+', 9, 0];
        assert_eq!(
            octet_hint("2d-1d-1d,1d:1d:1d.1d,1a1d:1d", &dt).unwrap(),
            "2026-10-27,13:5:9.4,+9:0"
        );
        assert_eq!(
            octet_hint("1d.1d.1d.1d/1d", &[10, 0, 0, 0, 8]).unwrap(),
            "10.0.0.0/8"
        );
        // Repeat indicator with terminator: two groups of 2 octets.
        assert_eq!(
            octet_hint("*1x:/", &[2, 0xaa, 0xbb, 1, 0xcc]).unwrap(),
            "aa:bb/cc"
        );
        assert_eq!(integer_hint("d-2", 1234).unwrap(), "12.34");
        assert_eq!(integer_hint("d-2", -5).unwrap(), "-0.05");
        assert_eq!(integer_hint("x", 255).unwrap(), "ff");
        assert_eq!(integer_hint("d", 7).unwrap(), "7");
        assert!(octet_hint("zz", b"a").is_none());
    }

    #[test]
    fn misc() {
        assert_eq!(timeticks(123_456), "00:20:34.56");
        assert_eq!(timeticks(8_640_000 + 360_000 + 1), "1d 01:00:00.01");
        assert_eq!(
            bits(&[0b1010_0000], &[(0, "a".into()), (2, "c".into())]),
            "a(0) c(2)"
        );
        assert_eq!(auto_octets(b"router1"), "router1");
        assert_eq!(auto_octets(&[0, 1, 0xff]), "00 01 ff");
        assert_eq!(auto_octets(b"abc\0"), "abc");
    }
}
