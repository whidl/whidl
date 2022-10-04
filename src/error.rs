use std::path::PathBuf;
use std::fs::File;
use std::io::{self, BufRead};

// Error type enum
pub enum ErrorKind {
    ParseError(crate::scanner::Token),
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
                let file = File::open(t.path.clone()).unwrap();
                let n2 = t.line;
                let line_num : usize = n2.try_into().unwrap();
                let l = io::BufReader::new(file).lines().nth(line_num - 1).unwrap().unwrap();
                let col = t.start;
                let digits = line_num.to_string();

                writeln!(f, "-- PARSE ERROR ----------- {}", t.path.clone().display());
                writeln!(f, "{}| {}", t.line, l);
                for _ in 0..col + digits.len() + 1 {
                    write!(f, " ");
                }
                for _ in 0..t.lexeme.len() {
                    write!(f, "^");
                }
                writeln!(f, "\n\n{}", self.msg)
            }
            _ => {
                panic!();
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
