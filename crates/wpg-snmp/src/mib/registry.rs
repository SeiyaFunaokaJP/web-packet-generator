//! The MIB registry: loaded modules, OID ⇄ name resolution, resolved types.
//!
//! Modules can be added in any order; [`Registry::rebuild`] resolves every
//! OID assignment and type reference afresh across all loaded modules. Names
//! are looked up in the module's own definitions, then its imports (also
//! through modules that re-export an imported symbol), then the ASN.1 roots
//! (`iso`, `ccitt`, `joint-iso-ccitt`), and finally — with a warning, as
//! vendor MIBs often forget an import — any loaded module defining the name.

use std::collections::{BTreeMap, HashMap, HashSet};

use super::parser::{self, DefKind, IndexItem, OidComp, Syntax, SyntaxBase};
use crate::oid::Oid;

/// Where a module came from.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", content = "file", rename_all = "snake_case")]
pub enum Source {
    /// Compiled into wpg.
    Bundled,
    /// A user file (file name).
    File(String),
}

/// Base types after resolving textual conventions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseType {
    /// INTEGER / Integer32.
    Integer,
    /// Unsigned32 / Gauge32.
    Unsigned,
    /// Counter32.
    Counter32,
    /// Counter64.
    Counter64,
    /// TimeTicks.
    TimeTicks,
    /// IpAddress / `NetworkAddress`.
    IpAddress,
    /// OCTET STRING.
    OctetString,
    /// OBJECT IDENTIFIER.
    ObjectIdentifier,
    /// BITS.
    Bits,
    /// Opaque.
    Opaque,
    /// NULL.
    Null,
    /// SEQUENCE / SEQUENCE OF / CHOICE / unresolved.
    Other,
}

impl BaseType {
    /// SMI spelling.
    #[must_use]
    pub const fn smi_name(self) -> &'static str {
        match self {
            BaseType::Integer => "INTEGER",
            BaseType::Unsigned => "Unsigned32",
            BaseType::Counter32 => "Counter32",
            BaseType::Counter64 => "Counter64",
            BaseType::TimeTicks => "TimeTicks",
            BaseType::IpAddress => "IpAddress",
            BaseType::OctetString => "OCTET STRING",
            BaseType::ObjectIdentifier => "OBJECT IDENTIFIER",
            BaseType::Bits => "BITS",
            BaseType::Opaque => "Opaque",
            BaseType::Null => "NULL",
            BaseType::Other => "SEQUENCE",
        }
    }

    fn builtin(name: &str) -> Option<Self> {
        Some(match name {
            "Integer32" | "INTEGER" => BaseType::Integer,
            "Unsigned32" | "Gauge32" | "Gauge" | "UInteger32" => BaseType::Unsigned,
            "Counter32" | "Counter" => BaseType::Counter32,
            "Counter64" => BaseType::Counter64,
            "TimeTicks" => BaseType::TimeTicks,
            "IpAddress" | "NetworkAddress" => BaseType::IpAddress,
            "Opaque" => BaseType::Opaque,
            "ObjectName" | "ObjectSyntax" => BaseType::ObjectIdentifier,
            _ => return None,
        })
    }
}

/// A SYNTAX with textual conventions resolved.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedSyntax {
    /// Base type.
    pub base: BaseType,
    /// The (outermost) named type, e.g. `DisplayString`, `InterfaceIndex`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_name: Option<String>,
    /// DISPLAY-HINT (from the nearest TC that has one).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_hint: Option<String>,
    /// INTEGER enumeration / BITS names.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub named: Vec<(i64, String)>,
    /// Constraint text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint: Option<String>,
    /// Fixed `SIZE (n)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fixed_size: Option<usize>,
}

impl ResolvedSyntax {
    /// Display text (`DisplayString (OCTET STRING)`, `INTEGER {up(1), ...}`).
    #[must_use]
    pub fn describe(&self) -> String {
        let mut s = match &self.type_name {
            Some(t) if t != self.base.smi_name() => format!("{t} ({})", self.base.smi_name()),
            _ => self.base.smi_name().to_owned(),
        };
        if let Some(c) = &self.constraint {
            s.push(' ');
            s.push_str(c);
        }
        s
    }

