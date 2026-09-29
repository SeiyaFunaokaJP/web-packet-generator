//! # wpg-protocols
//!
//! Built-in protocol definitions expressed in the `wpg-core` IR, plus the
//! [`Catalog`] registry served to the GUI (`GET /api/protocols`).

use std::sync::OnceLock;

use serde::Serialize;
use wpg_core::ProtocolDef;

pub mod l2;

/// Registry of protocol definitions, in palette order.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Catalog {
    protocols: Vec<ProtocolDef>,
}

impl Catalog {
    /// Builds a catalog from definitions.
    ///
    /// # Panics
    /// If a definition is internally inconsistent or two share an id; built-in
    /// definitions are covered by tests so this never fires in practice.
    #[must_use]
    pub fn new(protocols: Vec<ProtocolDef>) -> Self {
        for (i, def) in protocols.iter().enumerate() {
            if let Err(e) = def.validate() {
                panic!("invalid protocol definition `{}`: {e}", def.id);
            }
            assert!(
                protocols[..i].iter().all(|p| p.id != def.id),
                "duplicate protocol id `{}`",
                def.id
            );
        }
        Self { protocols }
    }

    /// Looks up a protocol by id.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&ProtocolDef> {
        self.protocols.iter().find(|p| p.id == id)
    }

    /// Iterates over all protocols in palette order.
    pub fn iter(&self) -> impl Iterator<Item = &ProtocolDef> {
        self.protocols.iter()
    }

    /// Number of protocols.
    #[must_use]
    pub fn len(&self) -> usize {
        self.protocols.len()
    }

    /// Whether the catalog is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.protocols.is_empty()
    }

    /// All definitions as a slice.
    #[must_use]
    pub fn as_slice(&self) -> &[ProtocolDef] {
        &self.protocols
    }
}

/// The built-in protocol catalog (built once, then shared).
#[must_use]
pub fn catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| Catalog::new(vec![l2::ethernet2()]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_ethernet2_first() {
        let cat = catalog();
        assert!(!cat.is_empty());
        assert_eq!(cat.iter().next().map(|p| p.id.as_str()), Some("ethernet2"));
        assert!(cat.get("ethernet2").is_some());
        assert!(cat.get("no-such-proto").is_none());
    }

    #[test]
    fn catalog_serialises() {
        let json = serde_json::to_value(catalog()).unwrap();
        assert_eq!(json["protocols"][0]["id"], "ethernet2");
    }
}
