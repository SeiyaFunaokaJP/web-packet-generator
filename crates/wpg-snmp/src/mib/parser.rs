//! Parser for SMIv1 (RFC 1155 / 1212 / 1215) and SMIv2 (RFC 2578 / 2579 /
//! 2580) MIB modules.
//!
//! It extracts what a manager needs (OID assignments, OBJECT-TYPE details,
//! textual conventions, notifications, imports) and skips the rest: MACRO
//! definitions, conformance details, unknown clauses and unknown macros.
//! A statement it cannot make sense of becomes a warning and parsing resumes
//! at the next plausible assignment, so one odd construct in a vendor MIB does
//! not lose the whole module.

use super::lexer::{tokenize, Tok, Token};

/// One component of an OID value (`{ iso org(3) 6 }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OidComp {
    /// A descriptor (`ifEntry`).
    Name(String),
    /// A number.
    Num(u32),
    /// `name(number)`.
    NamedNum(String, u32),
}

/// What kind of definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DefKind {
    /// `x OBJECT IDENTIFIER ::= {...}`.
    ObjectIdentifier,
    /// MODULE-IDENTITY.
    ModuleIdentity,
    /// OBJECT-IDENTITY.
    ObjectIdentity,
    /// OBJECT-TYPE.
    ObjectType,
    /// NOTIFICATION-TYPE.
    NotificationType,
    /// TRAP-TYPE (SMIv1; the OID is `enterprise.0.n`).
    TrapType,
    /// OBJECT-GROUP / NOTIFICATION-GROUP.
    Group,
    /// MODULE-COMPLIANCE.
    Compliance,
    /// AGENT-CAPABILITIES.
    Capabilities,
    /// Another (vendor) macro with an OID value.
    Other,
}

/// Base of a syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyntaxBase {
    /// INTEGER.
    Integer,
    /// OCTET STRING.
    OctetString,
    /// OBJECT IDENTIFIER.
    ObjectIdentifier,
    /// BITS.
    Bits,
    /// NULL.
    Null,
    /// SEQUENCE OF x.
    SequenceOf(String),
    /// SEQUENCE { ... } (row types).
    Sequence,
    /// CHOICE { ... }.
    Choice,
    /// A type reference (`DisplayString`, `Counter32`).
    Named(String),
}

/// A SYNTAX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Syntax {
    /// Base type.
    pub base: SyntaxBase,
    /// Named numbers (INTEGER enumerations / BITS positions).
    pub named: Vec<(i64, String)>,
    /// Constraint text (`(SIZE (0..255))`, `(1..10)`).
    pub constraint: Option<String>,
    /// `SIZE (n)` with a single fixed size.
    pub fixed_size: Option<usize>,
}

/// One INDEX item.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct IndexItem {
    /// The index object.
    pub name: String,
    /// `IMPLIED` (variable length without a length sub-identifier).
    pub implied: bool,
}

/// A definition with an OID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Def {
    /// Descriptor.
    pub name: String,
    /// Kind.
    pub kind: DefKind,
    /// OID value.
    pub oid: Vec<OidComp>,
    /// SYNTAX (OBJECT-TYPE).
    pub syntax: Option<Syntax>,
    /// UNITS.
    pub units: Option<String>,
    /// MAX-ACCESS / ACCESS.
    pub access: Option<String>,
    /// STATUS.
    pub status: Option<String>,
    /// DESCRIPTION.
    pub description: Option<String>,
    /// REFERENCE.
    pub reference: Option<String>,
    /// INDEX.
    pub index: Vec<IndexItem>,
    /// AUGMENTS.
    pub augments: Option<String>,
    /// OBJECTS / VARIABLES.
    pub objects: Vec<String>,
    /// DEFVAL text.
    pub defval: Option<String>,
    /// Line of the definition.
    pub line: u32,
}

/// A type assignment (`X ::= TEXTUAL-CONVENTION ...` or `X ::= <syntax>`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    /// Type name.
    pub name: String,
    /// The syntax.
    pub syntax: Syntax,
    /// DISPLAY-HINT.
    pub display_hint: Option<String>,
    /// DESCRIPTION.
    pub description: Option<String>,
    /// STATUS.
    pub status: Option<String>,
    /// Defined with TEXTUAL-CONVENTION.
    pub textual_convention: bool,
}

