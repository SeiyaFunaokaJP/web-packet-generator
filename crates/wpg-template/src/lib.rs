//! # wpg-template
//!
//! The `.wpg.yaml` template format (TODO §2.4): metadata, variables, a layer stack
//! with per-field values and generators, stream / packet options and a transmit
//! profile. This crate loads and saves templates ([`Template::from_yaml_str`],
//! [`Template::from_path`], ...), **lints** them ([`lint()`]), **compiles** them into
//! a [`wpg_core::Packet`] plus a [`wpg_core::stream::StreamSpec`] ([`compile()`]),
//! renders streams to pcapng ([`render`]) and exports the JSON Schema
//! ([`Template::json_schema`]).
//!
//! ```yaml
//! schema: 1
//! meta:
//!   name: ARP sweep
//! layers:
//!   - protocol: ethernet2
//!     fields:
//!       dst: ff:ff:ff:ff:ff:ff
//! ```
//!
//! Layers are addressed by their 0-based index in the stack (`FieldRef.layer` of the
//! compiled stream is the decimal index); [`apply_overrides`] applies expansion
//! results to a packet.
//!
//! Only [`Template::from_path`] and [`Template::save`] touch the file system; the
//! rest is usable from the browser (wasm) as well.

use std::path::Path;

use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use wpg_core::Generator;

pub mod bundle;
mod compile;
pub mod import;
mod input;
mod lint;
mod pcapng;
pub mod pcapread;
pub mod render;

pub use bundle::{
    export_bundle, import_bundle, BundleError, BundleTemplate, ImportedBundle, Manifest,
};
pub use compile::{apply_overrides, compile, Compiled, VarOverrides};
pub use import::{
    import_capture, import_frames, import_hex, parse_hex, ImportError, ImportMode, ImportNote,
    ImportOptions, ImportResult,
};
pub use input::{parse_text, FieldInput};
pub use lint::{lint, LintIssue, LintLevel};
pub use pcapng::PcapngWriter;

/// Current template schema version written by this crate.
pub const SCHEMA_VERSION: u32 = 1;

