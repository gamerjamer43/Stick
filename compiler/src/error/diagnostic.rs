use super::{ParseError, SemanticError, SyntaxError};
use crate::error::errors::LexError;
use ariadne::{Color, Config, Label, LabelAttach, Report, ReportKind, Source};
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

// hacky way to avoid defining names for every type
impl<'src> SyntaxError<'src> {
    pub fn title(&self) -> &str {
        match self {
            SyntaxError::Lex(e) => e.as_ref(),
            SyntaxError::Parse(e) => e.as_ref(),
            SyntaxError::Semantic(e) => e.as_ref(),
            SyntaxError::Unknown => "Unknown",
        }
    }

    pub fn label(&self) -> String {
        match self {
            SyntaxError::Lex(LexError::UnterminatedString(_)) => {
                "string literal starts here but never closes".into()
            }
            SyntaxError::Lex(LexError::UnterminatedChar(_)) => {
                "character literal starts here but never closes".into()
            }
            SyntaxError::Lex(LexError::UnknownToken(tok)) => format!("unexpected token `{tok}`"),
            SyntaxError::Parse(ParseError::MissingExpected(msg)) => {
                if msg.starts_with("expected expression") {
                    "expected an expression here".into()
                } else {
                    "required syntax is missing here".into()
                }
            }
            SyntaxError::Parse(ParseError::ConstDisallowed(_)) => {
                "`const` is not allowed here".into()
            }
            SyntaxError::Semantic(SemanticError::TypeInference(_)) => {
                "type cannot be inferred from this declaration".into()
            }
            SyntaxError::Semantic(SemanticError::TypeMismatch(_)) => {
                "this value does not match the target type".into()
            }
            SyntaxError::Semantic(SemanticError::UnknownIdentifier(name)) => {
                format!("`{name}` is not declared in this scope")
            }
            SyntaxError::Semantic(SemanticError::ImmutableBinding(name)) => {
                format!("`{name}` is immutable")
            }
            SyntaxError::Semantic(SemanticError::InvalidOperation(_)) => {
                "this operation is not valid for the selected value(s)".into()
            }
            SyntaxError::Semantic(SemanticError::Overflow(_)) => {
                "this constant does not fit in the target type".into()
            }
            SyntaxError::Unknown => "problematic code is here".into(),
        }
    }

    pub fn help(&self) -> &str {
        match self {
            SyntaxError::Lex(_) => {
                "lexer errors are only caused by things that would cause issues in tokenization."
            }

            // every way i look at flattening this just gets more gross
            SyntaxError::Parse(e) => match e {
                ParseError::MissingExpected(msg) => {
                    if msg.starts_with("let must have") {
                        "if you want to discard the value, use _, otherwise attach a name"
                    } else if msg.starts_with("type cannot be") {
                        "either declare the type beforehand, or add a right hand side and let the compiler infer it."
                    } else if msg.starts_with("all statements must") {
                        "either stick them on seperate lines, or seperate them using a semicolon (bad practice, SHAME!)"
                    } else if msg.starts_with("expected expression") {
                        "the right hand of an equals sign cannot be blank"
                    }
                    // else if msg.starts_with("message start") {
                    //     "special hint wao"
                    // }
                    else {
                        "Unknown"
                    }
                }

                ParseError::ConstDisallowed(msg) => {
                    if msg.ends_with("mutable") {
                        "either remove the mutable tag, or denote it static (placing it in a constant memory location)"
                    } else if msg.ends_with("static") {
                        "remove either const or static. const is a fixed constant, whereas static is constant memory location. constant handles both"
                    } else {
                        "Unknown"
                    }
                }
            },

            SyntaxError::Semantic(e) => match e {
                SemanticError::TypeInference(_) => {
                    "for inferred types: assign a value for type deduction to work, otherwise specify an explicit type"
                }
                SemanticError::TypeMismatch(_) => {
                    "initializer/result type is incompatible with the declared or expected type"
                }
                SemanticError::UnknownIdentifier(_) => {
                    "declare the identifier before use, or fix the identifier name"
                }
                SemanticError::ImmutableBinding(_) => {
                    "mark the declaration mutable before reassigning it, or stop mutating the binding"
                }
                SemanticError::InvalidOperation(_) => {
                    "check operator and operand types, not all operators can apply to all types."
                }
                SemanticError::Overflow(_) => {
                    "reduce the constant value, widen the destination type, or make the value explicit in a larger type first"
                }
            },

            SyntaxError::Unknown => "Only god can save you (or reading the docs lmao.)",
        }
    }
}

// so much fucking cleaner saves me a lot of pain
impl<'a, 'src> Display for Diagnostic<'a, 'src> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut buf: Vec<u8> = Vec::new();

        let config = Config::default()
            .with_label_attach(LabelAttach::Start);

        Report::build(
            ReportKind::Custom(self.err.title(), Color::Red),
            self.path,
            self.span.start,
        )
        .with_config(config)
        .with_message(&self.err)
        // points to what's fucked up
        .with_label(
            Label::new((self.path, self.span.clone()))
                .with_color(Color::Red)
                .with_message(self.err.label()),
        )
        // lexer help (doing different display in the parser, as i will need notes)
        .with_help(self.err.help()) // short hint
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

    for (idx, diag) in errors.iter().enumerate() {
        if idx > 0 {
            writeln!(writer)?;
        }

        // strip ANSI escape sequences
        let stripped: Vec<u8> = strip(diag.to_string().as_bytes());

        // write clean UTF-8 (lossy is fine for logs)
        writeln!(writer, "{}", String::from_utf8_lossy(&stripped))?;
    }

    writer.flush()
}
