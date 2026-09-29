# Golden packets

Each case has two files, both checked in:

- `<name>.hex` - the expected bytes as one hex string (`#` comment lines allowed).
- `<name>.pcap` - the same packet as a classic pcap (Ethernet), for Wireshark and the
  `tshark` cross-check.

The cases are defined in `tests/golden.rs`; `tests/tshark.rs` holds the optional
Wireshark expectations.

## Update workflow

```sh
just golden-update            # = WPG_UPDATE_GOLDEN=1 cargo test -p wpg-golden-tests --test golden
git diff tests/golden         # review every byte change
cargo test -p wpg-golden-tests
just tshark                   # optional: needs tshark, checks checksums/decoded fields
```

## Adding a case (e.g. a new protocol)

1. Add `fn case_<name>() -> Packet` to `tests/golden.rs`.
2. Add one line `("<name>", case_<name>())` to `cases()`.
3. Run `just golden-update`, review, commit the new `.hex` and `.pcap`.
4. Optionally add `(<name>, [(tshark field, expected)])` to `expectations()` in
   `tests/tshark.rs`. Checksum status and `_ws.malformed` are checked for every pcap.

Never edit `.hex` by hand to make a test pass: verify the bytes against an independent
source (RFC, Wireshark) first.
