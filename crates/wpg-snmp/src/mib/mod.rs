//! MIB support: [`lexer`] and [`parser`] for SMIv1 / SMIv2 modules, the
//! [`registry`] (OID ⇄ name, types, tables) with the bundled IETF modules, and
//! readable value [`format`]ting.
//!
//! The bundled modules (`crates/wpg-snmp/mibs/`, see its README) are IETF MIB
//! modules extracted from their RFCs (and the IANA-maintained IANAifType-MIB /
//! IANA-ENTITY-MIB), which the IETF Trust license permits to redistribute.
//! Vendor MIBs are uploaded by the user.

pub mod format;
pub mod lexer;
pub mod parser;
pub mod registry;

pub use format::{build_tables, format_value, format_varbind, Formatted, Table};
pub use registry::{
    BaseType, Child, ModuleInfo, ModuleStatus, Node, Registry, ResolveError, Source,
};

/// The bundled modules: (module name, text).
pub const BUNDLED: &[(&str, &str)] = &[
    ("RFC1155-SMI", include_str!("../../mibs/RFC1155-SMI.txt")),
    ("RFC-1212", include_str!("../../mibs/RFC-1212.txt")),
    ("RFC-1215", include_str!("../../mibs/RFC-1215.txt")),
    ("RFC1213-MIB", include_str!("../../mibs/RFC1213-MIB.txt")),
    ("SNMPv2-SMI", include_str!("../../mibs/SNMPv2-SMI.txt")),
    ("SNMPv2-TC", include_str!("../../mibs/SNMPv2-TC.txt")),
    ("SNMPv2-CONF", include_str!("../../mibs/SNMPv2-CONF.txt")),
    ("SNMPv2-MIB", include_str!("../../mibs/SNMPv2-MIB.txt")),
    (
        "SNMP-FRAMEWORK-MIB",
        include_str!("../../mibs/SNMP-FRAMEWORK-MIB.txt"),
    ),
    (
        "SNMP-USER-BASED-SM-MIB",
        include_str!("../../mibs/SNMP-USER-BASED-SM-MIB.txt"),
    ),
    (
        "IANAifType-MIB",
        include_str!("../../mibs/IANAifType-MIB.txt"),
    ),
    ("IF-MIB", include_str!("../../mibs/IF-MIB.txt")),
    (
        "INET-ADDRESS-MIB",
        include_str!("../../mibs/INET-ADDRESS-MIB.txt"),
    ),
    ("IP-MIB", include_str!("../../mibs/IP-MIB.txt")),
    (
        "HOST-RESOURCES-MIB",
        include_str!("../../mibs/HOST-RESOURCES-MIB.txt"),
    ),
    ("UUID-TC-MIB", include_str!("../../mibs/UUID-TC-MIB.txt")),
    (
        "IANA-ENTITY-MIB",
        include_str!("../../mibs/IANA-ENTITY-MIB.txt"),
    ),
    ("ENTITY-MIB", include_str!("../../mibs/ENTITY-MIB.txt")),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oid::Oid;
    use crate::value::{Value, VarBind};

    fn oid(s: &str) -> Oid {
        s.parse().unwrap()
    }

    #[test]
    fn bundled_modules_load_cleanly() {
        let r = Registry::bundled();
        let mods = r.modules();
        assert_eq!(
            mods.len(),
            BUNDLED.len(),
            "{:?}",
            mods.iter().map(|m| &m.name).collect::<Vec<_>>()
        );
        for m in &mods {
            assert_eq!(
                m.status,
                ModuleStatus::Ok,
                "{}: {:?} {:?}",
                m.name,
                m.missing_imports,
                m.errors
            );
            // These only define macros / textual conventions.
            let no_objects = matches!(
                m.name.as_str(),
                "RFC-1212" | "RFC-1215" | "SNMPv2-CONF" | "SNMPv2-TC"
            );
            assert!(m.nodes > 0 || no_objects, "{} has no nodes", m.name);
        }
        assert!(r.node_count() > 700, "{}", r.node_count());
    }

    #[test]
    fn names_and_oids() {
        let r = Registry::bundled();
        assert_eq!(r.resolve("sysUpTime.0").unwrap(), oid("1.3.6.1.2.1.1.3.0"));
        assert_eq!(
            r.resolve("IF-MIB::ifDescr.3").unwrap(),
            oid("1.3.6.1.2.1.2.2.1.2.3")
        );
        assert_eq!(r.resolve(".1.3.6.1.2.1").unwrap(), oid("1.3.6.1.2.1"));
        assert_eq!(
            r.resolve("ifHCInOctets").unwrap(),
            oid("1.3.6.1.2.1.31.1.1.1.6")
        );
        assert_eq!(
            r.resolve("hrStorageTable").unwrap(),
            oid("1.3.6.1.2.1.25.2.3")
        );
        assert_eq!(
            r.resolve("entPhysicalDescr").unwrap(),
            oid("1.3.6.1.2.1.47.1.1.1.1.2")
        );
        assert_eq!(
            r.resolve("ipAdEntAddr").unwrap(),
            oid("1.3.6.1.2.1.4.20.1.1")
        );
        assert_eq!(
            r.resolve("usmStatsWrongDigests.0").unwrap(),
            oid("1.3.6.1.6.3.15.1.1.5.0")
        );
        assert!(matches!(
            r.resolve("noSuchThing"),
            Err(ResolveError::UnknownName(_))
        ));
        assert!(matches!(
            r.resolve("NOPE-MIB::x"),
            Err(ResolveError::UnknownModule(_))
        ));
        assert!(matches!(
            r.resolve("ifDescr.x"),
            Err(ResolveError::Invalid(_))
        ));
        assert_eq!(
            r.name_of(&oid("1.3.6.1.2.1.2.2.1.2.3")),
            "IF-MIB::ifDescr.3"
        );
        assert_eq!(
            r.name_of(&oid("1.3.6.1.2.1.1.1.0")),
            "SNMPv2-MIB::sysDescr.0"
        );
        assert_eq!(
            r.name_of(&oid("1.3.6.1.4.1.99999.1")),
            "SNMPv2-SMI::enterprises.99999.1"
        );
        // SMIv2 wins over the RFC1213-MIB duplicate.
        assert_eq!(
            r.node(&oid("1.3.6.1.2.1.2.2.1.8")).unwrap().module,
            "IF-MIB"
        );
        let kids = r.children(&oid("1.3.6.1.2.1.2.2.1"));
        assert!(kids
            .iter()
            .any(|c| c.name.as_deref() == Some("ifOperStatus")));
        let roots = r.children(&Oid::default());
        assert!(roots.iter().any(|c| c.arc == 1 && c.has_children));
        let s = r.search("ifOper", 5);
        assert_eq!(s[0].name, "ifOperStatus");
        assert!(r
            .search("IF-MIB::descr", 10)
            .iter()
            .all(|n| n.module == "IF-MIB"));
    }

    #[test]
    fn module_tree() {
        let r = Registry::bundled();
        let roots = r.module_roots("IF-MIB");
        let names: Vec<_> = roots.iter().filter_map(|c| c.name.as_deref()).collect();
        // IF-MIB defines `interfaces` itself (ifNumber, ifTable sit below it)
        // and its notifications under snmpTraps.
        assert_eq!(names, ["interfaces", "ifMIB", "linkDown", "linkUp"]);
        assert!(roots.iter().all(|c| c.module.as_deref() == Some("IF-MIB")));
        // No root lies below another one.
        assert!(roots.iter().all(|a| roots
            .iter()
            .all(|b| a.oid == b.oid || !a.oid.starts_with(&b.oid))));
        assert!(r.module_roots("NOPE-MIB").is_empty());

        // Below a root: only branches holding IF-MIB definitions.
        let if_mib = oid("1.3.6.1.2.1.31");
        let kids = r.children_in(&if_mib, Some("IF-MIB"));
        assert!(!kids.is_empty());
        assert!(kids.len() <= r.children(&if_mib).len());
        // An intermediate entry from another module still leads to IF-MIB nodes.
        let mib2 = r.children_in(&oid("1.3.6.1.2.1"), Some("IF-MIB"));
        assert!(mib2.iter().any(|c| c.arc == 2 && c.has_children));
        assert!(mib2.iter().all(|c| c.arc == 2 || c.arc == 31), "{mib2:?}");
        assert!(r
            .children_in(&oid("1.3.6.1.2.1.1"), Some("IF-MIB"))
            .is_empty());
    }

    #[test]
    fn readable_values() {
        let r = Registry::bundled();
        let f = |o: &str, v: Value| format_varbind(&r, &VarBind::new(oid(o), v));
        let x = f("1.3.6.1.2.1.2.2.1.8.1", Value::Integer(1));
        assert_eq!(x.value, "up(1)");
        assert_eq!(x.name, "IF-MIB::ifOperStatus.1");
        assert_eq!(x.object.as_deref(), Some("ifOperStatus"));
        assert_eq!(x.instance, "1");
        assert_eq!(
            f(
                "1.3.6.1.2.1.2.2.1.6.2",
                Value::OctetString(vec![0, 0x1b, 0x21, 0xaa, 0xbb, 0xcc])
            )
            .value,
            "00:1b:21:aa:bb:cc"
        );
        assert_eq!(
            f("1.3.6.1.2.1.1.3.0", Value::TimeTicks(12_345_678)).value,
            "1d 10:17:36.78 (12345678)"
        );
        assert_eq!(
            f("1.3.6.1.2.1.1.2.0", Value::Oid(oid("1.3.6.1.2.1.2.2.1.1"))).value,
            "IF-MIB::ifIndex"
        );
        assert_eq!(
            f("1.3.6.1.2.1.2.2.1.3.1", Value::Integer(6)).value,
            "ethernetCsmacd(6)"
        );
        assert_eq!(
            f("1.3.6.1.2.1.2.2.1.5.1", Value::Gauge32(1000)).value,
            "1000"
        );
        let mem = f("1.3.6.1.2.1.25.2.2.0", Value::Integer(8048));
        assert_eq!(mem.value, "8048 KBytes");
        assert_eq!(mem.units.as_deref(), Some("KBytes"));
        assert_eq!(
            f(
                "1.3.6.1.2.1.25.1.2.0",
                Value::OctetString(vec![0x07, 0xea, 10, 27, 13, 5, 9, 4, b'+', 9, 0])
            )
            .value,
            "2026-10-27,13:5:9.4,+9:0"
        );
        assert_eq!(
            f(
                "1.3.6.1.2.1.4.20.1.1.10.0.0.1",
                Value::IpAddress([10, 0, 0, 1])
            )
            .value,
            "10.0.0.1"
        );
        assert_eq!(
            f("1.3.6.1.2.1.1.5.0", Value::OctetString(b"rtr1".to_vec())).value,
            "rtr1"
        );
        assert_eq!(
            f("1.3.6.1.2.1.1.5.0", Value::NoSuchInstance).value,
            "noSuchInstance"
        );
        assert_eq!(
            f("1.3.6.1.4.1.1.2.3", Value::Counter64(5)).name,
            "SNMPv2-SMI::enterprises.1.2.3"
        );
    }

    #[test]
    fn tables_and_indexes() {
        let r = Registry::bundled();
        let vbs = vec![
            VarBind::new(
                oid("1.3.6.1.2.1.2.2.1.2.1"),
                Value::OctetString(b"lo".to_vec()),
            ),
            VarBind::new(
                oid("1.3.6.1.2.1.2.2.1.2.2"),
                Value::OctetString(b"eth0".to_vec()),
            ),
            VarBind::new(oid("1.3.6.1.2.1.2.2.1.8.1"), Value::Integer(1)),
            VarBind::new(oid("1.3.6.1.2.1.2.2.1.8.2"), Value::Integer(2)),
            VarBind::new(oid("1.3.6.1.2.1.2.2.1.10.2"), Value::Counter32(77)),
            VarBind::new(oid("1.3.6.1.2.1.1.3.0"), Value::TimeTicks(1)),
        ];
        let t = build_tables(&r, &vbs);
        assert_eq!(t.len(), 1);
        let t = &t[0];
        assert_eq!(t.entry, "IF-MIB::ifEntry");
        assert_eq!(t.table.as_deref(), Some("IF-MIB::ifTable"));
        assert_eq!(t.index_names, ["ifIndex"]);
        assert_eq!(
            t.columns
                .iter()
                .map(|c| c.name.as_str())
                .collect::<Vec<_>>(),
            ["ifDescr", "ifOperStatus", "ifInOctets"]
        );
        assert!(t.columns[2].counter);
        assert_eq!(t.rows.len(), 2);
        assert_eq!(t.rows[1].index, ["2"]);
        assert_eq!(
            t.rows[1].cells,
            [
                Some("eth0".into()),
                Some("down(2)".into()),
                Some("77".into())
            ]
        );
        assert_eq!(t.rows[0].cells[2], None);
        assert_eq!(t.rows[1].numeric[2], Some(77));
        // IpAddress index; ifXEntry AUGMENTS ifEntry.
        let ip = r.node(&oid("1.3.6.1.2.1.4.20.1")).unwrap();
        assert_eq!(
            format::decode_index(&r, ip, &[10, 0, 0, 1]).unwrap(),
            ["10.0.0.1"]
        );
        let ifx = r.node(&oid("1.3.6.1.2.1.31.1.1.1")).unwrap();
        assert_eq!(format::decode_index(&r, ifx, &[5]).unwrap(), ["5"]);
        // ipAddressTable: { ipAddressAddrType, ipAddressAddr } (length-prefixed octets).
        let ipa = r.node(&oid("1.3.6.1.2.1.4.34.1")).unwrap();
        let idx = format::decode_index(&r, ipa, &[1, 4, 192, 0, 2, 1]).unwrap();
        assert_eq!(idx[0], "ipv4(1)");
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn vendor_mib_with_quirks() {
        let mut r = Registry::bundled();
        let res = r.load(
            include_str!("../../tests/data/WPG-EXAMPLE-VENDOR-MIB.my"),
            &Source::File("vendor.my".into()),
        );
        assert_eq!(res.modules, ["WPG-EXAMPLE-VENDOR-MIB"]);
        let info = r
            .modules()
            .into_iter()
            .find(|m| m.name == "WPG-EXAMPLE-VENDOR-MIB")
            .unwrap();
        assert_eq!(info.status, ModuleStatus::Partial, "{info:?}");
        assert_eq!(info.missing_imports, ["ACME-SMI-MISSING"]);
        // Usable despite the missing module and the broken definition.
        assert_eq!(
            r.resolve("wpgFanState.2").unwrap(),
            oid("1.3.6.1.4.1.99999.1.1.1.3.2")
        );
        let n = r.node_by_name("wpgFanState").unwrap();
        assert_eq!(n.syntax.as_ref().unwrap().enum_label(3), Some("failed"));
        let v = format_varbind(
            &r,
            &VarBind::new(oid("1.3.6.1.4.1.99999.1.1.1.4.2"), Value::Integer(4512)),
        );
        assert_eq!(v.value, "45.12 degC");
        let t = format_varbind(&r, &VarBind::new(oid("1.3.6.1.4.1.99999.0.1"), Value::Null));
        assert_eq!(t.name, "WPG-EXAMPLE-VENDOR-MIB::wpgFanFailure");
        assert!(r.node_by_name("wpgLegacyTrap").is_some());
        assert!(
            info.errors
                .iter()
                .any(|e| e.contains("wpgBroken") || e.contains("line")),
            "{:?}",
            info.errors
        );
        // A module that is only missing an import names it.
        let mut r2 = Registry::new();
        r2.load("X-MIB DEFINITIONS ::= BEGIN IMPORTS foo FROM Y-MIB; bar OBJECT IDENTIFIER ::= { foo 1 } END", &Source::File("x".into()));
        let m = &r2.modules()[0];
        assert_eq!(m.status, ModuleStatus::Error);
        assert_eq!(m.missing_imports, ["Y-MIB"]);
        // Removing and re-adding.
        assert!(r.remove("WPG-EXAMPLE-VENDOR-MIB"));
        r.rebuild();
        assert!(r.resolve("wpgFanState").is_err());
    }
}
