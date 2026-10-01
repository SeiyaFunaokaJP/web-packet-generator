//! The [`protocol!`](crate::protocol) definition macro.
//!
//! It is a thin, declarative front-end producing exactly the same
//! [`ProtocolDef`](crate::ProtocolDef) IR as the YAML loader.

/// Defines a [`ProtocolDef`](crate::ProtocolDef) concisely.
///
/// ```
/// use wpg_core::protocol;
///
/// let def = protocol! {
///     id: "demo",
///     name: "Demo",
///     layer: L3,
///     fields: {
///         version: uint(4) = 4;
///         hlen: uint(4), auto = length(header, unit = words32);
///         flags: flags(8) { 1 => df, 0 => mf } = 0b10;
///         proto: enum(8) { 17 => udp } = 0xfd, auto = next_protocol;
///         total: uint(16), auto = length(total);
///         csum: uint(16), auto = checksum(internet, header);
///         src: ipv4 = [192, 0, 2, 1];
///         dst: ipv4 = [192, 0, 2, 2];
///         opts: bytes(from = hlen, unit = 4, adjust = -16);
///         pad: padding(4);
///     },
///     next: proto { 17 => "udp" },
///     pseudo_header: (src, dst),
///     constraints: {
///         error: in_range(hlen, 4, 15);
///         warning: equals(version, 4);
///     },
/// };
/// def.validate().unwrap();
/// assert_eq!(def.fields.len(), 10);
/// ```
///
/// Field syntax: `name: type[(args)] [{ value => name, ... }] [= default]
/// [, auto = rule] [, desc = "fluent-key"];`
///
/// * types: `uint(N)`, `int(N)`, `enum(N) { v => name, .. }`,
///   `flags(N) { bit => name, .. }`, `mac`, `ipv4`, `ipv6`, `nsap`, `raw`,
///   `bytes(N)`, `bytes(to_end)`, `bytes(from = field[, unit = U][, adjust = A])`,
///   `padding(ALIGN)`, and `ty(expr)` for any [`FieldType`](crate::FieldType)
///   expression (TLV / list fields, whose defaults are `FieldValue::from(..)`)
/// * defaults are typed by the field: integers for `uint`/`enum`/`flags`/`int`,
///   `[u8; 6]` for `mac`, anything `Ipv4Addr::from` / `Ipv6Addr::from` accepts for
///   addresses, anything `Vec::<u8>::from` accepts for byte types
/// * binding entries are single values; ranges / defaults / re-parse entries are
///   added to `def.next` afterwards ([`BindingEntry::range`](crate::BindingEntry::range))
/// * auto rules: `next_protocol`, `counter`,
///   `length(header|payload|total[, unit = bytes|words16|words32|words64][, adjust = A])`,
///   `checksum(internet|internet_pseudo|fletcher|crc32c|crc32, header|header_payload[, nonzero])`
/// * constraints: `error|warning|info: in_range(f, min, max) | equals(f, v) |
///   not_equals(f, v) | min_length(span, n) | max_length(span, n);`
#[macro_export]
macro_rules! protocol {
    (
        id: $id:literal,
        name: $name:literal,
        layer: $layer:ident,
        fields: {
            $(
                $fname:ident : $fty:ident $( ( $($targ:tt)* ) )? $( { $($body:tt)* } )?
                $( = $default:expr )?
                $( , auto = $auto:ident $( ( $($aarg:tt)* ) )? )?
                $( , desc = $desc:literal )?
                ;
            )*
        }
        $( , next: $bfield:ident { $( $bval:expr => $bproto:literal ),* $(,)? } )?
        $( , pseudo_header: ( $phsrc:ident, $phdst:ident ) )?
        $( , constraints: { $( $clevel:ident : $crule:ident ( $($carg:tt)* ) ; )* } )?
        $(,)?
    ) => {{
        #[allow(unused_mut)]
        let mut def = $crate::ProtocolDef::new($id, $name, $crate::ProtocolLayer::$layer);
        def.fields = ::std::vec![
            $(
                $crate::FieldDef::new(
                    stringify!($fname),
                    $crate::__wpg_field_type!($fty $( ( $($targ)* ) )? $( { $($body)* } )?),
                )
                $( .with_default($crate::__wpg_default!($fty, $default)) )?
                $( .with_auto($crate::__wpg_auto!($auto $( ( $($aarg)* ) )?)) )?
                $( .with_description($desc) )?
            ),*
        ];
        $(
            def.next = ::std::option::Option::Some($crate::Binding::new(
                stringify!($bfield),
                ::std::vec![ $( $crate::BindingEntry::new($bval, $bproto) ),* ],
            ));
        )?
        $(
            def.pseudo_header = ::std::option::Option::Some($crate::PseudoHeader {
                src: ::std::string::String::from(stringify!($phsrc)),
                dst: ::std::string::String::from(stringify!($phdst)),
            });
        )?
        $(
            def.constraints = ::std::vec![
                $( $crate::Constraint::new(
                    $crate::__wpg_level!($clevel),
                    $crate::__wpg_constraint!($crule ( $($carg)* )),
                ) ),*
            ];
        )?
        def
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_or {
    (; $d:expr) => {
        $d
    };
    ($v:expr; $d:expr) => {
        $v
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_field_type {
    (uint ($b:expr)) => {
        $crate::FieldType::UInt { bits: $b }
    };
    (int ($b:expr)) => {
        $crate::FieldType::Int { bits: $b }
    };
    (mac) => {
        $crate::FieldType::Mac
    };
    (ipv4) => {
        $crate::FieldType::Ipv4
    };
    (ipv6) => {
        $crate::FieldType::Ipv6
    };
    (nsap) => {
        $crate::FieldType::Nsap
    };
    (raw) => {
        $crate::FieldType::Raw
    };
    (ty ($t:expr)) => {
        $t
    };
    (padding ($a:expr)) => {
        $crate::FieldType::Padding { align: $a }
    };
    (bytes (to_end)) => {
        $crate::FieldType::Bytes {
            len: $crate::Length::ToEnd,
        }
    };
    (bytes (from = $f:ident $(, unit = $u:expr)? $(, adjust = $a:expr)?)) => {
        $crate::FieldType::Bytes {
            len: $crate::Length::FromField {
                field: ::std::string::String::from(stringify!($f)),
                unit: $crate::__wpg_or!($($u)?; 1),
                adjust: $crate::__wpg_or!($($a)?; 0),
            },
        }
    };
    (bytes ($n:expr)) => {
        $crate::FieldType::Bytes {
            len: $crate::Length::Fixed { octets: $n },
        }
    };
    (enum ($b:expr) { $( $v:expr => $n:ident ),* $(,)? }) => {
        $crate::FieldType::Enum {
            bits: $b,
            variants: ::std::vec![ $( $crate::EnumVariant {
                value: $v,
                name: ::std::string::String::from(stringify!($n)),
                description: ::std::option::Option::None,
            } ),* ],
        }
    };
    (enum ($b:expr)) => {
        $crate::FieldType::Enum {
            bits: $b,
            variants: ::std::vec::Vec::new(),
        }
    };
    (flags ($b:expr) { $( $bit:expr => $n:ident ),* $(,)? }) => {
        $crate::FieldType::Flags {
            bits: $b,
            flags: ::std::vec![ $( $crate::FlagBit {
                bit: $bit,
                name: ::std::string::String::from(stringify!($n)),
            } ),* ],
        }
    };
    (flags ($b:expr)) => {
        $crate::FieldType::Flags {
            bits: $b,
            flags: ::std::vec::Vec::new(),
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_default {
    (uint, $v:expr) => {
        $crate::FieldValue::UInt($v)
    };
    (enum, $v:expr) => {
        $crate::FieldValue::UInt($v)
    };
    (flags, $v:expr) => {
        $crate::FieldValue::UInt($v)
    };
    (int, $v:expr) => {
        $crate::FieldValue::Int($v)
    };
    (mac, $v:expr) => {
        $crate::FieldValue::Mac($v)
    };
    (ipv4, $v:expr) => {
        $crate::FieldValue::Ipv4(::std::net::Ipv4Addr::from($v))
    };
    (ipv6, $v:expr) => {
        $crate::FieldValue::Ipv6(::std::net::Ipv6Addr::from($v))
    };
    (ty, $v:expr) => {
        $crate::FieldValue::from($v)
    };
    (nsap, $v:expr) => {
        $crate::FieldValue::Nsap(::std::vec::Vec::<u8>::from($v))
    };
    ($other:ident, $v:expr) => {
        $crate::FieldValue::Bytes(::std::vec::Vec::<u8>::from($v))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_auto {
    (next_protocol) => {
        $crate::AutoCompute::NextProtocol
    };
    (counter) => {
        $crate::AutoCompute::Counter
    };
    (length ($of:ident $(, unit = $unit:ident)? $(, adjust = $a:expr)?)) => {
        $crate::AutoCompute::Length {
            of: $crate::__wpg_length_of!($of),
            unit: $crate::__wpg_or!($($crate::__wpg_unit!($unit))?; $crate::LengthUnit::Bytes),
            adjust: $crate::__wpg_or!($($a)?; 0),
        }
    };
    (checksum ($kind:ident, $cov:ident)) => {
        $crate::AutoCompute::Checksum {
            algorithm: $crate::__wpg_checksum_kind!($kind),
            covers: $crate::__wpg_coverage!($cov),
            nonzero: false,
        }
    };
    (checksum ($kind:ident, $cov:ident, nonzero)) => {
        $crate::AutoCompute::Checksum {
            algorithm: $crate::__wpg_checksum_kind!($kind),
            covers: $crate::__wpg_coverage!($cov),
            nonzero: true,
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_length_of {
    (header) => {
        $crate::LengthOf::Header
    };
    (payload) => {
        $crate::LengthOf::Payload
    };
    (total) => {
        $crate::LengthOf::Total
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_unit {
    (bytes) => {
        $crate::LengthUnit::Bytes
    };
    (words16) => {
        $crate::LengthUnit::Words16
    };
    (words32) => {
        $crate::LengthUnit::Words32
    };
    (words64) => {
        $crate::LengthUnit::Words64
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_checksum_kind {
    (internet) => {
        $crate::ChecksumKind::Internet
    };
    (internet_pseudo) => {
        $crate::ChecksumKind::InternetPseudoHeader
    };
    (fletcher) => {
        $crate::ChecksumKind::Fletcher
    };
    (crc32c) => {
        $crate::ChecksumKind::Crc32c
    };
    (crc32) => {
        $crate::ChecksumKind::Crc32
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_coverage {
    (header) => {
        $crate::ChecksumCoverage::Header
    };
    (header_payload) => {
        $crate::ChecksumCoverage::HeaderAndPayload
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_level {
    (error) => {
        $crate::DiagLevel::Error
    };
    (warning) => {
        $crate::DiagLevel::Warning
    };
    (info) => {
        $crate::DiagLevel::Info
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __wpg_constraint {
    (in_range ($f:ident, $min:expr, $max:expr)) => {
        $crate::ConstraintRule::FieldInRange {
            field: ::std::string::String::from(stringify!($f)),
            min: $min,
            max: $max,
        }
    };
    (equals ($f:ident, $v:expr)) => {
        $crate::ConstraintRule::FieldEquals {
            field: ::std::string::String::from(stringify!($f)),
            value: $v,
        }
    };
    (not_equals ($f:ident, $v:expr)) => {
        $crate::ConstraintRule::FieldNotEquals {
            field: ::std::string::String::from(stringify!($f)),
            value: $v,
        }
    };
    (min_length ($of:ident, $n:expr)) => {
        $crate::ConstraintRule::MinLength {
            of: $crate::__wpg_length_of!($of),
            octets: $n,
        }
    };
    (max_length ($of:ident, $n:expr)) => {
        $crate::ConstraintRule::MaxLength {
            of: $crate::__wpg_length_of!($of),
            octets: $n,
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::{AutoCompute, FieldType, FieldValue, Length, LengthOf, LengthUnit};

    #[test]
    fn macro_builds_expected_ir() {
        let def = protocol! {
            id: "m",
            name: "M",
            layer: L4,
            fields: {
                a: uint(3) = 5, desc = "m-a";
                b: int(5) = -3;
                c: enum(8) { 1 => one, 2 => two };
                len: uint(16), auto = length(payload, unit = words64, adjust = -1);
                d: mac = [1, 2, 3, 4, 5, 6];
                e: ipv6 = [0u16, 0, 0, 0, 0, 0, 0, 1];
                f: bytes(2) = [0xaa, 0xbb];
                g: bytes(from = len, unit = 8);
                h: raw;
            },
        };
        def.validate().unwrap();
        assert_eq!(def.fields[0].default, Some(FieldValue::UInt(5)));
        assert_eq!(def.fields[0].description_key.as_deref(), Some("m-a"));
        assert_eq!(def.fields[1].default, Some(FieldValue::Int(-3)));
        assert!(
            matches!(&def.fields[2].ty, FieldType::Enum { variants, .. } if variants[1].name == "two")
        );
        assert_eq!(
            def.fields[3].auto,
            Some(AutoCompute::Length {
                of: LengthOf::Payload,
                unit: LengthUnit::Words64,
                adjust: -1
            })
        );
        assert_eq!(
            def.fields[5].default,
            Some(FieldValue::Ipv6(std::net::Ipv6Addr::LOCALHOST))
        );
        assert_eq!(
            def.fields[6].default,
            Some(FieldValue::Bytes(vec![0xaa, 0xbb]))
        );
        assert_eq!(
            def.fields[7].ty,
            FieldType::Bytes {
                len: Length::FromField {
                    field: "len".into(),
                    unit: 8,
                    adjust: 0
                }
            }
        );
        assert!(def.next.is_none() && def.constraints.is_empty());
    }
}
