use common::diagnostic::{Diagnostic, DiagnosticHint, Severity};
use common::{Ctx, Diagnostics, FmtStr, Span};
use toml::util::Datatype;

pub trait CargoCtx:
    Ctx<Error = Self::CargoError, Warning = Self::CargoWarning, Info = Self::CargoInfo>
{
    type CargoError: From<Error>;
    type CargoWarning: From<Warning>;
    type CargoInfo: From<Info>;
}

impl<E, W, I> CargoCtx for Diagnostics<E, W, I>
where
    E: From<Error>,
    W: From<Warning>,
    I: From<Info>,
{
    type CargoError = E;
    type CargoWarning = W;
    type CargoInfo = I;
}

#[derive(Debug, PartialEq, Eq)]
pub struct Error {
    lines: Box<[u32]>,
    path: FmtStr,
    span: Span,
    kind: ErrorKind,
}

impl Error {
    pub fn new(lines: Box<[u32]>, path: FmtStr, span: Span, kind: ErrorKind) -> Self {
        Self {
            lines,
            path,
            span,
            kind,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum ErrorKind {
    WrongDatatype {
        expected: Datatype,
        found: Datatype,
    },
    /// In the 2024 edition keys with underscores are unsupported.
    UnsupportedUnderscore {
        old: &'static str,
        new: &'static str,
    },
    DepWrongDatatype(Datatype),
    DepWorkspaceIsFalse,
    AmbigousDepSpecGitPath,
    AmbigousDepSpecGitRegistry,
    AmbigousGitSpec,
    /// Invalid in the 2024 edition.
    MissingDepSpec,
    DepIgnoredGitKey(&'static str),
}

impl Diagnostic for Error {
    type Hint = Hint;

    const SEVERITY: Severity = Severity::Error;

    fn span(&self) -> Span {
        self.span
    }

    fn description(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        use ErrorKind::*;
        let Self { path, kind, .. } = self;
        match kind {
            WrongDatatype { expected, found } => write!(f, "Expected `{path}` to be of type {expected}, found {found}"),
            UnsupportedUnderscore { old, new } => {
                if !path.is_empty() {
                    write!(f, "`{path}`: ")?;
                }
                write!(f, "`{old}` has been replaced with `{new}` and is unsupported in the 2024 edition")
            }
            DepWrongDatatype(found) => write!(f, "Expected `{path}` to be of type string or table, found {found}"),
            DepWorkspaceIsFalse => write!(f, "Invalid dependency specification `{path}`; `workspace` cannot be false"),
            AmbigousDepSpecGitPath => write!(f, "Dependency specification `{path}` is ambigous, only one of `git` or `path` is allowed"),
            AmbigousDepSpecGitRegistry => write!(f, "Dependency specification `{path}` is ambigous, only one of `git` or `registry` is allowed"),
            AmbigousGitSpec => write!(f, "Dependency specification `{path}` is ambigous, only one of `branch`, `tag` or `rev` is allowed"),
            MissingDepSpec => write!(f, "Dependency `{path}` is missing one of `workspace`, `path`, `git` or `version`, this is unsupported in the 2024 edition"),
            DepIgnoredGitKey(key) => write!(f, "Invalid dependency specification `{path}`; `{key}` without `git` is not allowed"),
        }
    }

    fn annotation(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        use ErrorKind::*;
        let Self { kind, .. } = self;
        match kind {
            WrongDatatype { expected, .. } => write!(f, "Expected {expected}"),
            UnsupportedUnderscore { new, .. } => write!(f, "Unsupported; instead use `{new}`"),
            DepWrongDatatype(..) => write!(f, "Expected string or table"),
            DepWorkspaceIsFalse => write!(f, "`workspace` cannot be false"),
            AmbigousDepSpecGitPath => write!(f, "Only one of `git` or `path` is allowed"),
            AmbigousDepSpecGitRegistry => write!(f, "Only one of `git` or `registry` is allowed"),
            AmbigousGitSpec => write!(f, "Only one of `branch`, `tag` or `rev` is allowed"),
            MissingDepSpec => write!(f, "Missing one of `workspace`, `path`, `git` or `version`"),
            DepIgnoredGitKey(_) => write!(f, "Not allowed without `git`"),
        }
    }

    fn context_lines(&self) -> Option<&[u32]> {
        Some(&self.lines)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Warning {
    pub lines: Box<[u32]>,
    pub path: FmtStr,
    pub span: Span,
    pub kind: WarningKind,
}

impl Warning {
    pub fn new(lines: Box<[u32]>, path: FmtStr, span: Span, kind: WarningKind) -> Self {
        Self {
            lines,
            path,
            span,
            kind,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WarningKind {
    /// Warn about future removal in the 2024 edition.
    DeprecatedUnderscore {
        old: &'static str,
        new: &'static str,
    },
    /// Warn about future removal in the 2024 edition.
    RedundantDeprecatedUnderscore {
        old: &'static str,
        new: &'static str,
        new_span: Span,
    },
    MissingDepSpec,
    WorkspaceDepIgnoredKey {
        workspace_span: Span,
    },
    IgnoredUnknownKey,
}

impl Diagnostic for Warning {
    type Hint = Hint;

    const SEVERITY: Severity = Severity::Warning;

    fn span(&self) -> Span {
        self.span
    }

    fn description(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        use WarningKind::*;
        let Self { path, kind, .. } = self;
        match kind {
            DeprecatedUnderscore { old, new } => {
                if !path.is_empty() {
                    write!(f, "`{path}`: ")?;
                }
                write!(f, "`{old}` is deprecated in favor of `{new}` and will be unsupported in the 2024 edition")
            }
            RedundantDeprecatedUnderscore { old, new, .. } => {
                if !path.is_empty() {
                    write!(f, "`{path}`: ")?;
                }
                write!(f, "`{old}` is redundant with `{new}` and will be unsupported in the 2024 edition")
            }
            MissingDepSpec => write!(f, "Missing one of `workspace`, `path`, `git` or `version`, this will be unsupported in the 2024 edition"),
            WorkspaceDepIgnoredKey { .. } => write!(f, "Key `{path}` is ignored, because `workspace` is set"),
            IgnoredUnknownKey => write!(f, "Unknown key `{path}` is ignored"),
        }
    }

    fn annotation(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        use WarningKind::*;
        let Self { kind, .. } = self;
        match kind {
            DeprecatedUnderscore { new, .. } => write!(f, "Deprecated in favor of `{new}`"),
            RedundantDeprecatedUnderscore { .. } => write!(f, "Ignored"),
            MissingDepSpec => write!(f, "Missing one of `workspace`, `path`, `git` or `version`"),
            WorkspaceDepIgnoredKey { .. } => write!(f, "Key is ignored"),
            IgnoredUnknownKey => write!(f, "Unknown key is ignored"),
        }
    }

    fn hint(&self) -> Option<Self::Hint> {
        use WarningKind::*;
        let Self { kind, .. } = self;
        match kind {
            DeprecatedUnderscore { .. } => None,
            RedundantDeprecatedUnderscore { new_span, .. } => {
                Some(Hint::RedundantDeprecatedUnderscore(*new_span))
            }
            MissingDepSpec => None,
            WorkspaceDepIgnoredKey { workspace_span } => {
                Some(Hint::WorkspaceDepIgnoredKey(*workspace_span))
            }
            IgnoredUnknownKey => None,
        }
    }

    fn context_lines(&self) -> Option<&[u32]> {
        Some(&self.lines)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Info {}

impl Diagnostic for Info {
    type Hint = Hint;

    const SEVERITY: Severity = Severity::Info;

    fn span(&self) -> Span {
        todo!()
    }

    fn description(&self, _f: &mut impl std::fmt::Write) -> std::fmt::Result {
        todo!()
    }

    fn annotation(&self, _f: &mut impl std::fmt::Write) -> std::fmt::Result {
        todo!()
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Hint {
    RedundantDeprecatedUnderscore(Span),
    WorkspaceDepIgnoredKey(Span),
}

impl DiagnosticHint for Hint {
    fn span(&self) -> Span {
        match self {
            Hint::RedundantDeprecatedUnderscore(s) => *s,
            Hint::WorkspaceDepIgnoredKey(s) => *s,
        }
    }

    fn annotation(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result {
        match self {
            Hint::RedundantDeprecatedUnderscore(_) => write!(f, "Used instead"),
            Hint::WorkspaceDepIgnoredKey(_) => write!(f, "Set here"),
        }
    }
}
