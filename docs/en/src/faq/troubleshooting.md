# FAQ and troubleshooting

When something goes wrong, run `wpg doctor` first. It diagnoses privileges, interfaces, the kernel, hugepages and Npcap / libpcap one line at a time, and prints OS-specific fix commands. `wpg doctor --json` returns the same for scripts, and the exit status is non-zero when a check fails.

## The `wpg doctor` checks

| Check id | State when unmet | Meaning and fix |
|---|---|---|
| `caps.raw_socket` | fail (Windows: per Npcap admin-only mode) | Raw-socket privilege: `CAP_NET_RAW` / root on Linux, BPF access on macOS, an elevated terminal on Windows only when Npcap is in administrator-only mode. See [Permissions](../getting-started/permissions.md) |
| `iface.any_up` | warn | A non-loopback interface is up with carrier. Check the cable / link |
| `kernel.af_xdp` | warn | Kernel 5.10+ for the AF_XDP backend |
| `kernel.tpacket_v3` | warn | Kernel 3.2+; without it afpacket falls back to `sendmmsg` |
| `hugepages` | warn | Hugepages configured; needed only for the DPDK backend |
| `npcap` | fail / warn (Windows only) | Npcap installed and `wpcap.dll` present (`System32\Npcap` or `System32`); skipped on other OSes |
| `pcap_lib` | warn | libpcap found (needed for the `pcap` backend); skipped on Windows |

Each result is `ok`, `warn`, `fail` or `skip` (not applicable on this OS).

## Common questions

### Transmission is refused with "permission denied"

Raw-socket transmission needs privilege. Run `wpg doctor` and apply the command it prints on the `caps.raw_socket` line (on Linux, something like `sudo setcap cap_net_raw,cap_net_admin+ep $(command -v wpg)`). A file capability is lost when the binary is replaced. See [Permissions](../getting-started/permissions.md).

### The rate falls short of the target

- When `average` is below the target and `dropped` is large, the backend cannot keep up. Try raising `--burst`, using `--qdisc-bypass` with `afpacket`, or adding `--workers` (one worker per core). See [Performance tuning](../operations/tuning.md).
- `--rate 50%` needs a real interface whose link speed is known. On `lo` it reports `Cannot determine the link speed`. Give an absolute rate such as `1Gbps`.
- `unlimited` on the `null` backend measures how fast frames are built, not a network figure.

### I want to check without transmitting

Dry-run with `--backend null` (build and count only) or `--backend pcapfile -i out.pcapng` (write to a file). `wpg render FILE -o out.pcapng` also writes a pcapng without transmitting.

### I want to catch template errors before sending

`wpg template lint FILE` reports unknown fields, out-of-range values, malformed generators and length / checksum mismatches. Deliberately broken templates (`templates/malformed/`) stay at the warning level (`diag-*`) and can still be sent.

### Is an imported template identical to the original capture?

The `wpg import` → `wpg render` round-trip matches byte for byte (timestamps differ, since wpg assigns its own). Confirm with `diff <(tshark -r a.pcapng -x) <(tshark -r b.pcapng -x)`.

### I want to verify in Wireshark

The tshark cross-check of the golden pcaps is `WPG_TSHARK=1 cargo test -p wpg-golden-tests -- --ignored`. For your own capture, `tshark -r out.pcapng -V` shows, for instance, that checksums read `[correct]`.

### User-facing text is still in English

The language is set by the settings screen in the GUI, by the `LANG` / `LC_MESSAGES` / `LC_ALL` environment variables (POSIX precedence) for the CLI, and by the `Accept-Language` header for the API. Untranslated strings fall back to English. To add keys, see [Adding i18n strings](../dev/add-i18n.md).

If your problem is not here, report it in an issue with the output of `wpg doctor --json`. Entries are added as issues come in.
