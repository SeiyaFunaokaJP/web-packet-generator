//! Template → [`Packet`] + [`StreamSpec`] (also the engine behind [`crate::lint()`]).

use std::collections::{BTreeMap, BTreeSet};

use wpg_core::generator::{CompileOptions, Generator};
use wpg_core::notation;
use wpg_core::stream::{Assignment, Expansion, FieldRef, Mode, Overrides, StreamSpec};
use wpg_core::{
    raw_protocol, FcsMode, FieldType, Layer, Packet, ProtocolDef, Resolve, RAW_PROTOCOL_ID,
};

use crate::input::numeric_operand;
use crate::lint::{LintIssue, LintLevel};
use crate::{FcsChoice, GeneratorSpec, StreamMode, Template, TemplateError, TxProfile};

/// Run-time values of template variables (`--var name=value`).
pub type VarOverrides = BTreeMap<String, String>;

/// A compiled template.
#[derive(Debug, Clone)]
pub struct Compiled {
    /// The base packet (generated fields are unset; see [`apply_overrides`]).
    pub packet: Packet,
    /// Generators of the stream; `FieldRef.layer` is the 0-based layer index.
    pub stream: StreamSpec,
    /// Transmit profile.
    pub tx: TxProfile,
}

/// Compiles `template`: substitutes `${var}` (defaults, then `vars`), parses the
/// human-notation fields against the catalog and assembles the stream.
///
/// # Errors
/// [`TemplateError::Lint`] with all findings when there is at least one error
/// (unknown protocol / field / variable, unparsable value or generator, ...).
pub fn compile(
    template: &Template,
    vars: &VarOverrides,
    resolve: &dyn Resolve,
) -> Result<Compiled, TemplateError> {
    let a = analyze(template, Some(vars), resolve);
    match a.compiled {
        Some(c) if !a.issues.iter().any(|i| i.level == LintLevel::Error) => Ok(c),
        _ => Err(TemplateError::Lint(a.issues)),
    }
}

/// Applies expansion results to a packet: sets each field as Manual.
///
/// # Errors
/// [`TemplateError::Lint`] if a [`FieldRef`] does not name a layer of the packet.
pub fn apply_overrides(packet: &mut Packet, overrides: &Overrides) -> Result<(), TemplateError> {
    for (r, v) in overrides {
        let layer = r
            .layer
            .parse::<usize>()
            .ok()
            .and_then(|i| packet.layers.get_mut(i))
            .ok_or_else(|| {
                TemplateError::Lint(vec![LintIssue::error(
                    "stream",
                    "lint-invalid-stream",
                    format!("stream target `{r}` does not name a layer"),
                )
                .arg("value", r)])
            })?;
        layer.fields.insert(r.field.clone(), v.clone());
    }
    Ok(())
}

pub(crate) struct Analysis {
    pub(crate) issues: Vec<LintIssue>,
    /// `Some` when everything was checked and there were no errors.
    pub(crate) compiled: Option<Compiled>,
}

enum VarErr {
    Unknown(String),
    Missing(String),
    Unterminated(String),
}

struct Vars {
    values: BTreeMap<String, Option<String>>,
    used: BTreeSet<String>,
    lenient: bool,
    skipped: bool,
}

impl Vars {
    /// Expands `${name}`; `Ok(None)` = a variable without value in lenient mode.
    fn expand(&mut self, s: &str) -> Result<Option<String>, VarErr> {
        let mut out = String::with_capacity(s.len());
        let mut rest = s;
        while let Some(at) = rest.find("${") {
            out.push_str(&rest[..at]);
            let after = &rest[at + 2..];
            let end = after
                .find('}')
                .ok_or_else(|| VarErr::Unterminated(s.to_owned()))?;
            let name = after[..end].trim();
            self.used.insert(name.to_owned());
            match self.values.get(name) {
                None => return Err(VarErr::Unknown(name.to_owned())),
                Some(Some(v)) => out.push_str(v),
                Some(None) if self.lenient => {
                    self.skipped = true;
                    return Ok(None);
                }
                Some(None) => return Err(VarErr::Missing(name.to_owned())),
            }
            rest = &after[end + 1..];
        }
        out.push_str(rest);
        Ok(Some(out))
    }
}

fn var_issue(path: &str, e: &VarErr) -> LintIssue {
    match e {
        VarErr::Unknown(n) => LintIssue::error(
            path,
            "lint-unknown-variable",
            format!("unknown variable `{n}`"),
        )
        .arg("value", n),
        VarErr::Missing(n) => LintIssue::error(
            path,
            "lint-missing-variable",
            format!("variable `{n}` has no default and no value was given"),
        )
        .arg("value", n),
        VarErr::Unterminated(s) => LintIssue::error(
            path,
            "lint-var-syntax",
            format!("unterminated `${{` in `{s}`"),
        )
        .arg("value", s),
    }
}

