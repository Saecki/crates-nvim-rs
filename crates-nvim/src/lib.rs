use bumpalo::collections::String as BString;
use nvim_oxi::conversion::ToObject;
use nvim_oxi::serde::Serializer;
use nvim_oxi::{Dictionary, Function, Object};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use toml::container::Container;
use toml::map::{
    MapArray, MapArrayInlineEntry, MapNode, MapTable, MapTableEntry, MapTableEntryRepr,
    MapTableEntryReprKind, Scalar,
};
use toml::onevec::OneVec;
use toml::parse::{Ident, StringVal};
use toml::Span;

use crate::error::{CargoError, Error, Warning};

mod error;

struct Ctx {
    errors: Vec<Error>,
    warnings: Vec<Warning>,
}

impl From<toml::Ctx> for Ctx {
    fn from(value: toml::Ctx) -> Self {
        Self {
            errors: value.errors.into_iter().map(Into::into).collect(),
            warnings: value.warnings.into_iter().map(Into::into).collect(),
        }
    }
}

impl Ctx {
    fn error(&mut self, error: impl Into<Error>) {
        self.errors.push(error.into());
    }

    fn warn(&mut self, warning: impl Into<Warning>) {
        self.warnings.push(warning.into());
    }
}

#[nvim_oxi::module]
pub fn crates_nvim() -> nvim_oxi::Result<Dictionary> {
    let parse_toml = Function::from_fn::<_, nvim_oxi::Error>(move |()| {
        let buf = nvim_oxi::api::get_current_buf();
        let num_lines = buf.line_count()?;
        let raw_lines = buf.get_lines(0..num_lines, true)?;

        let mut lines = Vec::with_capacity(raw_lines.len());
        let mut toml_ctx = toml::Ctx::default();
        let container = Container::parse_with(&mut toml_ctx, |bump| {
            let mut text = BString::new_in(bump);
            for line in raw_lines.into_iter() {
                // HACK
                let str = unsafe { std::str::from_utf8_unchecked(line.as_bytes()) };
                text.push_str(str);
                text.push('\n');

                // HACK
                lines.push(str.to_string());
            }
            text.into_bump_str()
        });

        let mut ctx = Ctx::from(toml_ctx);
        let toml = container.toml();
        let crates = find(&mut ctx, &lines, &toml.map);

        crates
            .into_iter()
            .map(|c| c.serialize(Serializer::new()).map_err(Into::into))
            .collect::<Result<Vec<Object>, _>>()
    });

    Ok(Dictionary::from_iter([("parse_toml", parse_toml)]))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Section {
    pub workspace: bool,
    pub target: Option<String>,
    pub kind: SectionKind,
    pub name: Option<String>,
    pub name_col: Range,
    pub lines: Range,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum SectionKind {
    Default = 1,
    Dev = 2,
    Build = 3,
}

#[derive(Debug)]
pub struct CrateBuilder {
    /// The explicit name is either the name of the package, or a rename
    /// if the following syntax is used:
    /// explicit_name = { package = "package" }
    pub explicit_name: String,
    pub explicit_name_col: Range,
    pub lines: Range,
    pub syntax: TomlCrateSyntax,
    pub section: Section,
    pub vers: Option<TomlCrateVers>,
    pub registry: Option<TomlCrateRegistry>,
    pub path: Option<TomlCratePath>,
    pub git: Option<TomlCrateGit>,
    pub branch: Option<TomlCrateBranch>,
    pub rev: Option<TomlCrateRev>,
    pub pkg: Option<TomlCratePkg>,
    pub workspace: Option<TomlCrateWorkspace>,
    pub opt: Option<TomlCrateOpt>,
    pub def: Option<TomlCrateDef>,
    pub feat: Option<TomlCrateFeat>,
}

impl CrateBuilder {
    fn new(
        explicit_name: String,
        explicit_name_col: Range,
        lines: Range,
        syntax: TomlCrateSyntax,
        section: Section,
    ) -> Self {
        Self {
            explicit_name,
            explicit_name_col,
            lines,
            syntax,
            section,
            vers: None,
            registry: None,
            path: None,
            git: None,
            branch: None,
            rev: None,
            pkg: None,
            workspace: None,
            opt: None,
            def: None,
            feat: None,
        }
    }

    fn try_build(self, ctx: &mut Ctx) -> Option<TomlCrate> {
        let dep_kind = (self.workspace.is_some().then_some(DepKind::Workspace))
            .or_else(|| self.path.is_some().then_some(DepKind::Path))
            .or_else(|| self.git.is_some().then_some(DepKind::Git))
            .or_else(|| self.vers.is_some().then_some(DepKind::Registry));

        let Some(dep_kind) = dep_kind else {
            todo!("warning");
        };

        Some(TomlCrate {
            explicit_name: self.explicit_name,
            explicit_name_col: self.explicit_name_col,
            lines: self.lines,
            syntax: self.syntax,
            section: self.section,
            dep_kind,
            vers: self.vers,
            registry: self.registry,
            path: self.path,
            git: self.git,
            branch: self.branch,
            rev: self.rev,
            pkg: self.pkg,
            workspace: self.workspace,
            opt: self.opt,
            def: self.def,
            feat: self.feat,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrate {
    /// The explicit name is either the name of the package, or a rename
    /// if the following syntax is used:
    /// explicit_name = { package = "package" }
    pub explicit_name: String,
    pub explicit_name_col: Range,
    pub lines: Range,
    pub syntax: TomlCrateSyntax,
    pub section: Section,
    pub dep_kind: DepKind,
    pub vers: Option<TomlCrateVers>,
    pub registry: Option<TomlCrateRegistry>,
    pub path: Option<TomlCratePath>,
    pub git: Option<TomlCrateGit>,
    pub branch: Option<TomlCrateBranch>,
    pub rev: Option<TomlCrateRev>,
    pub pkg: Option<TomlCratePkg>,
    pub workspace: Option<TomlCrateWorkspace>,
    pub opt: Option<TomlCrateOpt>,
    pub def: Option<TomlCrateDef>,
    pub feat: Option<TomlCrateFeat>,
}

impl ToObject for TomlCrate {
    fn to_object(self) -> Result<Object, nvim_oxi::conversion::Error> {
        self.serialize(Serializer::new()).map_err(Into::into)
    }
}

impl TomlCrate {
    pub fn plain(name: &Ident, version: &StringVal, section: Section) -> Self {
        Self {
            explicit_name: name.text.to_string(),
            explicit_name_col: Range::new(name.lit_span().start.char, name.lit_span().end.char),
            lines: Range::from_start_len(name.lit_span().start.line, 1),
            syntax: TomlCrateSyntax::Plain,
            section,
            dep_kind: DepKind::Registry,
            vers: Some(TomlCrateVers::plain(version)),
            registry: None,
            path: None,
            git: None,
            branch: None,
            rev: None,
            pkg: None,
            workspace: None,
            opt: None,
            def: None,
            feat: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum TomlCrateSyntax {
    Plain = 1,
    InlineTable = 2,
    Table = 3,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateVers {
    pub reqs: Vec<Requirement>,
    pub text: String,
    pub is_pre: bool,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

impl TomlCrateVers {
    fn plain(value: &StringVal) -> Self {
        Self {
            reqs: todo!(),
            text: value.text.to_string(),
            is_pre: todo!(),
            line: value.lit_span.start.line,
            col: Range::new(value.lit_span.start.char, value.lit_span.end.char),
            decl_col: Range::new(0, value.lit_span.end.char),
            quote: todo!(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateRegistry {
    pub text: String,
    pub is_pre: bool,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCratePath {
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateGit {
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateBranch {
    pub text: String,
    pub line: u32,
    /// 0-indexed
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateRev {
    pub text: String,
    pub line: u32,
    /// 0-indexed
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCratePkg {
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
    pub quote: Quotes,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateWorkspace {
    pub enabled: bool,
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateOpt {
    pub enabled: bool,
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateDef {
    pub enabled: bool,
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlCrateFeat {
    pub items: Vec<TomlFeature>,
    pub text: String,
    /// 0-indexed
    pub line: u32,
    pub col: Range,
    pub decl_col: Range,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum DepKind {
    Registry = 1,
    Path = 2,
    Git = 3,
    Workspace = 4,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TomlFeature {
    pub name: String,
    /// relative to to the start of the features text
    pub col: Range,
    /// relative to to the start of the features text
    pub decl_col: Range,
    pub quote: Quotes,
    pub comma: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Quotes {
    pub s: String,
    pub e: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Requirement {
    pub cond: Cond,
    /// relative to to the start of the requirement text
    pub cond_col: Range,
    pub vers: SemVer,
    /// relative to to the start of the requirement text
    pub vers_col: Range,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub pre: String,
    pub meta: String,
}

#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum Cond {
    Eq = 1,
    Lt = 2,
    Le = 3,
    Gt = 4,
    Ge = 5,
    Cr = 6,
    Tl = 7,
    Wl = 8,
    Bl = 9,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename = "Span")]
pub struct Range {
    /// 0-indexed inclusive
    pub s: u32,
    /// 0-indexed exclusive
    pub e: u32,
}

impl Range {
    pub fn new(s: u32, e: u32) -> Self {
        Self { s, e }
    }

    pub fn from_start_len(s: u32, len: u32) -> Self {
        Self { s, e: s + len }
    }

    pub fn from_span_cols(span: Span) -> Self {
        Self {
            s: span.start.char,
            e: span.end.char,
        }
    }

    pub fn from_span_lines(span: Span) -> Self {
        Self {
            s: span.start.line,
            e: span.end.line + 1,
        }
    }
}

fn find(ctx: &mut Ctx, lines: &[impl AsRef<str>], map: &MapTable) -> Vec<TomlCrate> {
    let mut crates = Vec::new();
    for (key, entry) in map.iter() {
        match *key {
            // TODO
            "package" => (),
            "lib" => (),
            "bin" => (),
            "example" => (),
            "test" => (),
            "bench" => (),
            "badges" => (),
            "features" => (),
            "lints" => (),
            "patch" => (),
            "replace" => (),
            "profile" => (),
            "workspace" => (),

            "dependencies" => match &entry.node {
                MapNode::Table(dependencies) => parse_dependencies(
                    ctx,
                    &mut crates,
                    lines,
                    &entry.reprs,
                    dependencies,
                    SectionKind::Default,
                    false,
                    None,
                ),
                _ => ctx.error(CargoError::ExpectedTable(
                    key.to_string(),
                    entry.reprs.first().key.repr_ident().lit_span(),
                )),
            },
            "dev-dependencies" => todo!(),
            "build-dependencies" => todo!(),
            "target" => todo!(),
            _ => todo!("warning unused {key}"),
        }
    }
    crates
}

fn parse_dependencies(
    ctx: &mut Ctx,
    crates: &mut Vec<TomlCrate>,
    lines: &[impl AsRef<str>],
    dependencies_reprs: &OneVec<MapTableEntryRepr>,
    dependencies: &MapTable,
    kind: SectionKind,
    workspace: bool,
    target: Option<String>,
) {
    for (crate_name, entry) in dependencies.iter() {
        let crt = match &entry.node {
            MapNode::Scalar(Scalar::String(version)) => {
                let name = entry.reprs.first().key.repr_ident();
                let section = Section {
                    workspace,
                    target,
                    kind,
                    name: todo!(),
                    name_col: todo!(),
                    lines: todo!(),
                };
                TomlCrate::plain(name, version, section)
            }
            MapNode::Scalar(_) => todo!("error"),
            MapNode::Table(t) => {
                let repr = entry.reprs.first();
                let syntax = match &repr.kind {
                    MapTableEntryReprKind::Table(_) => TomlCrateSyntax::Table,
                    MapTableEntryReprKind::ArrayEntry(_) => todo!("error"),
                    MapTableEntryReprKind::ToplevelAssignment(_) => todo!(),
                    MapTableEntryReprKind::InlineTableAssignment(_) => todo!(),
                };
                let mut builder = CrateBuilder::new(
                    crate_name.to_string(),
                    Range::from_span_cols(repr.key.repr_ident().lit_span()),
                    Range::from_span_lines(repr.kind.span()),
                    syntax,
                    todo!("section"),
                );
                for (k, e) in t.iter() {
                    match *k {
                        "version" => {
                            if let Some(s) = expect_string_in_table(ctx, e) {
                                todo!();
                            }
                        }
                        "registry" => todo!(),
                        "path" => todo!(),
                        "git" => todo!(),
                        "branch" => todo!(),
                        "rev" => todo!(),
                        "package" => todo!(),
                        "default-features" => todo!(),
                        "default_features" => todo!("warning or error"),
                        "features" => builder.feat = parse_dependency_features(ctx, lines, e),
                        "workspace" => todo!(),
                        "optional" => todo!(),
                        _ => todo!("warning"),
                    }
                }

                match builder.try_build(ctx) {
                    Some(c) => c,
                    None => continue,
                }
            }
            MapNode::Array(_) => todo!("error"),
        };

        crates.push(crt);
    }
}

fn parse_dependency_features(
    ctx: &mut Ctx,
    lines: &[impl AsRef<str>],
    entry: &MapTableEntry,
) -> Option<TomlCrateFeat> {
    let array = expect_array_in_table(ctx, entry)?;
    let features = match array {
        MapArray::Toplevel(_) => todo!("error: array of tables"),
        MapArray::Inline(i) => i,
    };

    let features_span = features.repr.span();
    if features_span.start.line != features_span.end.line {
        todo!("multiline arrays")
    }

    let mut items = Vec::with_capacity(features.len());
    for (i, e) in features.iter().enumerate() {
        let Some(f) = expect_string_in_array(ctx, e) else {
            continue;
        };

        let decl_start_col = (i.checked_sub(1))
            .map(|i| features[i].repr.span().end.char)
            .unwrap_or_else(|| features.repr.l_par.char + 1);

        let decl_end_col = (e.repr.comma.map(|p| p.char))
            .or_else(|| features.get(i + 1).map(|f| f.repr.span().start.char))
            .or_else(|| features.repr.r_par.map(|p| p.char))
            .unwrap_or(features.repr.span().end.char);

        items.push(TomlFeature {
            name: f.text.to_string(),
            col: Range::from_span_cols(f.text_span()),
            decl_col: Range::new(decl_start_col, decl_end_col),
            quote: Quotes {
                s: f.l_quote().to_string(),
                e: f.r_quote().map(ToString::to_string),
            },
            comma: e.repr.comma.is_some(),
        });
    }

    let line = features_span.start.line;
    let col = Range::from_span_cols(features_span);
    let decl_col = Range::from_span_cols(entry.reprs.first().kind.span());

    let text = lines[line as usize].as_ref()[col.s as usize..col.e as usize].to_string();

    Some(TomlCrateFeat {
        items,
        text,
        line,
        col,
        decl_col,
    })
}

fn expect_array_in_table<'a>(
    ctx: &mut Ctx,
    entry: &'a MapTableEntry<'a>,
) -> Option<&'a MapArray<'a>> {
    match &entry.node {
        MapNode::Array(a) => Some(a),
        _ => {
            let repr = entry.reprs.first();
            let key = repr.key.repr_ident().text.to_string();
            let span = Span::across(repr.key.repr_ident().lit_span(), repr.kind.span());
            ctx.error(CargoError::ExpectedArrayInTable(key, span));
            None
        }
    }
}

fn expect_string_in_table<'a>(
    ctx: &mut Ctx,
    entry: &'a MapTableEntry<'a>,
) -> Option<&'a StringVal<'a>> {
    match &entry.node {
        MapNode::Scalar(Scalar::String(s)) => Some(s),
        _ => {
            let repr = entry.reprs.first();
            let key = repr.key.repr_ident().text.to_string();
            let span = Span::across(repr.key.repr_ident().lit_span(), repr.kind.span());
            ctx.error(CargoError::ExpectedStringInTable(key, span));
            None
        }
    }
}

fn expect_string_in_array<'a>(
    ctx: &mut Ctx,
    entry: &'a MapArrayInlineEntry<'a>,
) -> Option<&'a StringVal<'a>> {
    match &entry.node {
        MapNode::Scalar(Scalar::String(s)) => Some(s),
        _ => {
            ctx.error(CargoError::ExpectedStringInArray(entry.repr.span()));
            None
        }
    }
}
