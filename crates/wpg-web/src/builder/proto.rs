//! Read-only helpers over the in-browser protocol catalog
//! ([`wpg_protocols::catalog`]): field types, next-layer bindings and layer-order
//! suggestions. Pure and unit tested; used by the stack, field editor and preview.

use wpg_core::{FieldType, ProtocolDef};
use wpg_protocols::Catalog;

/// The in-browser catalog: the built-ins plus the server's user-defined
/// protocols once they were fetched (identical to the server's catalog).
#[must_use]
pub(crate) fn catalog() -> &'static Catalog {
    wpg_protocols::catalog()
}

/// The full definition of a protocol id.
#[must_use]
pub(crate) fn def(id: &str) -> Option<&'static ProtocolDef> {
    catalog().get(id)
}

/// The type of one field of a protocol.
#[must_use]
pub(crate) fn field_type(protocol: &str, field: &str) -> Option<FieldType> {
    def(protocol)?.field(field).map(|f| f.ty.clone())
}

/// Protocols this protocol can carry, in binding-table order (default last), deduped.
#[must_use]
pub(crate) fn next_protocols(protocol: &str) -> Vec<String> {
    let Some(b) = def(protocol).and_then(|d| d.next.as_ref()) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for e in b
        .table
        .iter()
        .map(|e| e.protocol.clone())
        .chain(b.default.clone())
    {
        if !out.contains(&e) {
            out.push(e);
        }
    }
    out
}

/// Protocols whose binding table can carry `id` (candidate parents), in palette order.
#[must_use]
pub(crate) fn parent_protocols(id: &str) -> Vec<String> {
    catalog()
        .iter()
        .filter(|p| {
            p.next
                .as_ref()
                .is_some_and(|b| b.table.iter().any(|e| e.protocol == id))
        })
        .map(|p| p.id.clone())
        .collect()
}

/// Whether `cur` announces `next` through its binding (so the selector stays Auto).
/// `payload` / `raw` and protocols with no binding never need fixing.
#[must_use]
pub(crate) fn binding_ok(cur: &str, next: &str) -> bool {
    if matches!(next, "payload" | "raw") {
        return true;
    }
    match def(cur).and_then(|d| d.next.as_ref()) {
        None => true,
        Some(b) => b.value_for(next).is_some(),
    }
}

/// The `(selector field, value)` that makes `cur` announce `next`, for the
/// "fix parent next-protocol" action.
#[must_use]
pub(crate) fn binding_fix(cur: &str, next: &str) -> Option<(String, u64)> {
    let b = def(cur)?.next.as_ref()?;
    b.value_for(next).map(|v| (b.field.clone(), v))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_bindings_resolve() {
        // Ethernet II carries IPv4 via the ethertype binding.
        assert!(def("ethernet2").is_some());
        assert!(next_protocols("ethernet2").iter().any(|p| p == "ipv4"));
        assert!(binding_ok("ethernet2", "ipv4"));
        let (field, value) = binding_fix("ethernet2", "ipv4").unwrap();
        assert_eq!(field, "ethertype");
        assert_eq!(value, 0x0800);
        // A payload always fits.
        assert!(binding_ok("ipv4", "payload"));
        // Parents of IPv4 include Ethernet II.
        assert!(parent_protocols("ipv4").iter().any(|p| p == "ethernet2"));
        // Field types come from the catalog.
        assert!(field_type("ipv4", "src").is_some());
        assert!(field_type("ipv4", "nope").is_none());
    }
}