fn substitute_generator(g: &Generator, vars: &mut Vars) -> Result<Option<Generator>, VarErr> {
    macro_rules! sub {
        ($s:expr) => {
            match vars.expand($s)? {
                Some(v) => v,
                None => return Ok(None),
            }
        };
    }
    macro_rules! sub_opt {
        ($s:expr) => {
            match $s {
                Some(s) => Some(sub!(s)),
                None => None,
            }
        };
    }
    Ok(Some(match g {
        Generator::Range { start, end, step } => Generator::Range {
            start: sub!(start),
            end: sub!(end),
            step: *step,
        },
        Generator::List { values } => {
            let mut out = Vec::with_capacity(values.len());
            for v in values {
                out.push(sub!(v));
            }
            Generator::List { values: out }
        }
        Generator::Random { min, max, seed } => Generator::Random {
            min: sub_opt!(min),
            max: sub_opt!(max),
            seed: *seed,
        },
        Generator::RandomMac { oui, seed } => Generator::RandomMac {
            oui: sub!(oui),
            seed: *seed,
        },
        Generator::File { path } => Generator::File { path: sub!(path) },
        Generator::Expr { expr } => Generator::Expr { expr: sub!(expr) },
        other @ Generator::Counter { .. } => other.clone(),
    }))
}

/// Rewrites enum / flag names in the operands of `g` to numbers.
fn numeric_generator(g: Generator, ty: &FieldType) -> Result<Generator, String> {
    let num = |s: &str| numeric_operand(s, ty);
    Ok(match g {
        Generator::Range { start, end, step } => Generator::Range {
            start: num(&start)?,
            end: num(&end)?,
            step,
        },
        Generator::List { values } => Generator::List {
            values: values.iter().map(|v| num(v)).collect::<Result<_, _>>()?,
        },
        Generator::Random { min, max, seed } => Generator::Random {
            min: min.as_deref().map(num).transpose()?,
            max: max.as_deref().map(num).transpose()?,
            seed,
        },
        other => other,
    })
}

fn text_generator(s: &str, ty: &FieldType) -> Result<Generator, String> {
    if matches!(ty, FieldType::Enum { .. } | FieldType::Flags { .. }) {
        // names are only meaningful as a comma list here (`|` belongs to flags)
        let parts: Vec<String> = s
            .split(',')
            .map(|p| numeric_operand(p, ty))
            .collect::<Result<_, _>>()?;
        return notation::parse_generator(&parts.join(","), ty).map_err(|e| e.to_string());
    }
    notation::parse_generator(s, ty).map_err(|e| e.to_string())
}

fn resolve_layer_ref(s: &str, layers: &[Layer]) -> Result<FieldRef, String> {
    let (l, f) = s
        .rsplit_once('.')
        .filter(|(l, f)| !l.is_empty() && !f.is_empty())
        .ok_or_else(|| format!("`{s}` is not `<layer>.<field>`"))?;
    let index = if let Ok(i) = l.parse::<usize>() {
        (i < layers.len())
            .then_some(i)
            .ok_or_else(|| format!("layer {i} does not exist"))?
    } else {
        let mut hits = layers
            .iter()
            .enumerate()
            .filter(|(_, layer)| layer.protocol == l)
            .map(|(i, _)| i);
        let first = hits
            .next()
            .ok_or_else(|| format!("no layer with protocol `{l}`"))?;
        if hits.next().is_some() {
            return Err(format!(
                "protocol `{l}` occurs more than once; use its index"
            ));
        }
        first
    };
    Ok(FieldRef::new(index.to_string(), f))
}

