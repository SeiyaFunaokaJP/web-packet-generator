//! Field values as written in a template and their conversion to
//! [`wpg_core::FieldValue`].

use std::collections::BTreeMap;

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use wpg_core::notation::{self, parse_bytes, parse_uint};
use wpg_core::{FieldDef, FieldType, FieldValue, ItemDef, TlvDef, TlvItem};

/// A field value as written in YAML / JSON: a scalar in human notation or a
/// structured value for TLV (`options`) and list fields.
///
/// * scalars: `64`, `"192.168.1.1"`, `0x0800`, `syn|ack` (flag names), `ipv4` (enum
///   variant names), `auto`. Hex byte strings must be quoted (`"00 01 02"`);
/// * TLV: a list of items, each a bare variant name (`nop`), a one-key map
///   (`{ mss: 1460 }`, `{ timestamps: { tsval: 1, tsecr: 0 } }`) or the explicit form
///   `{ type: 99, data: "01 02" }` / `{ name: mss, length: 4, fields: { value: 1 } }`;
/// * list: a list of scalars, or of maps for lists of field groups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum FieldInput {
    /// Empty (`~`): e.g. an option without value.
    Null,
    /// `true` / `false` (1 / 0).
    Bool(bool),
    /// Unsigned integer.
    Uint(u64),
    /// Negative integer.
    Int(i64),
    /// Text in human notation.
    Str(String),
    /// Sequence (TLV items, list elements).
    List(Vec<FieldInput>),
    /// Mapping (TLV item, field group).
    Map(IndexMap<String, FieldInput>),
}

impl From<&str> for FieldInput {
    fn from(s: &str) -> Self {
        FieldInput::Str(s.to_owned())
    }
}

impl From<u64> for FieldInput {
    fn from(v: u64) -> Self {
        FieldInput::Uint(v)
    }
}

impl FieldInput {
    /// The text of a scalar value.
    #[must_use]
    pub fn scalar_text(&self) -> Option<String> {
        match self {
            FieldInput::Bool(b) => Some(u8::from(*b).to_string()),
            FieldInput::Uint(v) => Some(v.to_string()),
            FieldInput::Int(v) => Some(v.to_string()),
            FieldInput::Str(s) => Some(s.clone()),
            _ => None,
        }
    }

    /// True for the literal `auto` (case-insensitive).
    #[must_use]
    pub fn is_auto(&self) -> bool {
        matches!(self, FieldInput::Str(s) if s.trim().eq_ignore_ascii_case("auto"))
    }

    /// Applies `f` to every string; `Ok(None)` when `f` asked to skip one.
    pub(crate) fn map_strings<E>(
        &self,
        f: &mut impl FnMut(&str) -> Result<Option<String>, E>,
    ) -> Result<Option<FieldInput>, E> {
        Ok(Some(match self {
            FieldInput::Str(s) => match f(s)? {
                Some(s) => FieldInput::Str(s),
                None => return Ok(None),
            },
            FieldInput::List(items) => {
                let mut out = Vec::with_capacity(items.len());
                for i in items {
                    match i.map_strings(f)? {
                        Some(i) => out.push(i),
                        None => return Ok(None),
                    }
                }
                FieldInput::List(out)
            }
            FieldInput::Map(m) => {
                let mut out = IndexMap::new();
                for (k, v) in m {
                    match v.map_strings(f)? {
                        Some(v) => out.insert(k.clone(), v),
                        None => return Ok(None),
                    };
                }
                FieldInput::Map(out)
            }
            other => other.clone(),
        }))
    }

    /// Converts to a [`FieldValue`] of the field type `ty`.
    ///
    /// # Errors
    /// A message describing why the value does not fit the type.
    pub fn to_value(&self, ty: &FieldType) -> Result<FieldValue, String> {
        match ty {
            FieldType::Tlv(def) => {
                let FieldInput::List(items) = self else {
                    return Err("expected a list of TLV items".into());
                };
                items
                    .iter()
                    .enumerate()
                    .map(|(i, item)| tlv_item(item, def).map_err(|e| format!("item {i}: {e}")))
                    .collect::<Result<Vec<_>, _>>()
                    .map(FieldValue::Tlv)
            }
            FieldType::List { item, .. } => {
                let FieldInput::List(items) = self else {
                    return Err("expected a list".into());
                };
                let mut out = Vec::with_capacity(items.len());
                for (i, e) in items.iter().enumerate() {
                    let v = match item {
                        ItemDef::Field { ty } => e.to_value(ty),
                        ItemDef::Struct { fields } => match e {
                            FieldInput::Map(m) => group(m, fields).map(FieldValue::Struct),
                            _ => Err("expected a map of fields".into()),
                        },
                    };
                    out.push(v.map_err(|e| format!("element {i}: {e}"))?);
                }
                Ok(FieldValue::List(out))
            }
            FieldType::Padding { .. } => Err("padding is computed and cannot be set".into()),
            _ => {
                let text = self
                    .scalar_text()
                    .ok_or_else(|| "expected a single value".to_owned())?;
                parse_text(&text, ty)
            }
        }
    }
}

fn group(
    m: &IndexMap<String, FieldInput>,
    fields: &[FieldDef],
) -> Result<BTreeMap<String, FieldValue>, String> {
    let mut out = BTreeMap::new();
    for (k, v) in m {
        let def = fields
            .iter()
            .find(|f| &f.name == k)
            .ok_or_else(|| format!("unknown field `{k}`"))?;
        out.insert(
            k.clone(),
            v.to_value(&def.ty).map_err(|e| format!("`{k}`: {e}"))?,
        );
    }
    Ok(out)
}

