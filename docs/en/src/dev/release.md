# Release process

Status: draft

Releases are built by cargo-dist in a GitHub Actions matrix that produces Windows, macOS and Linux binaries with checksums. `scripts/release.sh` is the local entry point. Before a v1.0 release, golden pcap and tshark checks, `cargo audit`, `cargo deny` and the i18n lint must all be green. The CHANGELOG and both language docs must be complete.