#[allow(clippy::too_many_lines)] // one linear pass over the template
pub(crate) fn analyze(
    t: &Template,
    overrides: Option<&VarOverrides>,
    resolve: &dyn Resolve,
) -> Analysis {
    let mut issues: Vec<LintIssue> = Vec::new();
    let raw = raw_protocol();
    let lookup = |id: &str| -> Option<&ProtocolDef> {
        resolve
            .protocol(id)
            .or_else(|| (id == RAW_PROTOCOL_ID).then_some(&raw))
    };

    // -- schema / meta -------------------------------------------------------
    if t.schema != crate::SCHEMA_VERSION {
        issues.push(
            LintIssue::error(
                "schema",
                "lint-schema-version",
                format!(
                    "unsupported schema version {} (supported: {})",
                    t.schema,
                    crate::SCHEMA_VERSION
                ),
            )
            .arg("value", t.schema)
            .arg("expected", crate::SCHEMA_VERSION),
        );
    }
    if t.meta.name.trim().is_empty() {
        issues.push(LintIssue::warning(
            "meta.name",
            "lint-missing-name",
            "meta.name is empty",
        ));
    }

    // -- variables -------------------------------------------------------------
    let mut vars = Vars {
        values: BTreeMap::new(),
        used: BTreeSet::new(),
        lenient: overrides.is_none(),
        skipped: false,
    };
    for (i, v) in t.variables.iter().enumerate() {
        let path = format!("variables[{i}]");
        if vars.values.contains_key(&v.name) {
            issues.push(
                LintIssue::error(
                    &path,
                    "lint-duplicate-variable",
                    format!("variable `{}` is declared more than once", v.name),
                )
                .arg("value", &v.name),
            );
            continue;
        }
        let value = overrides
            .and_then(|o| o.get(&v.name).cloned())
            .or_else(|| v.default.clone());
        vars.values.insert(v.name.clone(), value);
    }
    if let Some(o) = overrides {
        for name in o.keys() {
            if !vars.values.contains_key(name) {
                issues.push(
                    LintIssue::error(
                        "variables",
                        "lint-override-unknown",
                        format!("unknown variable override `{name}`"),
                    )
                    .arg("value", name),
                );
            }
        }
    }

    // -- layers ------------------------------------------------------------------
    if t.layers.is_empty() {
        issues.push(LintIssue::error(
            "layers",
            "lint-no-layers",
            "the template has no layers",
        ));
    }
    let mut layers: Vec<Layer> = Vec::with_capacity(t.layers.len());
    let mut defs: Vec<Option<&ProtocolDef>> = Vec::with_capacity(t.layers.len());
    let mut assignments: Vec<Assignment> = Vec::new();
    let seed = t.stream.seed.unwrap_or(0);

    for (i, tl) in t.layers.iter().enumerate() {
        let base = format!("layers[{i}]");
        let mut layer = Layer::new(&tl.protocol);
        let def = lookup(&tl.protocol);
        defs.push(def);
        let Some(def) = def else {
            issues.push(
                LintIssue::error(
                    format!("{base}.protocol"),
                    "lint-unknown-protocol",
                    format!("unknown protocol `{}`", tl.protocol),
                )
                .arg("protocol", &tl.protocol),
            );
            layers.push(layer);
            continue;
        };

        for (name, input) in &tl.fields {
            let path = format!("{base}.fields.{name}");
            let Some(fd) = def.field(name) else {
                issues.push(unknown_field(&path, &tl.protocol, name));
                continue;
            };
            if input.is_auto() {
                continue;
            }
            let expanded = input.map_strings(&mut |s| vars.expand(s));
            let input = match expanded {
                Ok(Some(v)) => v,
                Ok(None) => continue,
                Err(e) => {
                    issues.push(var_issue(&path, &e));
                    continue;
                }
            };
            match input.to_value(&fd.ty) {
                Ok(v) => {
                    layer.fields.insert(name.clone(), v);
                }
                Err(msg) => issues.push(
                    LintIssue::error(
                        &path,
                        "lint-invalid-value",
                        format!("invalid value for `{name}`: {msg}"),
                    )
                    .arg("field", name)
                    .arg("value", msg),
                ),
            }
        }

        for (name, spec) in &tl.generators {
            let path = format!("{base}.generators.{name}");
            let Some(fd) = def.field(name) else {
                issues.push(unknown_field(&path, &tl.protocol, name));
                continue;
            };
            if tl.fields.contains_key(name) {
                issues.push(
                    LintIssue::warning(
                        &path,
                        "lint-field-and-generator",
                        format!(
                            "`{name}` has both a fixed value and a generator; the generator wins"
                        ),
                    )
                    .arg("field", name),
                );
            }
            let generator = match generator_of(spec, &fd.ty, &mut vars) {
                Ok(Some(g)) => g,
                Ok(None) => continue,
                Err(issue) => {
                    issues.push(issue.at(&path, name));
                    continue;
                }
            };
            // trial compile: type mismatches, zero steps, bad expressions, ...
            if !matches!(generator, Generator::File { .. }) {
                let opts = CompileOptions {
                    seed,
                    salt: assignments.len() as u64 + 1,
                    files: None,
                };
                if let Err(e) = generator.compile(&fd.ty, &opts) {
                    issues.push(
                        LintIssue::error(
                            &path,
                            "lint-invalid-generator",
                            format!("generator for `{name}`: {e}"),
                        )
                        .arg("field", name)
                        .arg("value", e),
                    );
                    continue;
                }
            }
            assignments.push(Assignment {
                target: FieldRef::new(i.to_string(), name),
                ty: fd.ty.clone(),
                generator,
            });
        }
        layers.push(layer);
    }

    // -- binding sanity ----------------------------------------------------------
    for (i, pair) in t.layers.windows(2).enumerate() {
        let (Some(def), cur, next) = (defs[i], &pair[0], &pair[1]) else {
            continue;
        };
        let Some(binding) = &def.next else { continue };
        if matches!(next.protocol.as_str(), "payload" | "raw") || defs[i + 1].is_none() {
            continue;
        }
        let set_by_user =
            cur.fields.contains_key(&binding.field) || cur.generators.contains_key(&binding.field);
        if !set_by_user && binding.value_for(&next.protocol).is_none() {
            issues.push(
                LintIssue::warning(
                    format!("layers[{}].protocol", i + 1),
                    "lint-next-protocol-manual",
                    format!(
                        "`{}` is not in the binding table of `{}`: `{}` will be Manual",
                        next.protocol, cur.protocol, binding.field
                    ),
                )
                .arg("protocol", &cur.protocol)
                .arg("next", &next.protocol)
                .arg("field", &binding.field),
            );
        }
    }

    // -- unused variables ----------------------------------------------------------
    for (i, v) in t.variables.iter().enumerate() {
        if !vars.used.contains(&v.name) {
            issues.push(
                LintIssue::warning(
                    format!("variables[{i}]"),
                    "lint-unused-variable",
                    format!("variable `{}` is never used", v.name),
                )
                .arg("value", &v.name),
            );
        }
    }

    // -- stream ----------------------------------------------------------------------
    let mut order = Vec::new();
    for (i, o) in t.stream.order.iter().enumerate() {
        match resolve_layer_ref(o, &layers) {
            Ok(r) if assignments.iter().any(|a| a.target == r) => order.push(r),
            Ok(r) => issues.push(
                LintIssue::error(
                    format!("stream.order[{i}]"),
                    "lint-unknown-order",
                    format!("`{r}` has no generator"),
                )
                .arg("value", o),
            ),
            Err(msg) => issues.push(
                LintIssue::error(
                    format!("stream.order[{i}]"),
                    "lint-invalid-stream",
                    msg.clone(),
                )
                .arg("value", msg),
            ),
        }
    }
    let mode = match t.stream.mode {
        StreamMode::Zip => Mode::Zip,
        StreamMode::Product => Mode::Product,
        StreamMode::Nested => Mode::Nested { order },
    };
    let spec = StreamSpec {
        assignments,
        mode,
        count: t.stream.count.or(t.tx.count),
        seed,
    };
    let has_file = spec
        .assignments
        .iter()
        .any(|a| matches!(a.generator, Generator::File { .. }));
    if !has_file && !issues.iter().any(|i| i.level == LintLevel::Error) {
        if let Err(e) = Expansion::new(&spec, None) {
            issues.push(
                LintIssue::error("stream", "lint-invalid-stream", e.to_string()).arg("value", e),
            );
        }
    }

    // -- frame options ---------------------------------------------------------------
    let mut packet = Packet::from_layers(layers);
    packet.pad_to_min = t.packet.pad_to_min;
    packet.fcs = match t.packet.fcs {
        FcsChoice::Auto => Some(FcsMode::Auto),
        FcsChoice::Bad => Some(FcsMode::Bad),
        FcsChoice::None => None,
    };

    let ok = !vars.skipped && !issues.iter().any(|i| i.level == LintLevel::Error);
    Analysis {
        compiled: ok.then(|| Compiled {
            packet,
            stream: spec,
            tx: t.tx.clone(),
        }),
        issues,
    }
}

