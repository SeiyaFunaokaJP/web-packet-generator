//! End-to-end check of the bindings and the C shim against the installed DPDK: EAL
//! without hugepages or PCI, a `net_ring` virtual port (its TX ring is its RX ring),
//! one frame sent with `wpg_rte_eth_tx_burst` and read back with
//! `wpg_rte_eth_rx_burst`.
//!
//! Needs only the DPDK runtime (the `net_ring` PMD and the ring mempool driver are
//! loaded from DPDK's plugin directory). Run with
//! `cargo test -p dpdk-sys --features link`.
#![cfg(feature = "link")]

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use dpdk_sys as sys;

fn errno_text() -> String {
    // SAFETY: rte_errno / rte_strerror are valid on any thread once EAL is loaded;
    // rte_strerror returns a static (thread-local) string.
    unsafe { CStr::from_ptr(sys::rte_strerror(sys::wpg_rte_errno())) }
        .to_string_lossy()
        .into_owned()
}

#[test]
fn eal_ring_port_round_trip() {
    // SAFETY: rte_version returns a static NUL-terminated string.
    let version = unsafe { CStr::from_ptr(sys::rte_version()) }.to_string_lossy();
    assert!(version.starts_with("DPDK "), "{version}");
    assert!(
        [(24, 11), (23, 11)].contains(&(sys::RTE_VER_YEAR, sys::RTE_VER_MONTH)),
        "untested DPDK {version}"
    );

    // EAL may be initialised only once per process, so everything is in one test.
    let args: Vec<CString> = [
        "wpg-dpdk-sys-test",
        "--no-huge",
        "-m",
        "256",
        "--no-pci",
        "--no-shconf",
        "--no-telemetry",
        "--log-level=lib.eal:error",
        "--vdev=net_ring0",
    ]
    .iter()
    .map(|s| CString::new(*s).unwrap())
    .collect();
    let mut argv: Vec<*mut c_char> = args.iter().map(|a| a.as_ptr().cast_mut()).collect();
    let argc = i32::try_from(argv.len()).unwrap();
    // SAFETY: argv holds argc valid C strings that outlive the call (EAL keeps
    // pointers into them, and `args` lives until the end of the test).
    let ret = unsafe { sys::rte_eal_init(argc, argv.as_mut_ptr()) };
    assert!(ret >= 0, "rte_eal_init: {}", errno_text());

    // SAFETY: plain FFI calls after a successful rte_eal_init; every pointer passed
    // is either null where DPDK allows it or points to live locals / DPDK objects.
    unsafe {
        assert_eq!(sys::rte_eth_dev_count_avail(), 1);
        let port: u16 = 0;
        let socket = sys::rte_eth_dev_socket_id(port);

        let name = CString::new("wpg_test_pool").unwrap();
        let pool = sys::rte_pktmbuf_pool_create(
            name.as_ptr(),
            511,
            0,
            0,
            u16::try_from(sys::RTE_MBUF_DEFAULT_BUF_SIZE).unwrap(),
            socket,
        );
        assert!(!pool.is_null(), "rte_pktmbuf_pool_create: {}", errno_text());

        let conf = sys::rte_eth_conf::default();
        assert_eq!(sys::rte_eth_dev_configure(port, 1, 1, &conf), 0);
        let socket = u32::try_from(socket.max(0)).unwrap();
        assert_eq!(
            sys::rte_eth_rx_queue_setup(port, 0, 128, socket, ptr::null(), pool),
            0
        );
        assert_eq!(
            sys::rte_eth_tx_queue_setup(port, 0, 128, socket, ptr::null()),
            0
        );
        assert_eq!(sys::rte_eth_dev_start(port), 0);

        let frame: Vec<u8> = (0..64u8).collect();
        let m = sys::wpg_rte_pktmbuf_alloc(pool);
        assert!(!m.is_null());
        assert_eq!(
            sys::wpg_rte_pktmbuf_copy_in(m, frame.as_ptr().cast(), 64),
            0
        );
        assert_eq!(sys::wpg_rte_pktmbuf_data_len(m), 64);
        assert_eq!(sys::wpg_rte_pktmbuf_pkt_len(m), 64);
        let mut tx = [m];
        assert_eq!(sys::wpg_rte_eth_tx_burst(port, 0, tx.as_mut_ptr(), 1), 1);

        let mut rx = [ptr::null_mut::<sys::rte_mbuf>(); 4];
        assert_eq!(sys::wpg_rte_eth_rx_burst(port, 0, rx.as_mut_ptr(), 4), 1);
        let got = rx[0];
        let len = usize::from(sys::wpg_rte_pktmbuf_data_len(got));
        let data = std::slice::from_raw_parts(sys::wpg_rte_pktmbuf_mtod(got).cast::<u8>(), len);
        assert_eq!(data, &frame[..]);
        sys::wpg_rte_pktmbuf_free(got);

        // A frame larger than the data room does not fit.
        let m = sys::wpg_rte_pktmbuf_alloc(pool);
        let big = vec![0u8; usize::from(sys::wpg_rte_pktmbuf_tailroom(m)) + 1];
        let big_len = u16::try_from(big.len()).unwrap();
        assert_eq!(
            sys::wpg_rte_pktmbuf_copy_in(m, big.as_ptr().cast(), big_len),
            -1
        );
        sys::wpg_rte_pktmbuf_free(m);

        let mut stats = sys::rte_eth_stats::default();
        assert_eq!(sys::rte_eth_stats_get(port, &mut stats), 0);
        // net_ring counts packets only (no byte counters).
        assert_eq!(stats.opackets, 1);
        assert_eq!(stats.ipackets, 1);

        assert_eq!(sys::rte_eth_dev_stop(port), 0);
        assert_eq!(sys::rte_eth_dev_close(port), 0);
        sys::rte_mempool_free(pool);
        assert_eq!(sys::rte_eal_cleanup(), 0);
    }
}
