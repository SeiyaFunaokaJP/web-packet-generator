//! Raw FFI bindings to [DPDK](https://www.dpdk.org/) for wpg's `dpdk` transmit backend
//! (ADR-0004).
//!
//! The crate is empty unless the `link` feature is on; then `build.rs` finds DPDK with
//! pkg-config (24.11 LTS primary, 23.11 LTS also checked), links it and generates the
//! bindings with bindgen. Covered: EAL, ethdev, mempool / mbuf and lcore control-path
//! functions, their types, the `RTE_*` constants, non-EAL thread registration
//! (`rte_thread_register`) and device hotplug (`rte_dev_probe`).
//!
//! DPDK's fast path (`rte_eth_tx_burst`, `rte_pktmbuf_alloc`, ...) is `static inline`
//! in its headers and has no symbol to link against, and `rte_errno` is a macro.
//! `csrc/shim.c` wraps them in out-of-line functions with a `wpg_` prefix
//! ([`wpg_rte_eth_tx_burst`], [`wpg_rte_pktmbuf_alloc`], [`wpg_rte_errno`], ...),
//! compiled with DPDK's own cflags.
//!
//! Everything here is `unsafe` and mirrors the C API one to one; the safe layer lives
//! in `wpg-tx`.

#![cfg_attr(
    feature = "link",
    allow(
        non_camel_case_types,
        non_snake_case,
        non_upper_case_globals,
        unreachable_pub,
        unused_qualifications,
        unnecessary_transmutes,
        missing_debug_implementations,
        clippy::all,
        clippy::pedantic,
        rustdoc::all
    )
)]

#[cfg(feature = "link")]
include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
