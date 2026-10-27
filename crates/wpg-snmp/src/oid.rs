//! Object identifiers.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// An OBJECT IDENTIFIER (`1.3.6.1.2.1.1.3.0`). Ordered lexicographically by
/// sub-identifier, which is the SNMP (GetNext) order.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid(Vec<u32>);

/// Error of [`Oid::from_str`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid OID `{0}`")]
pub struct OidParseError(pub String);

impl Oid {
    /// From sub-identifiers.
    #[must_use]
    pub fn new(arcs: impl Into<Vec<u32>>) -> Self {
        Self(arcs.into())
    }

    /// The sub-identifiers.
    #[must_use]
    pub fn arcs(&self) -> &[u32] {
        &self.0
    }

    /// Number of sub-identifiers.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// No sub-identifiers.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Whether `self` lies in the subtree rooted at `prefix` (or equals it).
    #[must_use]
    pub fn starts_with(&self, prefix: &Oid) -> bool {
        self.0.starts_with(&prefix.0)
    }

    /// `self` with `arcs` appended.
    #[must_use]
    pub fn child(&self, arcs: &[u32]) -> Oid {
        let mut v = self.0.clone();
        v.extend_from_slice(arcs);
        Oid(v)
    }

    /// Appends one sub-identifier.
    pub fn push(&mut self, arc: u32) {
        self.0.push(arc);
    }

    /// The parent (`None` for the empty OID).
    #[must_use]
    pub fn parent(&self) -> Option<Oid> {
        (!self.0.is_empty()).then(|| Oid(self.0[..self.0.len() - 1].to_vec()))
    }
}

impl From<&[u32]> for Oid {
    fn from(v: &[u32]) -> Self {
        Self(v.to_vec())
    }
}

impl<const N: usize> From<[u32; N]> for Oid {
    fn from(v: [u32; N]) -> Self {
        Self(v.to_vec())
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, a) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(".")?;
            }
            write!(f, "{a}")?;
        }
        Ok(())
    }
}

impl FromStr for Oid {
    type Err = OidParseError;

    /// Dotted decimal, an optional leading dot (`.1.3.6.1`).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let t = s.trim();
        let t = t.strip_prefix('.').unwrap_or(t);
        if t.is_empty() {
            return Err(OidParseError(s.to_owned()));
        }
        t.split('.')
            .map(|p| p.parse::<u32>().map_err(|_| OidParseError(s.to_owned())))
            .collect::<Result<Vec<_>, _>>()
            .map(Oid)
    }
}

impl Serialize for Oid {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Oid {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

/// Well-known OIDs used by the protocol machinery.
pub mod known {
    use super::Oid;

    /// `sysUpTime.0`.
    #[must_use]
    pub fn sys_uptime_0() -> Oid {
        Oid::from([1, 3, 6, 1, 2, 1, 1, 3, 0])
    }

    /// `snmpTrapOID.0`.
    #[must_use]
    pub fn snmp_trap_oid_0() -> Oid {
        Oid::from([1, 3, 6, 1, 6, 3, 1, 1, 4, 1, 0])
    }

    /// `snmpTrapEnterprise.0` (RFC 3584 §3.1).
    #[must_use]
    pub fn snmp_trap_enterprise_0() -> Oid {
        Oid::from([1, 3, 6, 1, 6, 3, 1, 1, 4, 3, 0])
    }

    /// `snmpTraps` (`coldStart` = `snmpTraps.1`, ...).
    #[must_use]
    pub fn snmp_traps() -> Oid {
        Oid::from([1, 3, 6, 1, 6, 3, 1, 1, 5])
    }

    /// `usmStats` (`usmStatsUnknownEngineIDs` = `usmStats.4.0`, ...).
    #[must_use]
    pub fn usm_stats() -> Oid {
        Oid::from([1, 3, 6, 1, 6, 3, 15, 1, 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_display_order() {
        let a: Oid = ".1.3.6.1.2.1.1.3.0".parse().unwrap();
        assert_eq!(a.to_string(), "1.3.6.1.2.1.1.3.0");
        assert!(a.starts_with(&"1.3.6.1".parse().unwrap()));
        assert!("1.3.6.1.2".parse::<Oid>().unwrap() < "1.3.6.1.10".parse().unwrap());
        assert!("1.3.6.1".parse::<Oid>().unwrap() < "1.3.6.1.0".parse().unwrap());
        assert!("1.x".parse::<Oid>().is_err());
        assert!("".parse::<Oid>().is_err());
        assert_eq!(a.parent().unwrap().to_string(), "1.3.6.1.2.1.1.3");
    }
}
