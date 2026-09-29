# Adding a protocol

Status: draft

Core protocols are defined with a Rust macro and user extensions with YAML files in `protocols.d/`; both compile to the same IR. From the IR the project generates building, parsing, the GUI editor and the reference docs. A new protocol needs a definition, a sample template, a golden pcap and, where possible, a tshark cross-check. This keeps the cost of each added protocol constant.