    /// The label of an enumeration value.
    #[must_use]
    pub fn enum_label(&self, v: i64) -> Option<&str> {
        if self.base == BaseType::Bits {
            return None;
        }
        self.named
            .iter()
            .find(|(n, _)| *n == v)
            .map(|(_, s)| s.as_str())
    }
}

/// A resolved definition.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Node {
    /// OID.
    pub oid: Oid,
    /// Descriptor.
    pub name: String,
    /// Defining module.
    pub module: String,
    /// Kind.
    pub kind: DefKind,
    /// Resolved SYNTAX (OBJECT-TYPE).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub syntax: Option<ResolvedSyntax>,
    /// MAX-ACCESS / ACCESS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// STATUS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// DESCRIPTION.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// UNITS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub units: Option<String>,
    /// REFERENCE.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// DEFVAL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub defval: Option<String>,
    /// INDEX.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub index: Vec<IndexItem>,
    /// AUGMENTS.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub augments: Option<String>,
    /// OBJECTS / VARIABLES.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub objects: Vec<String>,
}

impl Node {
    fn implicit(oid: Oid, name: String, module: String) -> Self {
        Self {
            oid,
            name,
            module,
            kind: DefKind::ObjectIdentifier,
            syntax: None,
            access: None,
            status: None,
            description: None,
            units: None,
            reference: None,
            defval: None,
            index: Vec::new(),
            augments: None,
            objects: Vec::new(),
        }
    }

    /// `MODULE::name`.
    #[must_use]
    pub fn qualified(&self) -> String {
        format!("{}::{}", self.module, self.name)
    }

    /// A table (`SEQUENCE OF`).
    #[must_use]
    pub fn is_table(&self) -> bool {
        self.kind == DefKind::ObjectType
            && self.access.as_deref() == Some("not-accessible")
            && self
                .syntax
                .as_ref()
                .is_some_and(|s| s.base == BaseType::Other)
            && self.index.is_empty()
            && self.augments.is_none()
    }

    /// A conceptual row (has INDEX or AUGMENTS).
    #[must_use]
    pub fn is_row(&self) -> bool {
        self.kind == DefKind::ObjectType && (!self.index.is_empty() || self.augments.is_some())
    }
}

/// Load state of a module.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleStatus {
    /// Everything resolved.
    Ok,
    /// Loaded with problems (missing imports, unresolved definitions).
    Partial,
    /// Nothing could be resolved.
    Error,
}

/// A module as listed by [`Registry::modules`].
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleInfo {
    /// Module name.
    pub name: String,
    /// Origin.
    pub source: Source,
    /// Load state.
    pub status: ModuleStatus,
    /// Modules named in IMPORTS that are not loaded.
    pub missing_imports: Vec<String>,
    /// Problems (parse warnings, unresolved names).
    pub errors: Vec<String>,
    /// Resolved definitions.
    pub nodes: usize,
}

struct Loaded {
    parsed: parser::Module,
    source: Source,
}

/// Result of [`Registry::add`].
#[derive(Debug, Clone, Default)]
pub struct AddResult {
    /// Modules found in the text.
    pub modules: Vec<String>,
    /// Fatal parse errors (no module found).
    pub errors: Vec<String>,
}

/// One level of the tree ([`Registry::children`]).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Child {
    /// OID.
    pub oid: Oid,
    /// The last sub-identifier.
    pub arc: u32,
    /// Descriptor (`None` for an undefined intermediate OID).
    pub name: Option<String>,
    /// Module.
    pub module: Option<String>,
    /// Kind.
    pub kind: Option<DefKind>,
    /// Whether anything is defined below.
    pub has_children: bool,
}

/// Error of [`Registry::resolve`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// Unknown descriptor.
    #[error("unknown object `{0}`")]
    UnknownName(String),
    /// Unknown module in `MODULE::name`.
    #[error("module `{0}` is not loaded")]
    UnknownModule(String),
    /// Malformed instance suffix.
    #[error("invalid OID `{0}`")]
    Invalid(String),
}

