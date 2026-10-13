//! Windows + `pcap` feature: link `wpcap.dll` with `/DELAYLOAD`, so `wpg.exe` starts (and
//! `wpg doctor` can explain what is missing) on machines without Npcap. The pcap backends
//! load the DLL through `wpg_sys::npcap::load` before the first libpcap call.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let windows_msvc = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if windows_msvc && std::env::var_os("CARGO_FEATURE_PCAP").is_some() {
        println!("cargo:rustc-link-arg=/DELAYLOAD:wpcap.dll");
        println!("cargo:rustc-link-arg=delayimp.lib");
    }
}