/// `sym, ... FROM Module`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    /// Module name.
    pub module: String,
    /// Imported symbols.
    pub symbols: Vec<String>,
}

/// A parsed module.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Module {
    /// Module name.
    pub name: String,
    /// IMPORTS.
    pub imports: Vec<Import>,
    /// OID definitions.
    pub defs: Vec<Def>,
    /// Type definitions.
    pub types: Vec<TypeDef>,
    /// Problems that were skipped.
    pub warnings: Vec<String>,
}

/// Result of [`parse`].
#[derive(Debug, Clone, Default)]
pub struct Parsed {
    /// Modules found.
    pub modules: Vec<Module>,
    /// Fatal problems (no module found, a module header that does not parse).
    pub errors: Vec<String>,
}

/// Parses every module in `src`.
#[must_use]
pub fn parse(src: &str) -> Parsed {
    let mut p = Parser {
        toks: tokenize(src),
        pos: 0,
    };
    let mut out = Parsed::default();
    while p.find_module_start() {
        match p.module() {
            Ok(m) => out.modules.push(m),
            Err(e) => {
                out.errors.push(e);
                p.bump();
            }
        }
    }
    if out.modules.is_empty() && out.errors.is_empty() {
        out.errors
            .push("no `<NAME> DEFINITIONS ::= BEGIN` module header found".into());
    }
    out
}

struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

type PResult<T> = Result<T, String>;

/// Macros whose instances are assignments with an OID value.
const MACROS: &[(&str, DefKind)] = &[
    ("MODULE-IDENTITY", DefKind::ModuleIdentity),
    ("OBJECT-IDENTITY", DefKind::ObjectIdentity),
    ("OBJECT-TYPE", DefKind::ObjectType),
    ("NOTIFICATION-TYPE", DefKind::NotificationType),
    ("TRAP-TYPE", DefKind::TrapType),
    ("OBJECT-GROUP", DefKind::Group),
    ("NOTIFICATION-GROUP", DefKind::Group),
    ("MODULE-COMPLIANCE", DefKind::Compliance),
    ("AGENT-CAPABILITIES", DefKind::Capabilities),
];

/// Clause keywords (never the head of a statement).
const CLAUSES: &[&str] = &[
    "SYNTAX",
    "WRITE-SYNTAX",
    "UNITS",
    "MAX-ACCESS",
    "ACCESS",
    "MIN-ACCESS",
    "STATUS",
    "DESCRIPTION",
    "REFERENCE",
    "INDEX",
    "AUGMENTS",
    "DEFVAL",
    "OBJECTS",
    "VARIABLES",
    "NOTIFICATIONS",
    "ENTERPRISE",
    "DISPLAY-HINT",
    "LAST-UPDATED",
    "ORGANIZATION",
    "CONTACT-INFO",
    "REVISION",
];

