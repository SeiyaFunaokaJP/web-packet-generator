# Install on macOS

Status: draft

wpg supports macOS 12 or later and sends frames through BPF via the pcap backend. The current binaries are not signed or notarized, so the first launch needs a Gatekeeper workaround. Access to `/dev/bpf*` must be granted to the user running wpg. Signing and notarization are still under consideration.
