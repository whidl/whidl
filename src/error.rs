use crate::parser::HdlProvider;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::PathBuf;
use std::rc::Rc;

// Error type enum
pub enum ErrorKind {
    ParseError(crate::scanner::Token),
    ParseIdentError(Rc<dyn HdlProvider>, crate::parser::Identifier),
    TestParseError(crate::test_scanner::Token),
    SimulationError(Option<PathBuf>),
    IOError,
    Other,
    NonNumeric,
}

pub struct N2VError {
    pub msg: String,
    pub kind: ErrorKind,
}

impl std::fmt::Debug for N2VError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.msg)
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
                        return writeln!(f, "Error: {}", self.msg);
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
                    return writeln!(f, "Error 1: {}", self.msg);
                }
                if ident.line.is_none() {
                    return writeln!(f, "Error 2: {}", self.msg);
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
                        return writeln!(f, "Error 3: {}", self.msg);
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
                writeln!(f, "Error 4: {}", self.msg)
            }
        }
    }
}

impl From<std::io::Error> for N2VError {
    fn from(e: std::io::Error) -> Self {
        N2VError {
            msg: format!("IO Error: {}", e),
            kind: ErrorKind::IOError,
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