/// Errors while reading, writing, compiling or rendering templates.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TemplateError {
    /// YAML syntax / shape error.
    #[error("invalid YAML template: {0}")]
    Yaml(#[from] serde_yaml::Error),
    /// JSON syntax / shape error.
    #[error("invalid JSON template: {0}")]
    Json(#[from] serde_json::Error),
    /// `schema:` is missing, older than 1 or newer than this build understands.
    #[error("unsupported template schema {found} (this build supports {SCHEMA_VERSION})")]
    UnsupportedSchema {
        /// Version found in the file.
        found: u32,
    },
    /// The file has no usable `schema:` key.
    #[error("template has no `schema:` version")]
    MissingSchema,
    /// A file could not be read or written.
    #[error("{path}: {source}")]
    File {
        /// The path.
        path: String,
        /// Underlying error.
        source: std::io::Error,
    },
    /// The template has errors (see [`LintIssue`]); also carries the warnings.
    #[error("{}", summarize(.0))]
    Lint(Vec<LintIssue>),
    /// A generator failed while expanding the stream.
    #[error("generator: {0}")]
    Generator(#[from] wpg_core::GeneratorError),
    /// Building a packet failed.
    #[error("build: {0}")]
    Build(#[from] wpg_core::CoreError),
    /// The stream is unbounded and no packet count was given.
    #[error("the stream is unbounded; specify a packet count")]
    Unbounded,
    /// Writing the output failed.
    #[error("write error: {0}")]
    Write(#[from] std::io::Error),
}

fn summarize(issues: &[LintIssue]) -> String {
    let errors: Vec<&LintIssue> = issues
        .iter()
        .filter(|i| i.level == LintLevel::Error)
        .collect();
    match errors.split_first() {
        Some((first, [])) => format!("{}: {}", first.path, first.message),
        Some((first, rest)) => format!(
            "{}: {} (and {} more error(s))",
            first.path,
            first.message,
            rest.len()
        ),
        None => "template has warnings only".to_owned(),
    }
}

/// Template metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Meta {
    /// Display name.
    pub name: String,
    /// Free-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Localised descriptions keyed by language tag (`ja`, `en`).
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub descriptions: IndexMap<String, String>,
    /// Tags for search / grouping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Creation timestamp (RFC 3339 or a plain date).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

/// A generator of a layer field: either a full [`Generator`] or the human shorthand
/// of the GUI (`192.168.1.1-192.168.5.254`, `10.0.0.0/24`, `1,5,9`, `00:11:22:*`).
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum GeneratorSpec {
    /// Full form, e.g. `{ kind: range, start: 192.168.1.1, end: 192.168.1.254 }`.
    Full(#[schemars(with = "GeneratorSchema")] Generator),
    /// Shorthand parsed with `wpg_core::notation::parse_generator`.
    Text(String),
}

impl<'de> Deserialize<'de> for GeneratorSpec {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        let mut v = serde_json::Value::deserialize(d)?;
        if let serde_json::Value::String(s) = v {
            return Ok(GeneratorSpec::Text(s));
        }
        stringify_operands(&mut v);
        serde_json::from_value(v)
            .map(GeneratorSpec::Full)
            .map_err(D::Error::custom)
    }
}

/// `min: 1024` is as good as `min: "1024"`: turns numeric operands into strings.
fn stringify_operands(v: &mut serde_json::Value) {
    use serde_json::Value;
    let scalar = |x: &mut Value| {
        if matches!(x, Value::Number(_) | Value::Bool(_)) {
            *x = Value::String(match &*x {
                Value::Bool(b) => u8::from(*b).to_string(),
                other => other.to_string(),
            });
        }
    };
    let Some(obj) = v.as_object_mut() else { return };
    let keys: &[&str] = match obj.get("kind").and_then(Value::as_str) {
        Some("range") => &["start", "end"],
        Some("random") => &["min", "max"],
        Some("random_mac") => &["oui"],
        _ => &[],
    };
    for k in keys {
        if let Some(x) = obj.get_mut(*k) {
            scalar(x);
        }
    }
    if let Some(Value::Array(values)) = obj.get_mut("values") {
        values.iter_mut().for_each(scalar);
    }
}

/// Stand-in for the `wpg_core::Generator` schema (which has no `JsonSchema` impl).
struct GeneratorSchema;

impl JsonSchema for GeneratorSchema {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Generator".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Field value generator, tagged by `kind`.",
            "oneOf": [
                {
                    "type": "object",
                    "required": ["kind", "start", "end"],
                    "properties": {
                        "kind": { "const": "range" },
                        "start": { "type": ["string", "integer"] },
                        "end": { "type": ["string", "integer"] },
                        "step": { "type": "integer", "minimum": 1, "default": 1 }
                    },
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "required": ["kind", "values"],
                    "properties": {
                        "kind": { "const": "list" },
                        "values": { "type": "array", "items": { "type": ["string", "integer"] } }
                    },
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "required": ["kind"],
                    "properties": {
                        "kind": { "const": "random" },
                        "min": { "type": ["string", "integer"] },
                        "max": { "type": ["string", "integer"] },
                        "seed": { "type": "integer", "minimum": 0 }
                    },
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "required": ["kind"],
                    "properties": {
                        "kind": { "const": "random_mac" },
                        "oui": { "type": "string" },
                        "seed": { "type": "integer", "minimum": 0 }
                    },
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "required": ["kind"],
                    "properties": {
                        "kind": { "const": "counter" },
                        "start": { "type": "integer", "minimum": 0, "default": 0 },
                        "step": { "type": "integer", "default": 1 },
                        "wrap": { "type": "integer", "minimum": 0 }
                    },
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "required": ["kind", "path"],
                    "properties": {
                        "kind": { "const": "file" },
                        "path": { "type": "string" }
                    },
                    "additionalProperties": false
                },
                {
                    "type": "object",
                    "required": ["kind", "expr"],
                    "properties": {
                        "kind": { "const": "expr" },
                        "expr": { "type": "string" }
                    },
                    "additionalProperties": false
                }
            ]
        })
    }
}

/// One layer of the template's packet.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateLayer {
    /// Protocol id from the catalog (e.g. `ethernet2`).
    pub protocol: String,
    /// Manually set fields in human notation (`192.168.1.1`, `0x0800`, `syn|ack`,
    /// `${src_ip}`). Unlisted fields use their default or Auto value; the literal
    /// `auto` also leaves a field Auto. TLV / list fields take structured values.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub fields: IndexMap<String, FieldInput>,
    /// Generators varying fields across the stream.
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub generators: IndexMap<String, GeneratorSpec>,
}

/// How generators of several fields are combined (see `wpg_core::stream::Mode`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StreamMode {
    /// Advance all generators in lockstep.
    #[default]
    Zip,
    /// Cartesian product, the last generator varies fastest.
    Product,
    /// Product with an explicit priority given by [`StreamOptions::order`].
    Nested,
}

