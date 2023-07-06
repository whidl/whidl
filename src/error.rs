use crate::parser::HdlProvider;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::rc::Rc;

// Error type enum
#[derive(Clone)]
pub enum ErrorKind {
    ParseError(crate::scanner::Token),
    ParseIdentError(Rc<dyn HdlProvider>, crate::parser::Identifier),
    TestParseError(crate::test_scanner::Token),
    SimulationError(Option<PathBuf>),
    IOError,
    Other,
    NonNumeric,
}

/// N2VError should be used when generating an error that has no other
/// source error object. This is the start of the error propagation chain.
pub struct N2VError {
    pub msg: String,
    pub kind: ErrorKind,
}

/// Transformed errors should be used when the source of the error is
/// another error. This is propagating an error with a new message.
pub struct TransformedError {
    pub msg: String,
    pub kind: ErrorKind,
    pub source: Option<Box<dyn Error + 'static>>,
}

impl std::fmt::Debug for N2VError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::fmt::Debug for TransformedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

/// This relies on the N2VError display implementation. It will print
/// the entire error chain.
impl std::fmt::Display for TransformedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.source.is_some() {
            write!(f, "{}", N2VError::from(self))?;
            let error_source = &**(self.source.as_ref().unwrap());
            write!(f, "{}", error_source)
        } else {
            write!(f, "{}", N2VError::from(self))
        }
    }
}

/// Strips the source from a transformed error. This is used to
/// display a TransformedError.
impl From<&TransformedError> for N2VError {
    fn from(e: &TransformedError) -> Self {
        N2VError {
            msg: e.msg.clone(),
            kind: e.kind.clone(),
        }
    }
}

impl std::fmt::Display for N2VError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        #[allow(unused_must_use)]
        match &self.kind {
            ErrorKind::ParseError(t) => {
                let file = match File::open(t.path.clone()) {
                    Ok(x) => x,
                    Err(_) => {
                        writeln!(f, "In : {:?}", t.path.clone());
                        return writeln!(f, "{}", self.msg);
                    }
                };

                let n2 = t.line;
                let line_num: usize = n2.try_into().unwrap();
                let l = io::BufReader::new(file).lines().nth(line_num - 1);
                if l.is_none() {
                    writeln!(f, "-- PARSE ERROR ----------- {}", t.path.clone().display());
                    writeln!(f, "{}|", t.line);
                    return writeln!(f, "\n\n{}", self.msg);
                }

                let l = l.unwrap().unwrap();
                let col = t.start;
                let digits = line_num.to_string();

                writeln!(f, "-- PARSE ERROR ----------- {}", t.path.clone().display());
                writeln!(f, "{}| {}", t.line, l);
                for _ in 0..(col + digits.len() + 2 - t.lexeme.len()) {
                    write!(f, " ");
                }
                for _ in 0..t.lexeme.len() {
                    write!(f, "^");
                }
                writeln!(f, "\n\n{}", self.msg)
            }
            ErrorKind::ParseIdentError(provider, ident) => {
                if ident.path.is_none() {
                    return writeln!(f, "{}", self.msg);
                }
                if ident.line.is_none() {
                    return writeln!(f, "{}", self.msg);
                }

                if ident.path.is_none() {
                    return writeln!(f, "Bad path: {}", self.msg);
                }

                if ident.path.as_ref().unwrap().file_name().is_none() {
                    return writeln!(f, "Bad filename: {}", self.msg);
                }

                if ident.path.is_none() {
                    return writeln!(f, "Bad path: {}", self.msg);
                }

                if ident.path.as_ref().unwrap().file_name().is_none() {
                    return writeln!(f, "Bad filename: {}", self.msg);
                }

                let hdl = match provider.get_hdl(
                    ident
                        .path
                        .as_ref()
                        .unwrap()
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap(),
                ) {
                    Ok(x) => x,
                    Err(e) => {
                        writeln!(f, "{:?}", e);
                        return writeln!(f, "{}", self.msg);
                    }
                };

                let n2 = *ident.line.as_ref().unwrap();
                let line_num: usize = n2.try_into().unwrap();
                let l = hdl.lines().nth(line_num - 1).unwrap();

                writeln!(
                    f,
                    "-- PARSE ERROR ----------- {}",
                    ident.path.as_ref().unwrap().clone().display()
                );
                writeln!(f, "{}| {}", line_num, l);
                writeln!(f, "\n\n{}", self.msg)
            }
            _ => {
                writeln!(f, "{}", self.msg)
            }
        }
    }
}

impl From<String> for N2VError {
    fn from(e: String) -> Self {
        N2VError {
            msg: e,
            kind: ErrorKind::Other,
        }
    }
}

impl Error for N2VError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        None
    }
}

impl Error for TransformedError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.source {
            None => None,
            // We store the source as an Box to a trait object.
            // Dereference once to get the Box behind the self reference,
            // and dereference the second time to get the inner trait object.
            // We then return a reference to that inner trait object.
            // This will be static lieftime because we return boxed dyn errors
            // as our result error type everywhere.
            Some(e) => Some(&**e),
        }
    }
}
