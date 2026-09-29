//! # wpg-template
//!
//! The `.wpg.yaml` template format (TODO §2.4): metadata, a layer stack with per-field
//! values and generators, a transmit profile and template variables.
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

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use wpg_core::Generator;

/// Current template schema version written by this crate.
pub const SCHEMA_VERSION: u32 = 1;

/// Errors while reading or writing templates.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TemplateError {
    /// YAML syntax / shape error.
    #[error("invalid YAML template: {0}")]
    Yaml(#[from] serde_yaml::Error),
    /// JSON syntax / shape error.
    #[error("invalid JSON template: {0}")]
    Json(#[from] serde_json::Error),
    /// `schema:` is newer than this build understands (migrations arrive in M5).
    #[error("unsupported template schema {found} (this build supports {SCHEMA_VERSION})")]
    UnsupportedSchema {
        /// Version found in the file.
        found: u32,
    },
}

/// Template metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    /// Display name.
    pub name: String,
    /// Free-form description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Localised descriptions keyed by language tag (`ja`, `en`).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub descriptions: BTreeMap<String, String>,
    /// Tags for search / grouping.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Author.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Creation timestamp (RFC 3339).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

/// One layer of the template's packet.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateLayer {
    /// Protocol id from the catalog (e.g. `ethernet2`).
    pub protocol: String,
    /// Manually set fields in human notation (`192.168.1.1`, `0x0800`, `${src_ip}`).
    /// Unlisted fields use their default or Auto value.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    /// Generators varying fields across the stream.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub generators: BTreeMap<String, Generator>,
}

/// How the stream is transmitted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    /// Placeholder name.
    pub name: String,
    /// Default value (human notation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// Description shown in the GUI / CLI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A `.wpg.yaml` template: one packet (+ generators) = one stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Template {
    /// Schema version, see [`SCHEMA_VERSION`].
    pub schema: u32,
    /// Metadata.
    pub meta: Meta,
    /// Layer stack, outermost first.
    #[serde(default)]
    pub layers: Vec<TemplateLayer>,
    /// Transmit profile.
    #[serde(default)]
    pub tx: TxProfile,
    /// Template variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Variable>,
}

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
            layers: Vec::new(),
            tx: TxProfile::default(),
            variables: Vec::new(),
        }
    }

    fn check_schema(self) -> Result<Self, TemplateError> {
        if self.schema == SCHEMA_VERSION {
            Ok(self)
        } else {
            Err(TemplateError::UnsupportedSchema { found: self.schema })
        }
    }

    /// Parses a YAML template.
    ///
    /// # Errors
    /// Syntax errors or an unsupported schema version.
    pub fn from_yaml(src: &str) -> Result<Self, TemplateError> {
        serde_yaml::from_str::<Self>(src)?.check_schema()
    }

    /// Serialises to YAML.
    ///
    /// # Errors
    /// Only if serialisation itself fails (should not happen for valid data).
    pub fn to_yaml(&self) -> Result<String, TemplateError> {
        Ok(serde_yaml::to_string(self)?)
    }

    /// Parses a JSON template.
    ///
    /// # Errors
    /// Syntax errors or an unsupported schema version.
    pub fn from_json(src: &str) -> Result<Self, TemplateError> {
        serde_json::from_str::<Self>(src)?.check_schema()
    }

    /// Serialises to pretty JSON.
    ///
    /// # Errors
    /// Only if serialisation itself fails (should not happen for valid data).
    pub fn to_json(&self) -> Result<String, TemplateError> {
        Ok(serde_json::to_string_pretty(self)?)
    }
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
tx:
  count: 254
  rate: 1000pps
variables:
  - name: src_mac
    default: 02:00:00:00:00:01
"#;

    fn sample() -> Template {
        let mut t = Template::new("sample");
        t.meta.tags = vec!["test".into()];
        t.layers.push(TemplateLayer {
            protocol: "ethernet2".into(),
            fields: BTreeMap::from([("ethertype".into(), "0x0806".into())]),
            generators: BTreeMap::from([(
                "src".into(),
                Generator::List {
                    values: vec!["02:00:00:00:00:01".into(), "02:00:00:00:00:02".into()],
                },
            )]),
        });
        t.tx.rate = Some("1Mpps".into());
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
            Generator::Range {
                start: "192.168.1.1".into(),
                end: "192.168.1.254".into(),
                step: 1
            }
        );
        assert_eq!(t.tx.count, Some(254));
        assert_eq!(t.variables[0].name, "src_mac");
        // and it survives a round trip through our own serializer
        assert_eq!(Template::from_yaml(&t.to_yaml().unwrap()).unwrap(), t);
    }

    #[test]
    fn rejects_unknown_schema() {
        let err = Template::from_yaml("schema: 2\nmeta: {name: x}\n").unwrap_err();
        assert!(matches!(err, TemplateError::UnsupportedSchema { found: 2 }));
    }
}