fn is_macro_name(s: &str) -> bool {
    s.len() > 1
        && s.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && s.chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-')
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n).map(|t| &t.tok)
    }

    fn line(&self) -> u32 {
        self.toks
            .get(self.pos)
            .or_else(|| self.toks.last())
            .map_or(0, |t| t.line)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).map(|t| t.tok.clone());
        self.pos += 1;
        t
    }

    fn at_end(&self) -> bool {
        self.pos >= self.toks.len()
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.peek().is_some_and(|t| t.is(kw)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: &Tok) -> PResult<()> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(self.unexpected(&t.text()))
        }
    }

    fn unexpected(&self, wanted: &str) -> String {
        match self.peek() {
            Some(t) => format!(
                "line {}: expected {wanted}, found `{}`",
                self.line(),
                t.text()
            ),
            None => format!("expected {wanted}, found the end of the file"),
        }
    }

    fn ident(&mut self) -> PResult<String> {
        match self.peek() {
            Some(Tok::Ident(s)) => {
                let s = s.clone();
                self.pos += 1;
                Ok(s)
            }
            _ => Err(self.unexpected("an identifier")),
        }
    }

    /// Skips a balanced `{...}` / `(...)` / `[...]` group starting at the
    /// current opening token (or one token otherwise). Returns the text.
    fn skip_group(&mut self) -> String {
        let mut depth = 0i32;
        let mut text = Vec::new();
        while let Some(t) = self.bump() {
            match t {
                Tok::LBrace | Tok::LParen | Tok::LBracket => depth += 1,
                Tok::RBrace | Tok::RParen | Tok::RBracket => depth -= 1,
                _ => {}
            }
            text.push(t.text());
            if depth <= 0 {
                break;
            }
        }
        text.join(" ")
            .replace("( ", "(")
            .replace(" )", ")")
            .replace(" .. ", "..")
    }

    /// Moves to the next `NAME [{...}] DEFINITIONS` header.
    fn find_module_start(&mut self) -> bool {
        while !self.at_end() {
            if let Some(Tok::Ident(_)) = self.peek() {
                if self.peek_at(1).is_some_and(|t| t.is("DEFINITIONS")) {
                    return true;
                }
                if self.peek_at(1) == Some(&Tok::LBrace) {
                    // `NAME { oid } DEFINITIONS`
                    let save = self.pos;
                    self.pos += 1;
                    self.skip_group();
                    let ok = self.peek().is_some_and(|t| t.is("DEFINITIONS"));
                    self.pos = save;
                    if ok {
                        return true;
                    }
                }
            }
            self.pos += 1;
        }
        false
    }

    fn module(&mut self) -> PResult<Module> {
        let name = self.ident()?;
        if self.peek() == Some(&Tok::LBrace) {
            self.skip_group();
        }
        if !self.eat_kw("DEFINITIONS") {
            return Err(self.unexpected("DEFINITIONS"));
        }
        // `IMPLICIT TAGS` and the like.
        while !self.at_end() && self.peek() != Some(&Tok::Assign) {
            self.pos += 1;
        }
        self.expect(&Tok::Assign)?;
        if !self.eat_kw("BEGIN") {
            return Err(self.unexpected("BEGIN"));
        }
        let mut m = Module {
            name,
            ..Module::default()
        };
        loop {
            match self.peek() {
                None => {
                    m.warnings.push(format!("{}: missing END", m.name));
                    break;
                }
                Some(t) if t.is("END") => {
                    self.pos += 1;
                    break;
                }
                Some(t) if t.is("IMPORTS") => {
                    self.pos += 1;
                    self.imports(&mut m);
                }
                Some(t) if t.is("EXPORTS") => {
                    while let Some(t) = self.bump() {
                        if t == Tok::Semi {
                            break;
                        }
                    }
                }
                Some(Tok::Ident(_)) => {
                    let start = self.pos;
                    if let Err(e) = self.statement(&mut m) {
                        m.warnings.push(e);
                        self.pos = start + 1;
                        self.recover();
                    }
                }
                Some(_) => {
                    self.pos += 1;
                }
            }
        }
        Ok(m)
    }

    /// Skips to the next token that starts a plausible statement.
    fn recover(&mut self) {
        while !self.at_end() {
            if self.is_statement_head() || self.peek().is_some_and(|t| t.is("END")) {
                return;
            }
            self.pos += 1;
        }
    }

    fn is_statement_head(&self) -> bool {
        let Some(Tok::Ident(_)) = self.peek() else {
            return false;
        };
        match self.peek_at(1) {
            // `SYNTAX ::=` inside a broken definition is not a type assignment.
            Some(Tok::Assign) => !self
                .peek()
                .and_then(Tok::ident)
                .is_some_and(|n| CLAUSES.contains(&n)),
            Some(Tok::Ident(n)) if n == "MACRO" => true,
            Some(Tok::Ident(n)) if n == "OBJECT" => {
                self.peek_at(2).is_some_and(|t| t.is("IDENTIFIER"))
                    && self.peek_at(3) == Some(&Tok::Assign)
            }
            Some(Tok::Ident(n)) => MACROS.iter().any(|(m, _)| m == n),
            _ => false,
        }
    }

    fn imports(&mut self, m: &mut Module) {
        let mut pending: Vec<String> = Vec::new();
        while let Some(t) = self.bump() {
            match t {
                Tok::Semi => break,
                Tok::Ident(s) if s == "FROM" => {
                    if let Ok(module) = self.ident() {
                        // An optional OID value after the module name.
                        if self.peek() == Some(&Tok::LBrace) {
                            self.skip_group();
                        }
                        m.imports.push(Import {
                            module,
                            symbols: std::mem::take(&mut pending),
                        });
                    }
                }
                Tok::Ident(s) => pending.push(s),
                Tok::Comma => {}
                // A missing `;`: the import list ended at the first statement.
                _ => {
                    self.pos -= 1;
                    break;
                }
            }
            if self.peek().is_some_and(|t| t.is("END"))
                || self.is_statement_head() && pending.is_empty()
            {
                break;
            }
        }
        if !pending.is_empty() {
            m.warnings.push(format!(
                "{}: imported symbols without FROM: {}",
                m.name,
                pending.join(", ")
            ));
        }
    }

    fn statement(&mut self, m: &mut Module) -> PResult<()> {
        let line = self.line();
        let name = self.ident()?;
        match self.peek().cloned() {
            Some(Tok::Ident(kw)) if kw == "MACRO" => {
                // Skip the macro definition up to its END.
                while let Some(t) = self.bump() {
                    if t.is("END") {
                        break;
                    }
                }
                Ok(())
            }
            Some(Tok::Assign) => {
                self.pos += 1;
                self.type_assignment(m, name)
            }
            Some(Tok::Ident(kw))
                if kw == "OBJECT" && self.peek_at(1).is_some_and(|t| t.is("IDENTIFIER")) =>
            {
                self.pos += 2;
                self.expect(&Tok::Assign)?;
                let oid = self.oid_value()?;
                m.defs
                    .push(Def::new(name, DefKind::ObjectIdentifier, oid, line));
                Ok(())
            }
            Some(Tok::Ident(kw)) if is_macro_name(&kw) || MACROS.iter().any(|(n, _)| *n == kw) => {
                self.pos += 1;
                let kind = MACROS
                    .iter()
                    .find(|(n, _)| *n == kw)
                    .map_or(DefKind::Other, |(_, k)| k.clone());
                self.macro_instance(m, name, kind, line)
            }
            Some(Tok::Ident(_)) => {
                // A value assignment of another type (`x INTEGER ::= 5`).
                while !self.at_end() && self.peek() != Some(&Tok::Assign) {
                    if self.is_statement_head() {
                        return Err(format!("line {line}: cannot parse `{name}`"));
                    }
                    self.pos += 1;
                }
                self.expect(&Tok::Assign)?;
                self.skip_group();
                Ok(())
            }
            _ => Err(self.unexpected(&format!("a definition after `{name}`"))),
        }
    }

    fn type_assignment(&mut self, m: &mut Module, name: String) -> PResult<()> {
        if self.eat_kw("TEXTUAL-CONVENTION") {
            let mut td = TypeDef {
                name,
                syntax: Syntax::new(SyntaxBase::OctetString),
                display_hint: None,
                description: None,
                status: None,
                textual_convention: true,
            };
            let mut have_syntax = false;
            loop {
                match self
                    .peek()
                    .and_then(Tok::ident)
                    .map(str::to_owned)
                    .as_deref()
                {
                    Some("DISPLAY-HINT") => {
                        self.pos += 1;
                        td.display_hint = Some(self.string()?);
                    }
                    Some("STATUS") => {
                        self.pos += 1;
                        td.status = Some(self.ident()?);
                    }
                    Some("DESCRIPTION") => {
                        self.pos += 1;
                        td.description = Some(self.string()?);
                    }
                    Some("REFERENCE") => {
                        self.pos += 1;
                        self.string()?;
                    }
                    Some("SYNTAX") => {
                        self.pos += 1;
                        td.syntax = self.syntax()?;
                        have_syntax = true;
                    }
                    _ => break,
                }
            }
            if !have_syntax {
                return Err(format!("{}: TEXTUAL-CONVENTION without SYNTAX", td.name));
            }
            m.types.push(td);
            return Ok(());
        }
        if self.peek().is_some_and(|t| t.is("BEGIN")) {
            // Something like a nested macro body: skip to END.
            while let Some(t) = self.bump() {
                if t.is("END") {
                    break;
                }
            }
            return Ok(());
        }
        let syntax = self.syntax()?;
        m.types.push(TypeDef {
            name,
            syntax,
            display_hint: None,
            description: None,
            status: None,
            textual_convention: false,
        });
        Ok(())
    }

    fn string(&mut self) -> PResult<String> {
        match self.bump() {
            Some(Tok::Str(s)) => Ok(s),
            // Tolerate an unquoted word.
            Some(Tok::Ident(s) | Tok::Number(s)) => Ok(s),
            _ => {
                self.pos -= 1;
                Err(self.unexpected("a string"))
            }
        }
    }

    /// `{ name, name ... }` (INDEX allows `IMPLIED`).
    fn name_list(&mut self) -> PResult<Vec<IndexItem>> {
        self.expect(&Tok::LBrace)?;
        let mut out = Vec::new();
        let mut implied = false;
        loop {
            match self.bump() {
                Some(Tok::RBrace) => break,
                Some(Tok::Comma) => {}
                Some(Tok::Ident(s)) if s == "IMPLIED" => implied = true,
                Some(Tok::Ident(s)) => {
                    out.push(IndexItem {
                        name: s,
                        implied: std::mem::take(&mut implied),
                    });
                }
                // SMIv1 INDEX may name types (`INDEX { INTEGER }`): skip.
                Some(_) => {}
                None => return Err(self.unexpected("`}`")),
            }
        }
        Ok(out)
    }

    fn macro_instance(
        &mut self,
        m: &mut Module,
        name: String,
        kind: DefKind,
        line: u32,
    ) -> PResult<()> {
        let mut d = Def::new(name, kind, Vec::new(), line);
        let mut enterprise: Option<Vec<OidComp>> = None;
        loop {
            let Some(t) = self.peek().cloned() else {
                return Err(format!(
                    "line {line}: `{}` is not terminated by `::=`",
                    d.name
                ));
            };
            match t {
                Tok::Assign => {
                    self.pos += 1;
                    break;
                }
                Tok::Ident(kw) => {
                    if kw == "END" && self.peek_at(1).is_none() {
                        return Err(format!(
                            "line {line}: `{}` is not terminated by `::=`",
                            d.name
                        ));
                    }
                    self.pos += 1;
                    match kw.as_str() {
                        "SYNTAX" => {
                            let s = self.syntax()?;
                            if d.syntax.is_none() {
                                d.syntax = Some(s);
                            }
                        }
                        "WRITE-SYNTAX" => {
                            self.syntax()?;
                        }
                        "UNITS" => d.units = Some(self.string()?),
                        "DESCRIPTION" => {
                            let s = self.string()?;
                            if d.description.is_none() {
                                d.description = Some(s);
                            }
                        }
                        "REFERENCE" => {
                            let s = self.string()?;
                            if d.reference.is_none() {
                                d.reference = Some(s);
                            }
                        }
                        "DISPLAY-HINT" | "LAST-UPDATED" | "ORGANIZATION" | "CONTACT-INFO"
                        | "REVISION" | "PRODUCT-RELEASE" => {
                            self.string()?;
                        }
                        "MAX-ACCESS" | "ACCESS" => {
                            let a = self.ident()?;
                            if d.access.is_none() {
                                d.access = Some(a);
                            }
                        }
                        "MIN-ACCESS" => {
                            self.ident()?;
                        }
                        "STATUS" => {
                            let s = self.ident()?;
                            if d.status.is_none() {
                                d.status = Some(s);
                            }
                        }
                        "INDEX" => d.index = self.name_list()?,
                        "AUGMENTS" => {
                            d.augments = self.name_list()?.into_iter().next().map(|i| i.name);
                        }
                        "OBJECTS" | "VARIABLES" | "NOTIFICATIONS" => {
                            let l = self.name_list()?;
                            if d.objects.is_empty() {
                                d.objects = l.into_iter().map(|i| i.name).collect();
                            }
                        }
                        "ENTERPRISE" => {
                            if self.peek() == Some(&Tok::LBrace) {
                                enterprise = Some(self.oid_value()?);
                            } else {
                                enterprise = Some(vec![OidComp::Name(self.ident()?)]);
                            }
                        }
                        "DEFVAL" if self.peek() == Some(&Tok::LBrace) => {
                            let t = self.skip_group();
                            d.defval = Some(
                                t.trim_start_matches('{')
                                    .trim_end_matches('}')
                                    .trim()
                                    .to_owned(),
                            );
                        }
                        _ => {}
                    }
                }
                Tok::LBrace | Tok::LParen | Tok::LBracket => {
                    self.skip_group();
                }
                _ => {
                    self.pos += 1;
                }
            }
        }
        if d.kind == DefKind::TrapType {
            let n = match self.bump() {
                Some(Tok::Number(n)) => n
                    .parse::<u32>()
                    .map_err(|_| format!("line {line}: bad trap number `{n}`"))?,
                _ => {
                    self.pos -= 1;
                    return Err(self.unexpected("a trap number"));
                }
            };
            let mut oid =
                enterprise.ok_or_else(|| format!("line {line}: TRAP-TYPE without ENTERPRISE"))?;
            oid.push(OidComp::Num(0));
            oid.push(OidComp::Num(n));
            d.oid = oid;
        } else {
            d.oid = self.oid_value()?;
        }
        m.defs.push(d);
        Ok(())
    }

    fn oid_value(&mut self) -> PResult<Vec<OidComp>> {
        self.expect(&Tok::LBrace)?;
        let mut out = Vec::new();
        loop {
            match self.bump() {
                Some(Tok::RBrace) => break,
                Some(Tok::Ident(s)) => {
                    if self.peek() == Some(&Tok::LParen) {
                        self.pos += 1;
                        let n = match self.bump() {
                            Some(Tok::Number(n)) => n
                                .parse::<u32>()
                                .map_err(|_| format!("bad OID number `{n}`"))?,
                            _ => return Err(self.unexpected("a number")),
                        };
                        self.expect(&Tok::RParen)?;
                        out.push(OidComp::NamedNum(s, n));
                    } else {
                        out.push(OidComp::Name(s));
                    }
                }
                Some(Tok::Number(n)) => {
                    out.push(OidComp::Num(n.parse::<u32>().map_err(|_| {
                        format!("line {}: bad OID number `{n}`", self.line())
                    })?));
                }
                Some(Tok::Comma | Tok::Other('.')) => {}
                _ => {
                    self.pos -= 1;
                    return Err(self.unexpected("an OID component"));
                }
            }
        }
        if out.is_empty() {
            return Err(format!("line {}: empty OID value", self.line()));
        }
        Ok(out)
    }

    /// `name(n), ...` inside braces (enumerations, BITS).
    fn named_numbers(&mut self) -> PResult<Vec<(i64, String)>> {
        self.expect(&Tok::LBrace)?;
        let mut out = Vec::new();
        loop {
            match self.bump() {
                Some(Tok::RBrace) => break,
                Some(Tok::Comma) => {}
                Some(Tok::Ident(name)) => {
                    self.expect(&Tok::LParen)?;
                    let n = match self.bump() {
                        Some(Tok::Number(n)) => {
                            n.parse::<i64>().map_err(|_| format!("bad number `{n}`"))?
                        }
                        _ => return Err(self.unexpected("a number")),
                    };
                    self.expect(&Tok::RParen)?;
                    out.push((n, name));
                }
                _ => {
                    self.pos -= 1;
                    return Err(self.unexpected("a named number"));
                }
            }
        }
        Ok(out)
    }

    fn syntax(&mut self) -> PResult<Syntax> {
        if self.peek() == Some(&Tok::LBracket) {
            self.skip_group();
            if self
                .peek()
                .is_some_and(|t| t.is("IMPLICIT") || t.is("EXPLICIT"))
            {
                self.pos += 1;
            }
        }
        let word = self.ident()?;
        let mut s = match word.as_str() {
            "INTEGER" => {
                let mut s = Syntax::new(SyntaxBase::Integer);
                if self.peek() == Some(&Tok::LBrace) {
                    s.named = self.named_numbers()?;
                }
                s
            }
            "OCTET" => {
                if !self.eat_kw("STRING") {
                    return Err(self.unexpected("STRING"));
                }
                Syntax::new(SyntaxBase::OctetString)
            }
            "OBJECT" => {
                if !self.eat_kw("IDENTIFIER") {
                    return Err(self.unexpected("IDENTIFIER"));
                }
                Syntax::new(SyntaxBase::ObjectIdentifier)
            }
            "BITS" => {
                let mut s = Syntax::new(SyntaxBase::Bits);
                if self.peek() == Some(&Tok::LBrace) {
                    s.named = self.named_numbers()?;
                }
                s
            }
            "NULL" => Syntax::new(SyntaxBase::Null),
            "SEQUENCE" => {
                if self.eat_kw("OF") {
                    Syntax::new(SyntaxBase::SequenceOf(self.ident()?))
                } else {
                    if self.peek() == Some(&Tok::LBrace) {
                        self.skip_group();
                    }
                    Syntax::new(SyntaxBase::Sequence)
                }
            }
            "CHOICE" => {
                if self.peek() == Some(&Tok::LBrace) {
                    self.skip_group();
                }
                Syntax::new(SyntaxBase::Choice)
            }
            _ => {
                let mut s = Syntax::new(SyntaxBase::Named(word));
                if self.peek() == Some(&Tok::LBrace) && !self.brace_is_oid_value() {
                    s.named = self.named_numbers()?;
                }
                s
            }
        };
        if self.peek() == Some(&Tok::LParen) {
            let text = self.skip_group();
            s.fixed_size = fixed_size(&text);
            s.constraint = Some(text);
        }
        Ok(s)
    }

    /// After a SYNTAX type name, `{` usually starts named numbers; it never
    /// starts the `::=` value (that comes after `::=`), so this is a guard for
    /// odd inputs only: named numbers look like `ident (`.
    fn brace_is_oid_value(&self) -> bool {
        !(matches!(self.peek_at(1), Some(Tok::Ident(_))) && self.peek_at(2) == Some(&Tok::LParen))
    }
}