const TLV_KEYS: [&str; 6] = ["name", "type", "length", "data", "raw", "fields"];

fn tlv_item(input: &FieldInput, def: &TlvDef) -> Result<TlvItem, String> {
    let known = || {
        def.variants
            .iter()
            .map(|v| v.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let variant = |name: &str| {
        def.variant_by_name(name.trim())
            .ok_or_else(|| format!("unknown option `{name}` (known: {})", known()))
    };
    match input {
        FieldInput::Str(name) => Ok(TlvItem::named(&variant(name)?.name)),
        FieldInput::Map(m)
            if m.len() == 1 && !TLV_KEYS.contains(&m.keys().next().unwrap().as_str()) =>
        {
            let (name, value) = m.iter().next().unwrap();
            let v = variant(name)?;
            let mut item = TlvItem::named(&v.name);
            match value {
                FieldInput::Null => {}
                FieldInput::Map(fields) => item.fields = group(fields, &v.fields)?,
                scalar => {
                    let [only] = v.fields.as_slice() else {
                        return Err(format!(
                            "`{name}` has {} value fields; write them as a map",
                            v.fields.len()
                        ));
                    };
                    item.fields
                        .insert(only.name.clone(), scalar.to_value(&only.ty)?);
                }
            }
            Ok(item)
        }
        FieldInput::Map(m) => {
            let mut item = TlvItem::default();
            let mut fields = None;
            for (k, v) in m {
                let num = || {
                    v.scalar_text()
                        .ok_or_else(|| format!("`{k}` must be a number"))
                        .and_then(|t| parse_uint(&t).map_err(|e| e.to_string()))
                        .and_then(|n| u64::try_from(n).map_err(|e| e.to_string()))
                };
                let bytes = || {
                    v.scalar_text()
                        .ok_or_else(|| format!("`{k}` must be a hex string"))
                        .and_then(|t| parse_bytes(&t).map_err(|e| e.to_string()))
                };
                match k.as_str() {
                    "name" => {
                        let name = v.scalar_text().ok_or("`name` must be text")?;
                        item.name = Some(variant(&name)?.name.clone());
                    }
                    "type" => item.ty = Some(num()?),
                    "length" => item.length = Some(num()?),
                    "data" => item.data = Some(bytes()?),
                    "raw" => item.raw = Some(bytes()?),
                    "fields" => match v {
                        FieldInput::Map(f) => fields = Some(f),
                        _ => return Err("`fields` must be a map".into()),
                    },
                    other => {
                        return Err(format!(
                            "unknown key `{other}` (use one of: {})",
                            TLV_KEYS.join(", ")
                        ))
                    }
                }
            }
            if let Some(f) = fields {
                let v = match (&item.name, item.ty) {
                    (Some(n), _) => def.variant_by_name(n),
                    (None, Some(t)) => def.variant_by_type(t),
                    (None, None) => None,
                }
                .ok_or("`fields` needs a known `name` or `type`")?;
                item.fields = group(f, &v.fields)?;
            }
            if item.name.is_none() && item.ty.is_none() && item.raw.is_none() {
                return Err("an item needs `name`, `type` or `raw`".into());
            }
            Ok(item)
        }
        _ => Err("expected an option name or a map".into()),
    }
}

/// Parses one scalar in human notation for `ty`, additionally accepting enum variant
/// names (`ipv4`) and `|`-separated flag names (`syn|ack`).
///
/// # Errors
/// A message describing the problem.
pub fn parse_text(s: &str, ty: &FieldType) -> Result<FieldValue, String> {
    let t = s.trim();
    match ty {
        FieldType::Enum { variants, .. } => {
            if let Some(v) = variants.iter().find(|v| v.name.eq_ignore_ascii_case(t)) {
                return Ok(FieldValue::UInt(v.value));
            }
            notation::parse_value(t, ty).map_err(|e| {
                let names: Vec<&str> = variants.iter().map(|v| v.name.as_str()).collect();
                format!("{e} (names: {})", names.join(", "))
            })
        }
        FieldType::Flags { bits, flags } => {
            if t.is_empty() {
                return Err("empty flags".into());
            }
            let mut acc = 0u64;
            for part in t.split('|') {
                let p = part.trim();
                if let Some(f) = flags.iter().find(|f| f.name.eq_ignore_ascii_case(p)) {
                    acc |= 1u64 << f.bit;
                } else if p.eq_ignore_ascii_case("none") {
                    // contributes nothing
                } else {
                    let v = parse_uint(p).map_err(|_| {
                        let names: Vec<&str> = flags.iter().map(|f| f.name.as_str()).collect();
                        format!("unknown flag `{p}` (names: {})", names.join(", "))
                    })?;
                    acc |= u64::try_from(v).map_err(|e| e.to_string())?;
                }
            }
            if *bits < 64 && acc >> *bits != 0 {
                return Err(format!("value {acc:#x} does not fit in {bits} bits"));
            }
            Ok(FieldValue::UInt(acc))
        }
        _ => notation::parse_value(t, ty).map_err(|e| e.to_string()),
    }
}

/// Replaces enum / flag names in a generator operand by their numeric value.
pub(crate) fn numeric_operand(s: &str, ty: &FieldType) -> Result<String, String> {
    if matches!(ty, FieldType::Enum { .. } | FieldType::Flags { .. }) {
        match parse_text(s, ty)? {
            FieldValue::UInt(v) => Ok(v.to_string()),
            _ => Ok(s.to_owned()),
        }
    } else {
        Ok(s.to_owned())
    }
}