/// Loaded modules and the resolved OID tree.
#[derive(Default)]
pub struct Registry {
    modules: BTreeMap<String, Loaded>,
    nodes: BTreeMap<Oid, Node>,
    by_name: HashMap<String, Vec<Oid>>,
    by_qualified: HashMap<(String, String), Oid>,
    status: HashMap<String, (Vec<String>, Vec<String>, usize)>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry")
            .field("modules", &self.modules.len())
            .field("nodes", &self.nodes.len())
            .finish()
    }
}

const ROOTS: &[(&str, u32)] = &[("ccitt", 0), ("iso", 1), ("joint-iso-ccitt", 2)];

impl Registry {
    /// An empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A registry with the bundled modules ([`super::BUNDLED`]).
    #[must_use]
    pub fn bundled() -> Self {
        let mut r = Self::new();
        for (_, text) in super::BUNDLED {
            r.add(text, &Source::Bundled);
        }
        r.rebuild();
        r
    }

    /// Parses `text` and adds (or replaces) its modules. Call
    /// [`Registry::rebuild`] afterwards (or use [`Registry::load`]).
    pub fn add(&mut self, text: &str, source: &Source) -> AddResult {
        let p = parser::parse(text);
        let mut res = AddResult {
            modules: Vec::new(),
            errors: p.errors,
        };
        for m in p.modules {
            res.modules.push(m.name.clone());
            self.modules.insert(
                m.name.clone(),
                Loaded {
                    parsed: m,
                    source: source.clone(),
                },
            );
        }
        res
    }

    /// [`Registry::add`] + [`Registry::rebuild`].
    pub fn load(&mut self, text: &str, source: &Source) -> AddResult {
        let r = self.add(text, source);
        self.rebuild();
        r
    }

