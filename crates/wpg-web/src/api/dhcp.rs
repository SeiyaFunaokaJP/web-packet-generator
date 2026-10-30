//! `/api/dhcp`: the DHCP server tool (settings, start / stop, leases,
//! activity).

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{enc, get, post, request, ApiError, Body, Method};

/// A fixed address.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Reservation {
    pub(crate) mac: String,
    pub(crate) ip: String,
}

/// DHCPv4 settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct V4 {
    pub(crate) enabled: bool,
    pub(crate) range_start: String,
    pub(crate) range_end: String,
    pub(crate) subnet_mask: String,
    pub(crate) routers: Vec<String>,
    pub(crate) dns: Vec<String>,
    pub(crate) domain: String,
    pub(crate) ntp: Vec<String>,
    pub(crate) lease_secs: u32,
    pub(crate) reservations: Vec<Reservation>,
    pub(crate) server_ip: Option<String>,
}

/// DHCPv6 and router advertisement settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct V6 {
    pub(crate) enabled: bool,
    pub(crate) na_enabled: bool,
    pub(crate) range_start: String,
    pub(crate) range_end: String,
    pub(crate) pd_enabled: bool,
    pub(crate) pd_prefix: String,
    pub(crate) pd_prefix_len: u8,
    pub(crate) pd_delegated_len: u8,
    pub(crate) dns: Vec<String>,
    pub(crate) domains: Vec<String>,
    pub(crate) preferred_secs: u32,
    pub(crate) valid_secs: u32,
    pub(crate) rapid_commit: bool,
    pub(crate) preference: u8,
    pub(crate) ra_enabled: bool,
    pub(crate) ra_interval_secs: u32,
    pub(crate) ra_router_lifetime_secs: u16,
    pub(crate) ra_prefix: String,
    pub(crate) ra_autonomous: bool,
}

/// Everything of the settings form.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Settings {
    pub(crate) addresses: Vec<String>,
    pub(crate) v4: V4,
    pub(crate) v6: V6,
}

/// Packet counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Counters {
    pub(crate) rx4: u64,
    pub(crate) tx4: u64,
    pub(crate) rx6: u64,
    pub(crate) tx6: u64,
    pub(crate) ra: u64,
}

/// `GET /api/dhcp`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct DhcpStatus {
    pub(crate) enabled: bool,
    pub(crate) supported: bool,
    pub(crate) pinned_iface: Option<String>,
    pub(crate) iface: String,
    pub(crate) running: bool,
    pub(crate) started_ms: Option<u64>,
    pub(crate) server_ip: Option<String>,
    pub(crate) server_duid: Option<String>,
    pub(crate) added_addresses: Vec<String>,
    pub(crate) settings: Settings,
    pub(crate) stats: Counters,
    pub(crate) leases: usize,
    pub(crate) error: Option<String>,
}

/// A lease.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Lease {
    /// `v4`, `na`, `pd`.
    pub(crate) kind: String,
    pub(crate) addr: String,
    pub(crate) plen: u8,
    pub(crate) client: String,
    pub(crate) iaid: u32,
    pub(crate) mac: Option<String>,
    pub(crate) hostname: Option<String>,
    /// `offered`, `bound`, `declined`.
    pub(crate) state: String,
    pub(crate) expires: u64,
    pub(crate) updated: u64,
}

impl Lease {
    /// The key the server deletes by.
    pub(crate) fn key(&self) -> String {
        if self.kind == "pd" {
            format!("{}/{}", self.addr, self.plen)
        } else {
            self.addr.clone()
        }
    }
}

/// One exchange.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub(crate) struct Activity {
    pub(crate) ts_ms: u64,
    /// `v4`, `v6`, `ra`.
    pub(crate) family: String,
    pub(crate) request: String,
    pub(crate) client: String,
    pub(crate) reply: Option<String>,
    pub(crate) addrs: Vec<String>,
    pub(crate) note: Option<String>,
}

pub(crate) async fn status() -> Result<DhcpStatus, ApiError> {
    get("/api/dhcp").await
}

pub(crate) async fn update(
    iface: Option<String>,
    settings: &Settings,
) -> Result<DhcpStatus, ApiError> {
    let body = json!({ "iface": iface, "settings": settings });
    let resp = request(Method::Put, "/api/dhcp", super::body_of(&body)?).await?;
    super::json(resp).await
}

pub(crate) async fn start() -> Result<DhcpStatus, ApiError> {
    post("/api/dhcp/start", &serde_json::Value::Null).await
}

pub(crate) async fn stop() -> Result<DhcpStatus, ApiError> {
    post("/api/dhcp/stop", &serde_json::Value::Null).await
}

pub(crate) async fn leases() -> Result<Vec<Lease>, ApiError> {
    get("/api/dhcp/leases").await
}

/// Deletes one lease (`Some(key)`) or all of them.
pub(crate) async fn delete_leases(key: Option<&str>) -> Result<(), ApiError> {
    let path = match key {
        Some(k) => format!("/api/dhcp/leases?key={}", enc(k)),
        None => "/api/dhcp/leases".to_owned(),
    };
    request(Method::Delete, &path, Body::Empty)
        .await
        .map(|_| ())
}

pub(crate) async fn activity() -> Result<Vec<Activity>, ApiError> {
    get("/api/dhcp/activity?limit=200").await
}

pub(crate) async fn clear_activity() -> Result<(), ApiError> {
    request(Method::Delete, "/api/dhcp/activity", Body::Empty)
        .await
        .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_status_and_round_trips_settings() {
        let s: DhcpStatus = serde_json::from_str(
            r#"{"enabled":true,"supported":true,"iface":"eth1","running":true,
                "added_addresses":["192.168.100.1/24"],
                "settings":{"addresses":[],"v4":{"enabled":true,"range_start":"192.168.100.100",
                "range_end":"192.168.100.199","subnet_mask":"255.255.255.0","routers":[],"dns":[],
                "domain":"","ntp":[],"lease_secs":3600,"reservations":[],"server_ip":null},
                "v6":{"enabled":false,"pd_prefix_len":40}},
                "stats":{"rx4":3,"tx4":2,"rx6":0,"tx6":0,"ra":0},"leases":1}"#,
        )
        .unwrap();
        assert!(s.running && s.pinned_iface.is_none());
        assert_eq!(s.settings.v4.lease_secs, 3600);
        assert_eq!(s.settings.v6.pd_prefix_len, 40);
        assert_eq!(s.stats.tx4, 2);
        let back: Settings =
            serde_json::from_value(serde_json::to_value(&s.settings).unwrap()).unwrap();
        assert_eq!(back, s.settings);
        let l = Lease {
            kind: "pd".into(),
            addr: "2001:db8::".into(),
            plen: 56,
            ..Lease::default()
        };
        assert_eq!(l.key(), "2001:db8::/56");
    }
}
