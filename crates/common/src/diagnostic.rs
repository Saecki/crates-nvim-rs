use unicode_width::UnicodeWidthStr;

use crate::Span;

pub fn lines(input: &str) -> Vec<&str> {
    let mut lines = input.split('\n').collect::<Vec<_>>();
    if let [terminated_lines @ .., _] = lines.as_mut_slice() {
        for l in terminated_lines {
            if l.ends_with('\r') {
                *l = &l[..l.len() - 1];
            }
        }
    }
    lines
}

pub fn cmp<D: Diagnostic>(a: &D, b: &D) -> std::cmp::Ordering {
    span_cmp(a.span(), b.span())
}

pub fn span_cmp(a: Span, b: Span) -> std::cmp::Ordering {
    a.start.cmp(&b.start)
}

pub trait Diagnostic {
    type Hint: DiagnosticHint;

    const SEVERITY: Severity;

    fn span(&self) -> Span;
    fn description(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result;
    fn annotation(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result;

    fn hint(&self) -> Option<Self::Hint> {
        None
    }

    fn lines(&self) -> Option<&[u32]> {
        None
    }
}

pub trait DiagnosticHint {
    fn span(&self) -> Span;
    fn annotation(&self, f: &mut impl std::fmt::Write) -> std::fmt::Result;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
    Hint,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => f.write_str("error"),
            Severity::Warning => f.write_str("warning"),
            Severity::Info => f.write_str("info"),
            Severity::Hint => f.write_str("hint"),
        }
    }
}

pub fn display(
    f: &mut impl std::fmt::Write,
    diagnostic: &impl Diagnostic,
    lines: &[&str],
) -> std::fmt::Result {
    writeln!(f, "{}", diagnostic.header(&lines))?;
    let context_lines = diagnostic.lines().unwrap_or(&[]);

    let mut start_line = 0;
    if let Some(hint) = diagnostic.hint() {
        let span = hint.span();
        for &l in context_lines {
            if l < span.start.line {
                display_line(f, l as usize, &lines[l as usize])?;
            } else {
                break;
            }
        }
        write!(f, "{}", hint.body(&lines))?;
        start_line = span.end.line + 1;
    }

    let span = diagnostic.span();
    for &l in context_lines {
        if l < start_line {
            continue;
        }
        if l < span.start.line {
            display_line(f, l as usize, &lines[l as usize])?;
        } else {
            break;
        }
    }
    write!(f, "{}", diagnostic.body(&lines))?;
    Ok(())
}

pub trait DisplayDiagnosticHeader: Diagnostic + Sized {
    fn header<'a, T>(&'a self, text: &'a [T]) -> DiagnosticHeader<'a, Self, T>;
}

impl<D: Diagnostic> DisplayDiagnosticHeader for D {
    fn header<'a, T>(&'a self, text: &'a [T]) -> DiagnosticHeader<'a, D, T> {
        DiagnosticHeader {
            diagnostic: self,
            text,
        }
    }
}

pub struct DiagnosticHeader<'a, D: Diagnostic, T> {
    diagnostic: &'a D,
    text: &'a [T],
}

impl<'a, D: Diagnostic, T: AsRef<str>> std::fmt::Display for DiagnosticHeader<'a, D, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        display_header(f, self.diagnostic, self.text)
    }
}

fn display_header<D: Diagnostic>(
    f: &mut impl std::fmt::Write,
    diagnostic: &D,
    text: &[impl AsRef<str>],
) -> std::fmt::Result {
    let severity = D::SEVERITY;
    let color = ansii_esc_color(severity);
    write!(f, "{color}{severity}{ANSII_CLEAR}: ")?;
    diagnostic.description(f)?;
    f.write_char('\n')?;
    let pos = diagnostic.span().start;
    let line_nr = pos.line + 1;
    let char = text[pos.line as usize].as_ref()[0..pos.char as usize]
        .chars()
        .count();
    write!(f, " {ANSII_COLOR_BLUE}-->{ANSII_CLEAR} {line_nr}:{char}")
}

pub trait DisplayDiagnosticBody: Diagnostic + Sized {
    fn body<'a, T>(&'a self, text: &'a [T]) -> DiagnosticBody<'a, Self, T>;
}

