//! Minimal Telnet client protocol (RFC 854/855) for the web terminal.
//!
//! The client is *passive*: it never starts a negotiation, it only answers the
//! server, so a raw TCP console (a terminal server port, an emulator's serial
//! console) never sees stray IAC bytes. It agrees to
//!
//! * `ECHO` and `SGA` from the server (remote echo, character at a time);
//! * `NAWS` (RFC 1073), sending the window size now and after every resize;
//! * `TTYPE` (RFC 1091), answering `XTERM-256COLOR`;
//! * `BINARY` (RFC 856) in both directions;
//!
//! and refuses everything else once. Incoming `IAC IAC` becomes `0xff` and the
//! NVT `CR NUL` becomes `CR`; outgoing `0xff` is doubled and a bare `CR` is sent as
//! `CR NUL` unless binary mode was agreed.

/// Interpret As Command.
pub const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

/// Option codes.
pub mod opt {
    /// Binary transmission (RFC 856).
    pub const BINARY: u8 = 0;
    /// Echo (RFC 857).
    pub const ECHO: u8 = 1;
    /// Suppress go-ahead (RFC 858).
    pub const SGA: u8 = 3;
    /// Terminal type (RFC 1091).
    pub const TTYPE: u8 = 24;
    /// Negotiate about window size (RFC 1073).
    pub const NAWS: u8 = 31;
}

const TTYPE_IS: u8 = 0;
const TTYPE_SEND: u8 = 1;

/// Longest sub-negotiation kept (longer ones are truncated).
const SB_MAX: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Data,
    /// After a data `CR` (a following `NUL` is dropped).
    Cr,
    Iac,
    /// After `IAC WILL|WONT|DO|DONT`.
    Verb(u8),
    Sb,
    SbIac,
}

/// Telnet protocol state of one connection.
#[derive(Debug, Clone)]
pub struct Telnet {
    state: State,
    sb: Vec<u8>,
    /// Options we perform (`WILL` sent and in force).
    local: [bool; 256],
    /// Options the server performs (`DO` sent and in force).
    remote: [bool; 256],
    /// Options already refused once (no refusal loops).
    refused: [bool; 256],
    term: String,
    cols: u16,
    rows: u16,
    /// Replies waiting to be written to the server.
    pending: Vec<u8>,
}

impl Telnet {
    /// A connection announcing terminal type `term` with a `cols` × `rows` window.
    #[must_use]
    pub fn new(term: &str, cols: u16, rows: u16) -> Self {
        Self {
            state: State::Data,
            sb: Vec::new(),
            local: [false; 256],
            remote: [false; 256],
            refused: [false; 256],
            term: term.to_ascii_uppercase(),
            cols,
            rows,
            pending: Vec::new(),
        }
    }

    /// Whether the server echoes what we send (`WILL ECHO` accepted).
    #[must_use]
    pub fn remote_echo(&self) -> bool {
        self.remote[usize::from(opt::ECHO)]
    }

    /// Whether we agreed to send the window size.
    #[must_use]
    pub fn naws(&self) -> bool {
        self.local[usize::from(opt::NAWS)]
    }