fn unknown_field(path: &str, protocol: &str, field: &str) -> LintIssue {
    LintIssue::error(
        path,
        "lint-unknown-field",
        format!("protocol `{protocol}` has no field `{field}`"),
    )
    .arg("protocol", protocol)
    .arg("field", field)
}

/// A generator problem before the path is known.
struct GenIssue {
    message: String,
    var: Option<VarErr>,
}

impl GenIssue {
    fn at(self, path: &str, field: &str) -> LintIssue {
        match self.var {
            Some(e) => var_issue(path, &e),
            None => LintIssue::error(
                path,
                "lint-invalid-generator",
                format!("generator for `{field}`: {}", self.message),
            )
            .arg("field", field)
            .arg("value", self.message),
        }
    }
}

fn generator_of(
    spec: &GeneratorSpec,
    ty: &FieldType,
    vars: &mut Vars,
) -> Result<Option<Generator>, GenIssue> {
    let var = |e: VarErr| GenIssue {
        message: String::new(),
        var: Some(e),
    };
    let msg = |m: String| GenIssue {
        message: m,
        var: None,
    };
    match spec {
        GeneratorSpec::Text(s) => {
            let Some(s) = vars.expand(s).map_err(var)? else {
                return Ok(None);
            };
            text_generator(&s, ty).map(Some).map_err(msg)
        }
        GeneratorSpec::Full(g) => {
            let Some(g) = substitute_generator(g, vars).map_err(var)? else {
                return Ok(None);
            };
            numeric_generator(g, ty).map(Some).map_err(msg)
        }
    }
}