/// Stream expansion options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StreamOptions {
    /// Combination mode of the generators.
    #[serde(default)]
    pub mode: StreamMode,
    /// For `nested`: fields from the slowest to the fastest, as `<layer>.<field>`
    /// where `<layer>` is a 0-based index or a protocol id (`ipv4.src`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<String>,
    /// Number of packets; defaults to `tx.count`, then to the natural length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Seed for random generators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

/// Frame check sequence choice.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FcsChoice {
    /// Append the correct CRC-32.
    Auto,
    /// Append a deliberately wrong CRC-32.
    Bad,
    /// No FCS (what capture / TX backends usually expect).
    #[default]
    None,
}

const fn yes() -> bool {
    true
}

/// Frame-level packet options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PacketOptions {
    /// Zero-pad to the minimum frame length (Ethernet: 60 octets).
    #[serde(default = "yes")]
    pub pad_to_min: bool,
    /// Frame check sequence.
    #[serde(default)]
    pub fcs: FcsChoice,
}

impl Default for PacketOptions {
    fn default() -> Self {
        Self {
            pad_to_min: true,
            fcs: FcsChoice::None,
        }
    }
}

/// How the stream is transmitted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TxProfile {
    /// Number of packets (`None` together with `duration_secs: None` = infinite).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u64>,
    /// Duration limit in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f64>,
    /// Rate in human notation: `1000pps`, `1Mpps`, `10Gbps`, `50%`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<String>,
    /// Burst size in packets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<u32>,
}

/// A template argument (`${name}` placeholder) overridable at run time.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Variable {
    /// Placeholder name.
    pub name: String,
    /// Default value (human notation; numbers and booleans are accepted too).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "scalar_opt"
    )]
    #[schemars(with = "Option<Scalar>")]
    pub default: Option<String>,
    /// Description shown in the GUI / CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A YAML scalar; variable defaults may be written as numbers.
#[derive(Deserialize, JsonSchema)]
#[serde(untagged)]
enum Scalar {
    Str(String),
    Uint(u64),
    Int(i64),
    Bool(bool),
}

fn scalar_opt<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(Option::<Scalar>::deserialize(d)?.map(|s| match s {
        Scalar::Str(s) => s,
        Scalar::Uint(v) => v.to_string(),
        Scalar::Int(v) => v.to_string(),
        Scalar::Bool(v) => u8::from(v).to_string(),
    }))
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

/// A `.wpg.yaml` template: one packet (+ generators) = one stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Template {
    /// Schema version, see [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Metadata.
    pub meta: Meta,
    /// Template variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Variable>,
    /// Layer stack, outermost first.
    #[serde(default)]
    pub layers: Vec<TemplateLayer>,
    /// Frame-level options (padding, FCS).
    #[serde(default, skip_serializing_if = "is_default")]
    pub packet: PacketOptions,
    /// Stream expansion options.
    #[serde(default, skip_serializing_if = "is_default")]
    pub stream: StreamOptions,
    /// Transmit profile.
    #[serde(default, skip_serializing_if = "is_default")]
    pub tx: TxProfile,
}

/// Migrates a parsed document of any supported schema version to the current
/// [`Template`]. Only `schema: 1` exists so far; future versions add a step to
/// [`MIGRATIONS`] (`MIGRATIONS[n - 1]` upgrades version `n` to `n + 1`).
///
/// # Errors
/// [`TemplateError::MissingSchema`], [`TemplateError::UnsupportedSchema`] or a shape
/// error of the (migrated) document.
pub fn migrate(mut value: serde_yaml::Value) -> Result<Template, TemplateError> {
    let mut version = schema_of(&value)?;
    if version == 0 || version > SCHEMA_VERSION {
        return Err(TemplateError::UnsupportedSchema { found: version });
    }
    while version < SCHEMA_VERSION {
        value = MIGRATIONS[(version - 1) as usize](value)?;
        version += 1;
    }
    Ok(serde_yaml::from_value(value)?)
}

/// Reads only the `schema:` version of a YAML or JSON template document, without
/// migrating or validating the rest (`wpg template migrate --check`).
///
/// # Errors
/// [`TemplateError::Yaml`] for a syntax error, [`TemplateError::MissingSchema`].
pub fn schema_version(src: &str) -> Result<u32, TemplateError> {
    schema_of(&serde_yaml::from_str(src)?)
}

