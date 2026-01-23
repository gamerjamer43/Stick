use super::error::SyntaxError;
use ariadne::{Label, Report, ReportKind, Source};
use std::{
    fmt::{Display, Formatter, Result},
    fs::File,
    io::{self, BufWriter, Write},
    ops::Range,
};
use strip_ansi_escapes::strip;

/// a structured way to print diagnostics. probably not struct required but is clean. will use for both lex and parse error likely
/// - path = the file path, displayed in the error message
/// - src = the source file, to scan for the error message
/// - span = the range of chars the error lies in
/// - err = the accompanying SyntaxError
///
/// - <'a> the lifetime of this Diagnostic
/// - <'src> the lifetime of the source file
pub struct Diagnostic<'a, 'src> {
    pub path: &'a str,
    pub src: &'src str,
    pub span: Range<usize>,
    pub err: SyntaxError<'src>,
}

// so much fucking cleaner saves me a lot of pain
impl<'a, 'src> Display for Diagnostic<'a, 'src> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut buf: Vec<u8> = Vec::new();

        // main report with a short, human-friendly header
        Report::build(ReportKind::Error, self.path, self.span.start)
            .with_message(format!("lex error: {}", self.err))
            // points to what's fucked up
            .with_label(Label::new((self.path, self.span.clone())).with_message("error here"))
            // lexer help (doing different display in the parser, as i will need notes)
            .with_help(
                "lexer errors are only caused by things that would cause issues in tokenization.",
            ) // short hint
            .finish()
            .write((self.path, Source::from(self.src)), &mut buf)
            .unwrap();

        // moo
        write!(f, "{}", String::from_utf8_lossy(&buf))
    }
}

// dump any found errors
pub fn dump(errors: &[Diagnostic<'_, '_>], path: &str) -> io::Result<()> {
    let file: File = File::create(path)?;
    let mut writer: BufWriter<File> = BufWriter::new(file);

    for diag in errors {
        // strip ANSI escape sequences
        let stripped: Vec<u8> = strip(diag.to_string().as_bytes());

        // write clean UTF-8 (lossy is fine for logs)
        writeln!(writer, "{}", String::from_utf8_lossy(&stripped))?;
    }

    writer.flush()
}