impl<D: Diagnostic> DisplayDiagnosticBody for D {
    fn body<'a, T>(&'a self, text: &'a [T]) -> DiagnosticBody<'a, D, T> {
        DiagnosticBody {
            diagnostic: self,
            text,
        }
    }
}

pub struct DiagnosticBody<'a, D: Diagnostic, T> {
    diagnostic: &'a D,
    text: &'a [T],
}

pub trait DisplayDiagnosticHintBody: DiagnosticHint + Sized {
    fn body<'a, T>(&'a self, text: &'a [T]) -> DiagnosticHintBody<'a, Self, T>;
}

impl<D: DiagnosticHint> DisplayDiagnosticHintBody for D {
    fn body<'a, T>(&'a self, text: &'a [T]) -> DiagnosticHintBody<'a, D, T> {
        DiagnosticHintBody {
            diagnostic: self,
            text,
        }
    }
}

pub struct DiagnosticHintBody<'a, D: DiagnosticHint, T> {
    diagnostic: &'a D,
    text: &'a [T],
}

impl<'a, D: Diagnostic, T: AsRef<str> + 'a> std::fmt::Display for DiagnosticBody<'a, D, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        display_body(
            f,
            |f| self.diagnostic.annotation(f),
            D::SEVERITY,
            self.diagnostic.span(),
            self.text,
        )
    }
}

impl<'a, D: DiagnosticHint, T: AsRef<str> + 'a> std::fmt::Display for DiagnosticHintBody<'a, D, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        display_body(
            f,
            |f| self.diagnostic.annotation(f),
            Severity::Hint,
            self.diagnostic.span(),
            self.text,
        )
    }
}

fn display_body<F: std::fmt::Write>(
    f: &mut F,
    annotation: impl Fn(&mut F) -> std::fmt::Result,
    severity: Severity,
    span: Span,
    text: &[impl AsRef<str>],
) -> std::fmt::Result {
    let start_line = span.start.line as usize;
    let end_line = span.end.line as usize + 1;
    let num_lines = end_line - start_line;
    let color = ansii_esc_color(severity);

    for (i, line) in text[start_line..end_line].iter().enumerate() {
        let line_nr = start_line + i;
        let line = line.as_ref();
        display_line(f, line_nr, line)?;

        let col_start = if i == 0 { span.start.char as usize } else { 0 };
        let col_end = if i == num_lines - 1 {
            span.end.char as usize
        } else {
            line.len()
        };
        let num_spaces = line[0..col_start].width();
        write!(
            f,
            "     {ANSII_COLOR_BLUE}|{ANSII_CLEAR} {:num_spaces$}",
            ""
        )?;

        let spanned_text = &line[col_start..col_end];
        let num_carets = spanned_text.width().max(1);
        write!(f, "{color}{:^<num_carets$}", "")?;
        if i == num_lines - 1 {
            f.write_char(' ')?;
            annotation(f)?;
        }
        writeln!(f, "{ANSII_CLEAR}")?;
    }

    Ok(())
}

/// `line_nr` is 0-based
pub fn display_line(f: &mut impl std::fmt::Write, line_nr: usize, line: &str) -> std::fmt::Result {
    let line_nr = line_nr + 1;
    write!(f, "{ANSII_COLOR_BLUE}{line_nr:4} |{ANSII_CLEAR} ")?;

    let mut next_start = 0;
    for (j, c) in line.char_indices() {
        match c {
            '\t' => (),
            // backspace
            '\x00'..='\x1f' | '\x7f' => {
                f.write_str(&line[next_start..j])?;
                next_start = j + 1;
            }
            _ => (),
        }
    }
    f.write_str(&line[next_start..])?;
    f.write_char('\n')?;
    Ok(())
}

const ANSII_CLEAR: &str = "\x1b[0m";
const ANSII_COLOR_RED: &str = "\x1b[91m";
const ANSII_COLOR_YELLOW: &str = "\x1b[93m";
const ANSII_COLOR_BLUE: &str = "\x1b[94m";
const ANSII_COLOR_CYAN: &str = "\x1b[96m";

fn ansii_esc_color(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => ANSII_COLOR_RED,
        Severity::Warning => ANSII_COLOR_YELLOW,
        Severity::Info => ANSII_COLOR_CYAN,
        Severity::Hint => ANSII_COLOR_BLUE,
    }
}
