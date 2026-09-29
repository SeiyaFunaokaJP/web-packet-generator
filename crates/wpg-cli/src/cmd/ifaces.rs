//! `wpg ifaces`: list network interfaces (TODO §2.7).

use std::process::ExitCode;

use wpg_i18n::{t, t_args, FluentArgs, Locale};
use wpg_sys::ifaces::{self, Iface};

use crate::cmd::cli_locale;

/// Runs `wpg ifaces`.
pub(crate) fn run(args: &crate::IfacesArgs) -> ExitCode {
    let locale = cli_locale();
    let list = match ifaces::list() {
        Ok(l) => l,
        Err(e) => {
            let mut a = FluentArgs::new();
            a.set("error", e.to_string());
            eprintln!("{}", t_args(locale, "ifaces-error", &a));
            return ExitCode::FAILURE;
        }
    };
    if args.json {
        match serde_json::to_string_pretty(&list) {
            Ok(s) => println!("{s}"),
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::FAILURE;
            }
        }
    } else if list.is_empty() {
        println!("{}", t(locale, "ifaces-none"));
    } else {
        print!("{}", render_table(locale, &list));
    }
    ExitCode::SUCCESS
}

/// Terminal cell width: CJK / kana count as two columns.
fn display_width(s: &str) -> usize {
    s.chars()
        .map(|c| if u32::from(c) >= 0x2e80 { 2 } else { 1 })
        .sum()
}

fn state_text(locale: Locale, i: &Iface) -> String {
    let base = if !i.up {
        t(locale, "ifaces-state-down")
    } else if !i.running {
        t(locale, "ifaces-state-no-carrier")
    } else {
        t(locale, "ifaces-state-up")
    };
    if i.loopback {
        format!("{base}, {}", t(locale, "ifaces-state-loopback"))
    } else {
        base
    }
}

fn or_dash(s: String) -> String {
    if s.is_empty() {
        "-".to_owned()
    } else {
        s
    }
}

fn row(locale: Locale, i: &Iface) -> Vec<String> {
    let name = match &i.description {
        Some(d) => format!("{} ({d})", i.name),
        None => i.name.clone(),
    };
    let join = |v: Vec<String>| or_dash(v.join(", "));
    vec![
        name,
        i.index.to_string(),
        i.mac
            .as_ref()
            .map_or_else(|| "-".to_owned(), ifaces::format_mac),
        join(i.ipv4.iter().map(ToString::to_string).collect()),
        join(i.ipv6.iter().map(ToString::to_string).collect()),
        i.mtu.map_or_else(|| "-".to_owned(), |m| m.to_string()),
        state_text(locale, i),
        i.speed_mbps
            .map_or_else(|| "-".to_owned(), |s| format!("{s} Mbit/s")),
        i.driver.clone().unwrap_or_else(|| "-".to_owned()),
    ]
}

/// Renders the interface table (header + one line per interface).
pub(crate) fn render_table(locale: Locale, list: &[Iface]) -> String {
    let header: Vec<String> = [
        "ifaces-col-name",
        "ifaces-col-index",
        "ifaces-col-mac",
        "ifaces-col-ipv4",
        "ifaces-col-ipv6",
        "ifaces-col-mtu",
        "ifaces-col-state",
        "ifaces-col-speed",
        "ifaces-col-driver",
    ]
    .iter()
    .map(|k| t(locale, k))
    .collect();
    let mut rows = vec![header];
    rows.extend(list.iter().map(|i| row(locale, i)));
    let widths: Vec<usize> = (0..rows[0].len())
        .map(|c| rows.iter().map(|r| display_width(&r[c])).max().unwrap_or(0))
        .collect();
    let mut out = String::new();
    for r in &rows {
        let mut line = String::new();
        for (c, cell) in r.iter().enumerate() {
            line.push_str(cell);
            if c + 1 < r.len() {
                let pad = widths[c] - display_width(cell) + 2;
                line.extend(std::iter::repeat_n(' ', pad));
            }
        }
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Iface {
        Iface {
            name: "eth0".into(),
            index: 2,
            mac: Some([2, 0x42, 0xac, 0x11, 0, 2]),
            ipv4: vec!["10.0.0.2".parse().unwrap()],
            ipv6: vec![],
            mtu: Some(1500),
            up: true,
            running: true,
            loopback: false,
            speed_mbps: Some(10_000),
            driver: Some("virtio_net".into()),
            description: None,
        }
    }

    #[test]
    fn table_has_header_and_row() {
        let out = render_table(Locale::En, &[sample()]);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("NAME"));
        assert!(lines[1].starts_with("eth0"));
        assert!(lines[1].contains("02:42:ac:11:00:02"));
        assert!(lines[1].contains("10000 Mbit/s"));
        assert!(lines[1].contains("virtio_net"));
    }

    #[test]
    fn japanese_headers_and_states() {
        let mut i = sample();
        i.running = false;
        let out = render_table(Locale::Ja, &[i]);
        assert!(out.lines().next().unwrap().contains("ドライバ"));
        assert!(out.contains("キャリアなし"));
    }

    #[test]
    fn loopback_state() {
        let mut i = sample();
        i.loopback = true;
        assert_eq!(state_text(Locale::En, &i), "up, loopback");
        i.up = false;
        assert_eq!(state_text(Locale::En, &i), "down, loopback");
    }

    #[test]
    fn width_counts_cjk_double() {
        assert_eq!(display_width("abc"), 3);
        assert_eq!(display_width("状態"), 4);
    }
}