impl Def {
    fn new(name: String, kind: DefKind, oid: Vec<OidComp>, line: u32) -> Self {
        Self {
            name,
            kind,
            oid,
            syntax: None,
            units: None,
            access: None,
            status: None,
            description: None,
            reference: None,
            index: Vec::new(),
            augments: None,
            objects: Vec::new(),
            defval: None,
            line,
        }
    }
}

impl Syntax {
    /// A syntax without named numbers or constraint.
    #[must_use]
    pub fn new(base: SyntaxBase) -> Self {
        Self {
            base,
            named: Vec::new(),
            constraint: None,
            fixed_size: None,
        }
    }
}

/// `(SIZE (6))` → 6; ranges / alternatives → `None`.
fn fixed_size(text: &str) -> Option<usize> {
    let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let inner = t.strip_prefix("(SIZE(")?.strip_suffix("))")?;
    inner.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_type_and_tc() {
        let p = parse(
            r#"
TEST-MIB DEFINITIONS ::= BEGIN
IMPORTS
    MODULE-IDENTITY, OBJECT-TYPE, Integer32, mib-2 FROM SNMPv2-SMI
    TEXTUAL-CONVENTION, DisplayString FROM SNMPv2-TC;

testMIB MODULE-IDENTITY
    LAST-UPDATED "202601010000Z"
    ORGANIZATION "wpg"
    CONTACT-INFO "none"
    DESCRIPTION "Test."
    REVISION "202601010000Z"
    DESCRIPTION "First."
    ::= { mib-2 9999 }

Mac ::= TEXTUAL-CONVENTION
    DISPLAY-HINT "1x:"
    STATUS current
    DESCRIPTION "MAC."
    SYNTAX OCTET STRING (SIZE (6))

testTable OBJECT-TYPE
    SYNTAX SEQUENCE OF TestEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "T."
    ::= { testMIB 1 }

testEntry OBJECT-TYPE
    SYNTAX TestEntry
    MAX-ACCESS not-accessible
    STATUS current
    DESCRIPTION "E."
    INDEX { testIndex, IMPLIED testName }
    ::= { testTable 1 }

TestEntry ::= SEQUENCE { testIndex Integer32, testState INTEGER }

testState OBJECT-TYPE
    SYNTAX INTEGER { up(1), down(2), -- comment
                     testing(3) }
    UNITS "widgets"
    MAX-ACCESS read-write
    STATUS current
    DESCRIPTION "S."
    DEFVAL { up }
    ::= { testEntry 2 }
END
"#,
        );
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let m = &p.modules[0];
        assert_eq!(m.name, "TEST-MIB");
        assert!(m.warnings.is_empty(), "{:?}", m.warnings);
        assert_eq!(m.imports.len(), 2);
        assert_eq!(
            m.imports[1].symbols,
            ["TEXTUAL-CONVENTION", "DisplayString"]
        );
        let mac = m.types.iter().find(|t| t.name == "Mac").unwrap();
        assert_eq!(mac.display_hint.as_deref(), Some("1x:"));
        assert_eq!(mac.syntax.fixed_size, Some(6));
        let entry = m.defs.iter().find(|d| d.name == "testEntry").unwrap();
        assert_eq!(entry.index.len(), 2);
        assert!(entry.index[1].implied);
        let st = m.defs.iter().find(|d| d.name == "testState").unwrap();
        assert_eq!(st.syntax.as_ref().unwrap().named.len(), 3);
        assert_eq!(st.units.as_deref(), Some("widgets"));
        assert_eq!(st.access.as_deref(), Some("read-write"));
        assert_eq!(st.defval.as_deref(), Some("up"));
        assert_eq!(
            st.oid,
            vec![OidComp::Name("testEntry".into()), OidComp::Num(2)]
        );
        let mi = m.defs.iter().find(|d| d.name == "testMIB").unwrap();
        assert_eq!(mi.kind, DefKind::ModuleIdentity);
        assert_eq!(mi.description.as_deref(), Some("Test."));
    }

    #[test]
    fn smiv1_trap_and_recovery() {
        let p = parse(
            r#"
OLD-MIB DEFINITIONS ::= BEGIN
IMPORTS enterprises FROM RFC1155-SMI
        OBJECT-TYPE FROM RFC-1212
        TRAP-TYPE FROM RFC-1215;
acme OBJECT IDENTIFIER ::= { enterprises 99999 }
-------------------------------------------------
-- banner
acmeFan OBJECT-TYPE
    SYNTAX INTEGER
    ACCESS read-only
    STATUS mandatory
    ::= { acme 1 }
acmeWeird SOMETHING-ODD garbage ( ] 12 ::= { acme 7 }
acmeBroken OBJECT-TYPE SYNTAX ::= }
acmeFanFail TRAP-TYPE
    ENTERPRISE acme
    VARIABLES { acmeFan }
    DESCRIPTION "fan"
    ::= 3
END
"#,
        );
        let m = &p.modules[0];
        let names: Vec<_> = m.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"acmeFan"), "{names:?}");
        assert!(names.contains(&"acmeWeird"), "{names:?}");
        assert!(names.contains(&"acmeFanFail"), "{names:?}");
        assert_eq!(m.warnings.len(), 1, "{:?}", m.warnings);
        let t = m.defs.iter().find(|d| d.name == "acmeFanFail").unwrap();
        assert_eq!(
            t.oid,
            vec![
                OidComp::Name("acme".into()),
                OidComp::Num(0),
                OidComp::Num(3)
            ]
        );
        assert_eq!(t.objects, ["acmeFan"]);
    }

    #[test]
    fn no_module() {
        assert!(!parse("hello world").errors.is_empty());
    }
}
