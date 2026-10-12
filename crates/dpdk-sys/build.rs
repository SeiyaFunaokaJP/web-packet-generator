//! Build script for `dpdk-sys` (ADR-0004).
//!
//! Without the `link` feature it does nothing, so the crate builds anywhere. With it:
//!
//! 1. finds DPDK with pkg-config (`libdpdk`, at least 23.11) and links it,
//! 2. checks the version against the supported LTS releases (24.11 primary, 23.11),
//! 3. compiles `csrc/shim.c` (out-of-line wrappers for the inline fast path) with
//!    DPDK's own cflags,
//! 4. runs bindgen on `csrc/shim.h` and writes `$OUT_DIR/bindings.rs`.
//!
//! Use `PKG_CONFIG_PATH` to point at a DPDK built from source (for example
//! `/usr/local/lib/x86_64-linux-gnu/pkgconfig`).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=csrc/shim.h");
    println!("cargo:rerun-if-changed=csrc/shim.c");
    #[cfg(feature = "link")]
    link::run();
}

#[cfg(feature = "link")]
mod link {
    use std::env;
    use std::path::PathBuf;
    use std::process::Command;

    /// Minimum DPDK accepted by pkg-config.
    const MIN_VERSION: &str = "23.11";
    /// LTS releases the bindings and the shim are checked against (year, month).
    const SUPPORTED: &[(u32, u32)] = &[(24, 11), (23, 11)];

    pub(crate) fn run() {
        println!("cargo:rerun-if-env-changed=PKG_CONFIG_PATH");
        println!("cargo:rerun-if-env-changed=PKG_CONFIG");

        let lib = match pkg_config::Config::new()
            .atleast_version(MIN_VERSION)
            .probe("libdpdk")
        {
            Ok(lib) => lib,
            Err(e) => panic!(
                "\n\ndpdk-sys: DPDK {MIN_VERSION}+ was not found with pkg-config (`libdpdk`).\n\
                 Install the development package (Debian / Ubuntu: `apt install dpdk-dev \
                 libclang-dev`) or set PKG_CONFIG_PATH to a DPDK built from source.\n\
                 See docs/en/src/operations/dpdk-afxdp.md.\n\npkg-config said: {e}\n"
            ),
        };

        let version = parse_version(&lib.version);
        match version {
            Some(v) if SUPPORTED.contains(&v) => {}
            _ => println!(
                "cargo:warning=dpdk-sys: DPDK {} is not one of the tested LTS releases \
                 (24.11, 23.11); the bindings may not match",
                lib.version
            ),
        }
        // Visible to dependents' build scripts as DEP_DPDK_VERSION.
        println!("cargo:version={}", lib.version);

        let cflags = dpdk_cflags();

        let mut cc = cc::Build::new();
        cc.file("csrc/shim.c")
            .include("csrc")
            .warnings(true)
            .opt_level(3);
        for flag in &cflags {
            cc.flag(flag);
        }
        cc.compile("wpg_dpdk_shim");

        let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
        bindgen::Builder::default()
            .header("csrc/shim.h")
            .clang_args(&cflags)
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            // The control path (EAL, ethdev, mempool) and the wrappers for the inline
            // fast path; everything they reference is pulled in recursively.
            .allowlist_function("wpg_.*")
            .allowlist_function("rte_eal_.*")
            .allowlist_function("rte_eth_.*")
            .allowlist_function("rte_pktmbuf_.*")
            .allowlist_function("rte_mempool_.*")
            .allowlist_function("rte_lcore_.*")
            .allowlist_function("rte_get_.*_lcore")
            .allowlist_function("rte_socket_.*")
            .allowlist_function("rte_strerror")
            .allowlist_function("rte_version")
            .allowlist_function("rte_exit")
            .allowlist_var("RTE_.*")
            .allowlist_type("rte_eth_.*")
            // Static inline functions have no symbol; the wpg_ wrappers stand in.
            .generate_inline_functions(false)
            .derive_default(true)
            .prepend_enum_name(false)
            .layout_tests(true)
            // The workspace denies `unsafe_op_in_unsafe_fn`.
            .wrap_unsafe_ops(true)
            .generate()
            .expect("dpdk-sys: bindgen failed (is libclang installed? apt install libclang-dev)")
            .write_to_file(out.join("bindings.rs"))
            .expect("dpdk-sys: writing bindings.rs");
    }

    /// `"24.11.2"` → `(24, 11)`.
    fn parse_version(v: &str) -> Option<(u32, u32)> {
        let mut it = v.split('.');
        Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
    }

    /// The full `pkg-config --cflags libdpdk`: besides the include paths (which the
    /// pkg-config crate exposes) it force-includes `rte_config.h` and sets `-march`,
    /// both of which the shim and bindgen need to see the same headers DPDK was built
    /// with.
    fn dpdk_cflags() -> Vec<String> {
        let pkg_config = env::var("PKG_CONFIG").unwrap_or_else(|_| "pkg-config".to_owned());
        let output = Command::new(&pkg_config)
            .args(["--cflags", "libdpdk"])
            .output()
            .unwrap_or_else(|e| panic!("dpdk-sys: running {pkg_config}: {e}"));
        assert!(
            output.status.success(),
            "dpdk-sys: `{pkg_config} --cflags libdpdk` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).expect("pkg-config output is UTF-8");
        // `-include rte_config.h` is two words; keep them as two arguments.
        text.split_whitespace().map(str::to_owned).collect()
    }
}