    /// Removes a module (rebuild afterwards). Returns whether it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        self.modules.remove(name).is_some()
    }

    /// Source of a module.
    #[must_use]
    pub fn source_of(&self, module: &str) -> Option<&Source> {
        self.modules.get(module).map(|l| &l.source)
    }

    /// Whether a module is loaded.
    #[must_use]
    pub fn has_module(&self, module: &str) -> bool {
        self.modules.contains_key(module)
    }

    /// Number of resolved nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// The modules with their load state.
    #[must_use]
    pub fn modules(&self) -> Vec<ModuleInfo> {
        self.modules
            .iter()
            .map(|(name, l)| {
                let (missing, errors, nodes) = self.status.get(name).cloned().unwrap_or_default();
                let total = l.parsed.defs.len();
                let status = if missing.is_empty() && errors.is_empty() {
                    ModuleStatus::Ok
                } else if nodes == 0 && total > 0 {
                    ModuleStatus::Error
                } else {
                    ModuleStatus::Partial
                };
                ModuleInfo {
                    name: name.clone(),
                    source: l.source.clone(),
                    status,
                    missing_imports: missing,
                    errors,
                    nodes,
                }
            })
            .collect()
    }

    /// Resolves everything again.
    pub fn rebuild(&mut self) {
        let mut r = Resolver {
            reg: &self.modules,
            oid_cache: HashMap::new(),
            in_progress: HashSet::new(),
            warnings: HashMap::new(),
            global: HashMap::new(),
        };
        for (mname, l) in &self.modules {
            for d in &l.parsed.defs {
                r.global
                    .entry(d.name.clone())
                    .or_insert_with(|| mname.clone());
            }
            for t in &l.parsed.types {
                r.global
                    .entry(t.name.clone())
                    .or_insert_with(|| mname.clone());
            }
        }
        let mut nodes: BTreeMap<Oid, Node> = BTreeMap::new();
        let mut by_name: HashMap<String, Vec<Oid>> = HashMap::new();
        let mut by_qualified = HashMap::new();
        let mut status = HashMap::new();
        for (mname, l) in &self.modules {
            let missing: Vec<String> = l
                .parsed
                .imports
                .iter()
                .map(|i| i.module.clone())
                .filter(|m| !self.modules.contains_key(m))
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect();
            let mut errors = l.parsed.warnings.clone();
            let mut count = 0;
            for d in &l.parsed.defs {
                let Some(oid) = r.def_oid(mname, &d.name) else {
                    errors.push(format!(
                        "line {}: cannot resolve the OID of `{}` ({})",
                        d.line,
                        d.name,
                        describe_oid_value(&d.oid)
                    ));
                    continue;
                };
                count += 1;
                let syntax = d.syntax.as_ref().map(|s| r.resolve_syntax(mname, s, 0));
                let node = Node {
                    oid: oid.clone(),
                    name: d.name.clone(),
                    module: mname.clone(),
                    kind: d.kind.clone(),
                    syntax,
                    access: d.access.clone(),
                    status: d.status.clone(),
                    description: d.description.clone(),
                    units: d.units.clone(),
                    reference: d.reference.clone(),
                    defval: d.defval.clone(),
                    index: d.index.clone(),
                    augments: d.augments.clone(),
                    objects: d.objects.clone(),
                };
                by_qualified.insert((mname.clone(), d.name.clone()), oid.clone());
                let names = by_name.entry(d.name.clone()).or_default();
                if !names.contains(&oid) {
                    names.push(oid.clone());
                }
                // Prefer the "real" definition over a bare OBJECT IDENTIFIER
                // and SMIv2 over SMIv1 duplicates (RFC1213-MIB vs IF-MIB).
                let replace = match nodes.get(&oid) {
                    None => true,
                    Some(old) => rank(&node) > rank(old),
                };
                if replace {
                    nodes.insert(oid, node);
                }
            }
            if let Some(w) = r.warnings.get(mname) {
                errors.extend(w.iter().cloned());
            }
            status.insert(mname.clone(), (missing, errors, count));
        }
        // `name(n)` components of OID values (`{ iso org(3) dod(6) 1 }`) and
        // the ASN.1 roots name intermediate nodes nobody defines explicitly.
        let mut implicit: Vec<(Oid, String, String)> = ROOTS
            .iter()
            .map(|(n, v)| (Oid::from([*v]), (*n).to_owned(), "SNMPv2-SMI".to_owned()))
            .collect();
        for (mname, l) in &self.modules {
            for d in &l.parsed.defs {
                let Some(oid) = r
                    .oid_cache
                    .get(&(mname.clone(), d.name.clone()))
                    .cloned()
                    .flatten()
                else {
                    continue;
                };
                let Some(tail) = d.oid.len().checked_sub(1) else {
                    continue;
                };
                for (i, c) in d.oid.iter().enumerate() {
                    if let OidComp::NamedNum(n, _) = c {
                        let keep = oid.len().saturating_sub(tail - i);
                        if keep > 0 {
                            implicit.push((
                                Oid::from(&oid.arcs()[..keep]),
                                n.clone(),
                                mname.clone(),
                            ));
                        }
                    }
                }
            }
        }
        for (oid, name, module) in implicit {
            if nodes.contains_key(&oid) {
                continue;
            }
            by_name.entry(name.clone()).or_default().push(oid.clone());
            by_qualified
                .entry((module.clone(), name.clone()))
                .or_insert_with(|| oid.clone());
            nodes.insert(oid.clone(), Node::implicit(oid, name, module));
        }
        self.nodes = nodes;
        self.by_name = by_name;
        self.by_qualified = by_qualified;
        self.status = status;
    }

    /// The node defined exactly at `oid`.
    #[must_use]
    pub fn node(&self, oid: &Oid) -> Option<&Node> {
        self.nodes.get(oid)
    }

    /// The node with the longest prefix of `oid`, and the remaining
    /// sub-identifiers (the instance).
    #[must_use]
    pub fn lookup<'a>(&'a self, oid: &'a Oid) -> Option<(&'a Node, &'a [u32])> {
        let a = oid.arcs();
        (1..=a.len()).rev().find_map(|n| {
            self.nodes
                .get(&Oid::from(&a[..n]))
                .map(|node| (node, &a[n..]))
        })
    }

    /// A node by (optionally qualified) descriptor.
    #[must_use]
    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        if let Some((m, n)) = name.split_once("::") {
            return self
                .by_qualified
                .get(&(m.to_owned(), n.to_owned()))
                .and_then(|o| self.nodes.get(o));
        }
        self.by_name
            .get(name)
            .and_then(|v| v.iter().find_map(|o| self.nodes.get(o)))
    }

    /// `IF-MIB::ifDescr.3` for an OID (numeric when nothing is known).
    #[must_use]
    pub fn name_of(&self, oid: &Oid) -> String {
        match self.lookup(oid) {
            Some((n, rest)) => {
                let mut s = n.qualified();
                for a in rest {
                    s.push('.');
                    s.push_str(&a.to_string());
                }
                s
            }
            None => oid.to_string(),
        }
    }

    /// Resolves user input to an OID: dotted decimal (`1.3.6.1.2.1.1.3.0`,
    /// `.1.3.6...`), `name`, `name.instance` or `MODULE::name.instance`
    /// (`IF-MIB::ifDescr.3`, `sysUpTime.0`).
    ///
    /// # Errors
    /// Unknown names / modules, malformed suffixes.
    pub fn resolve(&self, input: &str) -> Result<Oid, ResolveError> {
        let s = input.trim();
        if s.is_empty() {
            return Err(ResolveError::Invalid(input.to_owned()));
        }
        if let Ok(o) = s.parse::<Oid>() {
            return Ok(o);
        }
        let (module, rest) = match s.split_once("::") {
            Some((m, r)) => (Some(m.trim()), r.trim()),
            None => (None, s),
        };
        let (name, suffix) = match rest.split_once('.') {
            Some((n, suf)) => (n, Some(suf)),
            None => (rest, None),
        };
        let base = match module {
            Some(m) => {
                if !self.modules.contains_key(m) {
                    return Err(ResolveError::UnknownModule(m.to_owned()));
                }
                self.by_qualified
                    .get(&(m.to_owned(), name.to_owned()))
                    .cloned()
                    .ok_or_else(|| ResolveError::UnknownName(format!("{m}::{name}")))?
            }
            None => self
                .by_name
                .get(name)
                .and_then(|v| v.first())
                .cloned()
                .ok_or_else(|| ResolveError::UnknownName(name.to_owned()))?,
        };
        match suffix {
            None | Some("") => Ok(base),
            Some(suf) => {
                let arcs = suf
                    .split('.')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| ResolveError::Invalid(input.to_owned()))?;
                Ok(base.child(&arcs))
            }
        }
    }

    /// One level below `oid` (the empty OID lists the roots).
    #[must_use]
    pub fn children(&self, oid: &Oid) -> Vec<Child> {
        let depth = oid.len();
        let mut out: BTreeMap<u32, Child> = BTreeMap::new();
        for (o, _) in self.nodes.range(oid.clone()..) {
            if !o.starts_with(oid) {
                break;
            }
            if o.len() <= depth {
                continue;
            }
            let arc = o.arcs()[depth];
            let child_oid = Oid::from(&o.arcs()[..=depth]);
            let deeper = o.len() > depth + 1;
            let e = out.entry(arc).or_insert_with(|| {
                let n = self.nodes.get(&child_oid);
                Child {
                    oid: child_oid.clone(),
                    arc,
                    name: n.map(|n| n.name.clone()),
                    module: n.map(|n| n.module.clone()),
                    kind: n.map(|n| n.kind.clone()),
                    has_children: false,
                }
            });
            e.has_children |= deeper;
        }
        out.into_values().collect()
    }

    /// Nodes whose descriptor matches `q` (case-insensitive; prefix matches
    /// first, then substrings; `MODULE::` narrows to a module).
    #[must_use]
    pub fn search(&self, q: &str, limit: usize) -> Vec<&Node> {
        let q = q.trim();
        let (module, q) = match q.split_once("::") {
            Some((m, r)) => (Some(m), r),
            None => (None, q),
        };
        let lq = q.to_ascii_lowercase();
        let mut prefix = Vec::new();
        let mut infix = Vec::new();
        for n in self.nodes.values() {
            if module.is_some_and(|m| m != n.module) {
                continue;
            }
            let ln = n.name.to_ascii_lowercase();
            if ln.starts_with(&lq) {
                prefix.push(n);
            } else if !lq.is_empty() && ln.contains(&lq) {
                infix.push(n);
            }
        }
        prefix.sort_by(|a, b| {
            a.name
                .len()
                .cmp(&b.name.len())
                .then_with(|| a.name.cmp(&b.name))
        });
        infix.sort_by(|a, b| a.name.cmp(&b.name));
        prefix.into_iter().chain(infix).take(limit).collect()
    }

    /// The row definition (INDEX) of a row node, following AUGMENTS.
    #[must_use]
    pub fn index_of<'a>(&'a self, row: &'a Node) -> &'a [IndexItem] {
        if row.index.is_empty() {
            if let Some(a) = &row.augments {
                if let Some(base) = self
                    .node_by_name(&format!("{}::{a}", row.module))
                    .or_else(|| self.node_by_name(a))
                {
                    return &base.index;
                }
            }
        }
        &row.index
    }

    /// The node of an INDEX object named in `row`'s module scope.
    #[must_use]
    pub fn index_node(&self, row: &Node, name: &str) -> Option<&Node> {
        self.node_by_name(&format!("{}::{name}", row.module))
            .or_else(|| self.node_by_name(name))
    }
}

