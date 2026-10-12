//! CLI reference (`reference/cli.md`): the `--help` text of every visible command
//! of the clap definition, plus an optional per-command note from Fluent
//! (`docgen-cli-note-<command path>`, e.g. `docgen-cli-note-template-lint`).

use std::fmt::Write as _;

use clap::CommandFactory;
use wpg_i18n::{try_t, Locale};

use super::md::{code, finish, heading, para, tx};
use super::{src_path, Output, GENERATED};
use crate::Cli;

/// Help is rendered at a fixed width so that it does not depend on the terminal.
const WIDTH: usize = 100;

/// The command tree with fixed rendering settings: fixed width, and environment
/// values hidden (`[env: WPG_CONFIG=]` would otherwise show the caller's value).
fn command() -> clap::Command {
    fn fix(cmd: clap::Command) -> clap::Command {
        cmd.term_width(WIDTH)
            .max_term_width(WIDTH)
            .mut_args(|a| a.hide_env_values(true))
            .mut_subcommands(fix)
    }
    let mut cmd = fix(Cli::command());
    cmd.build();
    cmd
}

fn walk(out: &mut String, locale: Locale, cmd: &mut clap::Command, path: &[String]) {
    let title = std::iter::once("wpg".to_owned())
        .chain(path.iter().cloned())
        .collect::<Vec<_>>()
        .join(" ");
    heading(out, 3, &code(&title));
    if !path.is_empty() {
        if let Some(note) = try_t(locale, &format!("docgen-cli-note-{}", path.join("-"))) {
            para(out, &note);
        }
    } else if let Some(note) = try_t(locale, "docgen-cli-note-wpg") {
        para(out, &note);
    }
    let help = cmd.render_long_help().to_string();
    let help: Vec<&str> = help.lines().map(str::trim_end).collect();
    let _ = write!(out, "```text\n{}\n```\n\n", help.join("\n").trim_end());
    let names: Vec<String> = cmd
        .get_subcommands()
        .filter(|s| !s.is_hide_set() && s.get_name() != "help")
        .map(|s| s.get_name().to_owned())
        .collect();
    for name in names {
        let mut p = path.to_vec();
        p.push(name.clone());
        if let Some(sub) = cmd.find_subcommand_mut(&name) {
            walk(out, locale, sub, &p);
        }
    }
}

/// `reference/cli.md` of `locale`.
pub(crate) fn output(locale: Locale) -> Output {
    let mut s = format!("{GENERATED}\n\n");
    heading(&mut s, 1, &tx(locale, "docgen-cli-title"));
    para(&mut s, &tx(locale, "docgen-cli-intro"));
    heading(&mut s, 2, &tx(locale, "docgen-cli-commands"));
    let mut cmd = command();
    walk(&mut s, locale, &mut cmd, &[]);
    Output::whole(src_path(locale, "reference/cli.md"), finish(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_uses_full_command_names_and_hides_dev_commands() {
        let Output { content, .. } = output(Locale::En);
        let super::super::Content::Whole(text) = content else {
            panic!("whole file expected")
        };
        assert!(
            text.contains("Usage: wpg send [OPTIONS] <TEMPLATE>"),
            "{text}"
        );
        assert!(text.contains("### `wpg template lint`"));
        assert!(!text.contains("### `wpg docs"));
    }
}