    /// Bytes to send to the server (negotiation replies), draining them.
    pub fn take_pending(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.pending)
    }

    /// Processes bytes from the server; returns the terminal data among them.
    /// Replies accumulate in [`Telnet::take_pending`].
    pub fn receive(&mut self, input: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(input.len());
        for &b in input {
            self.state = match self.state {
                State::Data | State::Cr => {
                    let after_cr = self.state == State::Cr;
                    match b {
                        IAC => State::Iac,
                        0 if after_cr => State::Data,
                        b'\r' => {
                            out.push(b);
                            State::Cr
                        }
                        _ => {
                            out.push(b);
                            State::Data
                        }
                    }
                }
                State::Iac => match b {
                    IAC => {
                        out.push(IAC);
                        State::Data
                    }
                    WILL | WONT | DO | DONT => State::Verb(b),
                    SB => {
                        self.sb.clear();
                        State::Sb
                    }
                    // NOP, GA, DM, BRK, AYT, ...: nothing to do.
                    _ => State::Data,
                },
                State::Verb(verb) => {
                    self.negotiate(verb, b);
                    State::Data
                }
                State::Sb => {
                    if b == IAC {
                        State::SbIac
                    } else {
                        if self.sb.len() < SB_MAX {
                            self.sb.push(b);
                        }
                        State::Sb
                    }
                }
                State::SbIac => match b {
                    SE => {
                        self.subnegotiation();
                        State::Data
                    }
                    IAC => {
                        if self.sb.len() < SB_MAX {
                            self.sb.push(IAC);
                        }
                        State::Sb
                    }
                    // Malformed: give up on this sub-negotiation.
                    _ => State::Data,
                },
            };
        }
        out
    }

    fn send_cmd(&mut self, verb: u8, option: u8) {
        self.pending.extend_from_slice(&[IAC, verb, option]);
    }

    fn negotiate(&mut self, verb: u8, option: u8) {
        let i = usize::from(option);
        match verb {
            DO => {
                let supported = matches!(option, opt::NAWS | opt::TTYPE | opt::SGA | opt::BINARY);
                if supported {
                    if !self.local[i] {
                        self.local[i] = true;
                        self.send_cmd(WILL, option);
                    }
                    if option == opt::NAWS {
                        self.send_naws();
                    }
                } else if !self.refused[i] {
                    self.refused[i] = true;
                    self.send_cmd(WONT, option);
                }
            }
            DONT if self.local[i] => {
                self.local[i] = false;
                self.send_cmd(WONT, option);
            }
            WILL => {
                let supported = matches!(option, opt::ECHO | opt::SGA | opt::BINARY);
                if supported {
                    if !self.remote[i] {
                        self.remote[i] = true;
                        self.send_cmd(DO, option);
                    }
                } else if !self.refused[i] {
                    self.refused[i] = true;
                    self.send_cmd(DONT, option);
                }
            }
            WONT if self.remote[i] => {
                self.remote[i] = false;
                self.send_cmd(DONT, option);
            }
            _ => {}
        }
    }

    fn subnegotiation(&mut self) {
        if self.sb.first() == Some(&opt::TTYPE)
            && self.sb.get(1) == Some(&TTYPE_SEND)
            && self.local[usize::from(opt::TTYPE)]
        {
            self.pending
                .extend_from_slice(&[IAC, SB, opt::TTYPE, TTYPE_IS]);
            let term = self.term.clone();
            self.pending.extend_from_slice(term.as_bytes());
            self.pending.extend_from_slice(&[IAC, SE]);
        }
    }

    fn send_naws(&mut self) {
        self.pending.extend_from_slice(&[IAC, SB, opt::NAWS]);
        for v in [self.cols, self.rows] {
            for b in v.to_be_bytes() {
                self.pending.push(b);
                if b == IAC {
                    self.pending.push(IAC);
                }
            }
        }
        self.pending.extend_from_slice(&[IAC, SE]);
    }

    /// Records a new window size; queues a NAWS update when agreed.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        if self.naws() {
            self.send_naws();
        }
    }

    /// Encodes user input for the wire (IAC doubling, NVT `CR`).
    #[must_use]
    pub fn encode(&self, input: &[u8]) -> Vec<u8> {
        let binary = self.local[usize::from(opt::BINARY)];
        let mut out = Vec::with_capacity(input.len() + 2);
        for (i, &b) in input.iter().enumerate() {
            match b {
                IAC => out.extend_from_slice(&[IAC, IAC]),
                b'\r' if !binary && input.get(i + 1) != Some(&b'\n') => {
                    out.extend_from_slice(b"\r\0");
                }
                _ => out.push(b),
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_data_passes_through() {
        let mut t = Telnet::new("xterm-256color", 80, 24);
        assert_eq!(t.receive(b"login: "), b"login: ");
        assert!(t.take_pending().is_empty(), "passive: nothing sent");
        // CR NUL -> CR, CR LF kept, IAC IAC -> 0xff
        assert_eq!(t.receive(b"a\r\0b\r\nc\xff\xffd"), b"a\rb\r\nc\xffd");
        // split across reads
        assert_eq!(t.receive(b"x\r"), b"x\r");
        assert_eq!(t.receive(b"\0y"), b"y");
        assert_eq!(t.receive(&[IAC]), b"");
        assert_eq!(t.receive(&[IAC, b'z']), b"\xffz");
    }

    #[test]
    fn typical_router_negotiation() {
        // Cisco IOS: WILL ECHO, WILL SGA, DO TTYPE, DO NAWS
        let mut t = Telnet::new("xterm-256color", 120, 40);
        let out = t.receive(&[
            IAC,
            WILL,
            opt::ECHO,
            IAC,
            WILL,
            opt::SGA,
            IAC,
            DO,
            opt::TTYPE,
            IAC,
            DO,
            opt::NAWS,
            b'>',
        ]);
        assert_eq!(out, b">");
        assert!(t.remote_echo() && t.naws());
        assert_eq!(
            t.take_pending(),
            [
                IAC,
                DO,
                opt::ECHO,
                IAC,
                DO,
                opt::SGA,
                IAC,
                WILL,
                opt::TTYPE,
                IAC,
                WILL,
                opt::NAWS,
                IAC,
                SB,
                opt::NAWS,
                0,
                120,
                0,
                40,
                IAC,
                SE
            ]
        );
        // repeated requests do not loop
        t.receive(&[IAC, WILL, opt::ECHO, IAC, DO, opt::TTYPE]);
        assert!(t.take_pending().is_empty());
        // TTYPE SEND
        t.receive(&[IAC, SB, opt::TTYPE, TTYPE_SEND, IAC, SE]);
        let mut want = vec![IAC, SB, opt::TTYPE, TTYPE_IS];
        want.extend_from_slice(b"XTERM-256COLOR");
        want.extend_from_slice(&[IAC, SE]);
        assert_eq!(t.take_pending(), want);
        // resize sends NAWS, with 255 doubled
        t.resize(255, 50);
        assert_eq!(
            t.take_pending(),
            [IAC, SB, opt::NAWS, 0, 255, 255, 0, 50, IAC, SE]
        );
    }

    #[test]
    fn refuses_unknown_options_once() {
        let mut t = Telnet::new("xterm", 80, 24);
        t.receive(&[IAC, DO, 39, IAC, WILL, 5]); // NEW-ENVIRON, STATUS
        assert_eq!(t.take_pending(), [IAC, WONT, 39, IAC, DONT, 5]);
        t.receive(&[IAC, DO, 39, IAC, WILL, 5]);
        assert!(t.take_pending().is_empty());
        // DONT / WONT on agreed options are acknowledged
        t.receive(&[IAC, DO, opt::SGA, IAC, WILL, opt::ECHO]);
        t.take_pending();
        t.receive(&[IAC, DONT, opt::SGA, IAC, WONT, opt::ECHO]);
        assert_eq!(
            t.take_pending(),
            [IAC, WONT, opt::SGA, IAC, DONT, opt::ECHO]
        );
        assert!(!t.remote_echo());
        // resize without NAWS sends nothing
        t.resize(100, 30);
        assert!(t.take_pending().is_empty());
        // TTYPE SEND without having agreed to TTYPE is ignored
        t.receive(&[IAC, SB, opt::TTYPE, TTYPE_SEND, IAC, SE]);
        assert!(t.take_pending().is_empty());
        // other commands (NOP, GA) vanish
        assert_eq!(t.receive(&[b'a', IAC, 241, IAC, 249, b'b']), b"ab");
    }

    #[test]
    fn encodes_input() {
        let mut t = Telnet::new("xterm", 80, 24);
        assert_eq!(t.encode(b"show ver\r"), b"show ver\r\0");
        assert_eq!(t.encode(b"a\r\nb"), b"a\r\nb");
        assert_eq!(t.encode(&[1, IAC, 2]), [1, IAC, IAC, 2]);
        t.receive(&[IAC, DO, opt::BINARY]);
        assert_eq!(t.encode(b"x\r"), b"x\r");
    }
}