/// Preference among nodes defined at the same OID.
fn rank(n: &Node) -> u8 {
    let kind = match n.kind {
        DefKind::ObjectIdentifier => 0,
        _ => 2,
    };
    // SMIv2 modules have MAX-ACCESS values like `read-only` too; RFC1213-MIB
    // is the classic SMIv1 duplicate of IF-MIB / IP-MIB / SNMPv2-MIB.
    let v2 = u8::from(n.module != "RFC1213-MIB" && n.module != "RFC1155-SMI");
    kind + v2
}

fn describe_oid_value(v: &[OidComp]) -> String {
    let parts: Vec<String> = v
        .iter()
        .map(|c| match c {
            OidComp::Name(n) => n.clone(),
            OidComp::Num(n) => n.to_string(),
            OidComp::NamedNum(n, x) => format!("{n}({x})"),
        })
        .collect();
    format!("{{ {} }}", parts.join(" "))
}

struct Resolver<'a> {
    reg: &'a BTreeMap<String, Loaded>,
    oid_cache: HashMap<(String, String), Option<Oid>>,
    in_progress: HashSet<(String, String)>,
    warnings: HashMap<String, Vec<String>>,
    /// First module defining each descriptor / type (last-resort lookup).
    global: HashMap<String, String>,
}

impl Resolver<'_> {
    /// The module that defines `name` as seen from `module` (own definitions,
    /// imports, re-exports).
    fn owner(&mut self, module: &str, name: &str, depth: usize) -> Option<String> {
        let l = self.reg.get(module)?;
        if l.parsed.defs.iter().any(|d| d.name == name)
            || l.parsed.types.iter().any(|t| t.name == name)
        {
            return Some(module.to_owned());
        }
        if depth > 6 {
            return None;
        }
        for imp in &l.parsed.imports {
            if imp.symbols.iter().any(|s| s == name) && self.reg.contains_key(&imp.module) {
                if let Some(o) = self.owner(&imp.module, name, depth + 1) {
                    return Some(o);
                }
            }
        }
        None
    }

    fn owner_or_global(&mut self, module: &str, name: &str) -> Option<String> {
        if let Some(o) = self.owner(module, name, 0) {
            return Some(o);
        }
        let g = self.global.get(name).cloned()?;
        let w = self.warnings.entry(module.to_owned()).or_default();
        let msg = format!("`{name}` is not imported; using the definition from {g}");
        if !w.contains(&msg) {
            w.push(msg);
        }
        Some(g)
    }

    fn def_oid(&mut self, module: &str, name: &str) -> Option<Oid> {
        let key = (module.to_owned(), name.to_owned());
        if let Some(c) = self.oid_cache.get(&key) {
            return c.clone();
        }
        if !self.in_progress.insert(key.clone()) {
            return None; // cycle
        }
        let reg = self.reg;
        let comps = reg
            .get(module)
            .and_then(|l| l.parsed.defs.iter().find(|d| d.name == name))
            .map(|d| d.oid.clone());
        let res = comps.and_then(|c| self.eval(module, &c));
        self.in_progress.remove(&key);
        self.oid_cache.insert(key, res.clone());
        res
    }

    fn eval(&mut self, module: &str, comps: &[OidComp]) -> Option<Oid> {
        let mut arcs = Vec::with_capacity(comps.len() + 8);
        for (i, c) in comps.iter().enumerate() {
            match c {
                OidComp::Num(n) | OidComp::NamedNum(_, n) => arcs.push(*n),
                OidComp::Name(n) if i == 0 => {
                    if let Some((_, v)) = ROOTS.iter().find(|(r, _)| r == n) {
                        if self.owner(module, n, 0).is_none() {
                            arcs.push(*v);
                            continue;
                        }
                    }
                    let owner = self.owner_or_global(module, n)?;
                    let base = self.def_oid(&owner, n)?;
                    arcs.extend_from_slice(base.arcs());
                }
                OidComp::Name(_) => return None,
            }
        }
        Some(Oid::new(arcs))
    }

    fn resolve_syntax(&mut self, module: &str, s: &Syntax, depth: usize) -> ResolvedSyntax {
        let mut out = ResolvedSyntax {
            base: BaseType::Other,
            type_name: None,
            display_hint: None,
            named: s.named.clone(),
            constraint: s.constraint.clone(),
            fixed_size: s.fixed_size,
        };
        match &s.base {
            SyntaxBase::Integer => out.base = BaseType::Integer,
            SyntaxBase::OctetString => out.base = BaseType::OctetString,
            SyntaxBase::ObjectIdentifier => out.base = BaseType::ObjectIdentifier,
            SyntaxBase::Bits => out.base = BaseType::Bits,
            SyntaxBase::Null => out.base = BaseType::Null,
            SyntaxBase::Sequence | SyntaxBase::SequenceOf(_) | SyntaxBase::Choice => {}
            SyntaxBase::Named(n) => {
                out.type_name = Some(n.clone());
                let td = if depth < 12 {
                    self.owner_or_global(module, n).and_then(|owner| {
                        self.reg
                            .get(&owner)
                            .and_then(|l| l.parsed.types.iter().find(|t| &t.name == n))
                            .map(|t| (owner, t.clone()))
                    })
                } else {
                    None
                };
                match td {
                    // SNMPv2-SMI's own `Counter32 ::= [APPLICATION 1] ...`
                    // and friends: the builtin wins.
                    _ if BaseType::builtin(n).is_some() => {
                        out.base = BaseType::builtin(n).unwrap_or(BaseType::Other);
                    }
                    Some((owner, td)) => {
                        let inner = self.resolve_syntax(&owner, &td.syntax, depth + 1);
                        out.base = inner.base;
                        out.display_hint = td.display_hint.clone().or(inner.display_hint);
                        if out.named.is_empty() {
                            out.named = inner.named;
                        }
                        if out.constraint.is_none() {
                            out.constraint = inner.constraint;
                        }
                        if out.fixed_size.is_none() {
                            out.fixed_size = inner.fixed_size;
                        }
                    }
                    None => {}
                }
                // SMIv1 spellings without DISPLAY-HINT (RFC 1213).
                if out.display_hint.is_none() {
                    out.display_hint = match n.as_str() {
                        "DisplayString" => Some("255a".into()),
                        "PhysAddress" | "MacAddress" => Some("1x:".into()),
                        _ => None,
                    };
                }
            }
        }
        out
    }
}
