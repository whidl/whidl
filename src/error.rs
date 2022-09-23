use std::path::PathBuf;

pub struct N2VError {
    pub msg: String,
    pub path: Option<PathBuf>,
    pub line: Option<u32>,
}

impl std::fmt::Debug for N2VError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?}:{:?} Failed parsing, {}",
            self.path, self.line, self.msg
        )
    }
}

impl From<std::io::Error> for N2VError {
    fn from(e: std::io::Error) -> Self {
        N2VError {
            msg: format!("IO Error: {}", e),
            path: None,
            line: None,
        }
    }
}

impl From<String> for N2VError {
    fn from(e: String) -> Self {
        N2VError {
            msg: e,
            path: None,
            line: None,
        }
    }
}