fn schema_of(value: &serde_yaml::Value) -> Result<u32, TemplateError> {
    let found = value
        .get("schema")
        .and_then(serde_yaml::Value::as_u64)
        .ok_or(TemplateError::MissingSchema)?;
    Ok(u32::try_from(found).unwrap_or(u32::MAX))
}

/// One migration step: rewrites a document of version `n` into version `n + 1`
/// (including the `schema:` key).
type Migration = fn(serde_yaml::Value) -> Result<serde_yaml::Value, TemplateError>;

/// Migration steps; empty while only schema 1 exists.
const MIGRATIONS: &[Migration] = &[];

impl Template {
    /// Creates an empty template with the current schema version.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: SCHEMA_VERSION,
            meta: Meta {
                name: name.into(),
                ..Meta::default()
            },
            variables: Vec::new(),
            layers: Vec::new(),
            packet: PacketOptions::default(),
            stream: StreamOptions::default(),
            tx: TxProfile::default(),
        }
    }

    /// Parses a YAML template (migrating older schemas).
    ///
    /// # Errors
    /// Syntax errors or an unsupported schema version.
    pub fn from_yaml_str(src: &str) -> Result<Self, TemplateError> {
        migrate(serde_yaml::from_str(src)?)
    }

    /// Alias of [`Template::from_yaml_str`].
    ///
    /// # Errors
    /// See [`Template::from_yaml_str`].
    pub fn from_yaml(src: &str) -> Result<Self, TemplateError> {
        Self::from_yaml_str(src)
    }

    /// Serialises to YAML.
    ///
    /// # Errors
    /// Only if serialisation itself fails (should not happen for valid data).
    pub fn to_yaml_string(&self) -> Result<String, TemplateError> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Alias of [`Template::to_yaml_string`].
    ///
    /// # Errors
    /// See [`Template::to_yaml_string`].
    pub fn to_yaml(&self) -> Result<String, TemplateError> {
        self.to_yaml_string()
    }

    /// Parses a JSON template (migrating older schemas).
    ///
    /// # Errors
    /// Syntax errors or an unsupported schema version.
    pub fn from_json_str(src: &str) -> Result<Self, TemplateError> {
        let v: serde_json::Value = serde_json::from_str(src)?;
        migrate(serde_yaml::to_value(v)?)
    }

    /// Alias of [`Template::from_json_str`].
    ///
    /// # Errors
    /// See [`Template::from_json_str`].
    pub fn from_json(src: &str) -> Result<Self, TemplateError> {
        Self::from_json_str(src)
    }

    /// Serialises to pretty JSON.
    ///
    /// # Errors
    /// Only if serialisation itself fails (should not happen for valid data).
    pub fn to_json(&self) -> Result<String, TemplateError> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Reads a template file: JSON for `*.json` (`.wpg.json`), YAML otherwise
    /// (`.wpg.yaml`, `.yaml`, `.yml`).
    ///
    /// # Errors
    /// I/O errors, syntax errors or an unsupported schema version.
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self, TemplateError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| TemplateError::File {
            path: path.display().to_string(),
            source,
        })?;
        if is_json_path(path) {
            Self::from_json_str(&text)
        } else {
            Self::from_yaml_str(&text)
        }
    }

    /// Writes the template (format chosen by the extension like
    /// [`Template::from_path`]).
    ///
    /// # Errors
    /// I/O or serialisation errors.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<(), TemplateError> {
        let path = path.as_ref();
        let text = if is_json_path(path) {
            self.to_json()?
        } else {
            self.to_yaml_string()?
        };
        std::fs::write(path, text).map_err(|source| TemplateError::File {
            path: path.display().to_string(),
            source,
        })
    }

    /// The JSON Schema of the template file format (also published as
    /// `docs/schema/template.schema.json`).
    #[must_use]
    pub fn json_schema() -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(Template)).unwrap_or(serde_json::Value::Null)
    }
}

fn is_json_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARP_SWEEP: &str = r#"
schema: 1
meta:
  name: ARP sweep
  descriptions:
    ja: VLAN 越し ARP スイープ
    en: ARP sweep across a VLAN
  tags: [arp, sweep]
variables:
  - name: src_mac
    default: 02:00:00:00:00:01
layers:
  - protocol: ethernet2
    fields:
      dst: ff:ff:ff:ff:ff:ff
      src: ${src_mac}
  - protocol: arp
    generators:
      target_ip:
        kind: range
        start: 192.168.1.1
        end: 192.168.1.254
      sender_ip: 10.0.0.1-10.0.0.9
