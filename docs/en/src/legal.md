# Legal notice

## Responsible use

wpg is a tool for verifying and testing networks. Use it only against **networks you own, or that you have explicit authorization to test.**

The tool sends arbitrary frames statelessly and at high speed, and can generate traffic that disrupts a live network — ARP / ND sweeps, SYN / MAC floods, malformed frames. Directing such traffic at networks or devices you do not control, without authorization, is illegal in many jurisdictions. You are solely responsible for how you use it.

- Always dry-run a new template with a non-transmitting backend (`--backend null`, or `pcapfile` with `-i out.pcapng`) before sending it.
- Test in a lab isolated from production networks (a virtual environment such as CML2 is useful; see [CML2 notes](operations/cml2.md)).
- The administrator settings `[tx] max_rate` and `allowed_ifaces` can cap the usable rate and interfaces ([Configuration](reference/config.md)).

Changes intended to turn wpg into an attack tool are not accepted (see "Responsible use" in CONTRIBUTING.md).

## License

This software is dual-licensed under [MIT](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/LICENSE-MIT) or [Apache-2.0](https://github.com/SeiyaFunaokaJP/web-packet-generator/blob/main/LICENSE-APACHE), at your option. Contributions are, unless stated otherwise, taken to be offered under the same dual license (Developer Certificate of Origin; sign off with `git commit -s`).

## Third-party components

Dependency crates keep their own licenses. The set and the license policy are checked with `cargo deny check` (`deny.toml`).

**Npcap**, used for transmission on Windows, is not bundled with this software; you obtain it from [npcap.com](https://npcap.com/) and install it under its own terms. Redistribution may require a separate license.

## Disclaimer

The software is provided "as is", without warranty of any kind, express or implied, as set out in the MIT / Apache-2.0 licenses.
