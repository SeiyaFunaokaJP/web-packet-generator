//! Template inputs shared by `send`, `render`, `template show` and `bench`:
//! `--var`, `--vars-file`, `--set` and `--gen`.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use wpg_i18n::{t_args, FluentArgs, Locale};
use wpg_template::overrides::parse_vars_file;
use wpg_template::{FieldOverride, VarOverrides};

use crate::cmd::template::parse_vars;

/// Variable values and field overrides for a template.
#[derive(Debug, Clone, Default, Args)]
pub(crate) struct TemplateInputArgs {
    /// Template variable override (`name=value`), repeatable; wins over --vars-file.
    #[arg(long = "var", value_name = "NAME=VALUE")]
    pub(crate) vars: Vec<String>,
    /// YAML or JSON map of template variable values.
    #[arg(long, value_name = "FILE")]
    pub(crate) vars_file: Option<PathBuf>,
    /// Set a field to a fixed value, repeatable. LAYER is a protocol id, `proto[n]`
    /// or a layer index, e.g. `ipv4.ttl=1`.
    #[arg(long = "set", value_name = "LAYER.FIELD=VALUE")]
    pub(crate) sets: Vec<String>,
    /// Vary a field with a generator, repeatable, e.g. `udp.src_port=range:1000..1999`,
    /// `ipv4.dst=list:10.0.0.1,10.0.0.2`, `ipv4.id=random`.
    #[arg(long = "gen", value_name = "LAYER.FIELD=GEN")]
    pub(crate) gens: Vec<String>,
}

/// The parsed inputs.
pub(crate) struct TemplateInputs {
    /// Variable values: the file, then `--var`.
    pub(crate) vars: VarOverrides,
    /// `--set` then `--gen`, in command-line order.
    pub(crate) fields: Vec<FieldOverride>,
}

fn bad(locale: Locale, key: &str, pairs: &[(&str, String)]) -> ExitCode {
    let mut a = FluentArgs::new();
    for (k, v) in pairs {
        a.set(*k, v.clone());
    }
    eprintln!("{}", t_args(locale, key, &a));
    ExitCode::from(2)
}

impl TemplateInputArgs {
    /// Reads the variables file and parses every argument. Errors are printed;
    /// the exit status is 2 (usage).
    pub(crate) fn parse(&self, locale: Locale) -> Result<TemplateInputs, ExitCode> {
        let mut vars = match &self.vars_file {
            Some(path) => std::fs::read_to_string(path)
                .map_err(|e| e.to_string())
                .and_then(|text| parse_vars_file(&text))
                .map_err(|e| {
                    bad(
                        locale,
                        "cli-vars-file-error",
                        &[("value", path.display().to_string()), ("error", e)],
                    )
                })?,
            None => VarOverrides::new(),
        };
        vars.extend(parse_vars(&self.vars, locale).map_err(|m| {
            eprintln!("{m}");
            ExitCode::from(2)
        })?);
        let mut fields = Vec::with_capacity(self.sets.len() + self.gens.len());
        for s in &self.sets {
            let o = FieldOverride::parse_set(s).ok_or_else(|| {
                bad(
                    locale,
                    "cli-bad-override",
                    &[("flag", "--set".into()), ("value", s.clone())],
                )
            })?;
            fields.push(o);
        }
        for g in &self.gens {
            match FieldOverride::parse_gen(g) {
                Ok(o) => fields.push(o),
                Err(None) => {
                    return Err(bad(
                        locale,
                        "cli-bad-override",
                        &[("flag", "--gen".into()), ("value", g.clone())],
                    ))
                }
                Err(Some(e)) => {
                    return Err(bad(
                        locale,
                        "cli-bad-gen",
                        &[("value", g.clone()), ("error", e)],
                    ))
                }
            }
        }
        Ok(TemplateInputs { vars, fields })
    }
}