tx:
  count: 254
  rate: 1000pps
"#;

    fn sample() -> Template {
        let mut t = Template::new("sample");
        t.meta.tags = vec!["test".into()];
        t.layers.push(TemplateLayer {
            protocol: "ethernet2".into(),
            fields: IndexMap::from([("ethertype".into(), FieldInput::Str("0x0806".into()))]),
            generators: IndexMap::from([(
                "src".into(),
                GeneratorSpec::Full(Generator::List {
                    values: vec!["02:00:00:00:00:01".into(), "02:00:00:00:00:02".into()],
                }),
            )]),
        });
        t.tx.rate = Some("1Mpps".into());
        t.stream.mode = StreamMode::Product;
        t.packet.fcs = FcsChoice::Auto;
        t.variables.push(Variable {
            name: "iface".into(),
            ..Variable::default()
        });
        t
    }

    #[test]
    fn yaml_round_trip() {
        let t = sample();
        let yaml = t.to_yaml().unwrap();
        assert!(yaml.starts_with("schema: 1\n"), "{yaml}");
        assert_eq!(Template::from_yaml(&yaml).unwrap(), t);
    }

    #[test]
    fn json_round_trip() {
        let t = sample();
        assert_eq!(Template::from_json(&t.to_json().unwrap()).unwrap(), t);
    }

    #[test]
    fn parses_hand_written_yaml() {
        let t = Template::from_yaml(ARP_SWEEP).unwrap();
        assert_eq!(t.meta.name, "ARP sweep");
        assert_eq!(t.layers.len(), 2);
        assert_eq!(
            t.layers[1].generators["target_ip"],
            GeneratorSpec::Full(Generator::Range {
                start: "192.168.1.1".into(),
                end: "192.168.1.254".into(),
                step: 1
            })
        );
        assert_eq!(
            t.layers[1].generators["sender_ip"],
            GeneratorSpec::Text("10.0.0.1-10.0.0.9".into())
        );
        assert_eq!(t.tx.count, Some(254));
        assert_eq!(t.variables[0].name, "src_mac");
        // old files without stream / packet still load with defaults
        assert_eq!(t.stream, StreamOptions::default());
        assert_eq!(t.packet, PacketOptions::default());
        // and it survives a round trip through our own serializer
        assert_eq!(Template::from_yaml(&t.to_yaml().unwrap()).unwrap(), t);
    }

    #[test]
    fn rejects_unknown_schema() {
        let err = Template::from_yaml("schema: 2\nmeta: {name: x}\n").unwrap_err();
        assert!(matches!(err, TemplateError::UnsupportedSchema { found: 2 }));
        let err = Template::from_yaml("schema: 0\nmeta: {name: x}\n").unwrap_err();
        assert!(matches!(err, TemplateError::UnsupportedSchema { found: 0 }));
        let err = Template::from_yaml("meta: {name: x}\n").unwrap_err();
        assert!(matches!(err, TemplateError::MissingSchema));
    }

    #[test]
    fn reads_schema_version_only() {
        assert_eq!(schema_version("schema: 1\nbogus: [1]\n").unwrap(), 1);
        assert_eq!(schema_version("{\"schema\": 7}").unwrap(), 7);
        assert!(matches!(
            schema_version("meta: {}\n"),
            Err(TemplateError::MissingSchema)
        ));
        assert!(matches!(
            schema_version("schema: [\n"),
            Err(TemplateError::Yaml(_))
        ));
    }

    #[test]
    fn rejects_unknown_keys() {
        let err = Template::from_yaml("schema: 1\nmeta: {name: x}\nlayerz: []\n").unwrap_err();
        assert!(err.to_string().contains("layerz"), "{err}");
    }

    #[test]
    fn save_and_load_by_extension() {
        let dir = std::env::temp_dir().join(format!("wpg-template-io-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = sample();
        for name in ["t.wpg.yaml", "t.yaml", "t.wpg.json"] {
            let p = dir.join(name);
            t.save(&p).unwrap();
            assert_eq!(Template::from_path(&p).unwrap(), t, "{name}");
        }
        assert!(std::fs::read_to_string(dir.join("t.wpg.json"))
            .unwrap()
            .trim_start()
            .starts_with('{'));
        assert!(matches!(
            Template::from_path(dir.join("missing.wpg.yaml")),
            Err(TemplateError::File { .. })
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
