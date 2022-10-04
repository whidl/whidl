use std::collections::HashMap;
use std::path::PathBuf;
use std::str::Chars;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum TokenType {
    Chip,
    Identifier,
    LeftCurly,
    RightCurly,
    LeftBracket,
    RightBracket,
    LeftParen,
    RightParen,
    LeftAngle,
    RightAngle,
    Semicolon,
    Colon,
    In,
    Out,
    Comma,
    Parts,
    Number,
    Equal,
    Dot,
    Invalid,
    For,
    To,
    Generate,
    Plus,
    Minus,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub line: u32,
    pub start: usize,
    pub path: PathBuf,
}

pub struct Scanner<'a> {
    source_chars: std::iter::Peekable<Chars<'a>>,
    pub line: u32,
    pub col: usize,
    keywords: HashMap<&'a str, TokenType>,
    peeked: Option<Token>,
    pub path: PathBuf,
}

impl<'a> Scanner<'a> {
    pub fn new(source_code: &str, source_path: PathBuf) -> Scanner {
        let source_chars = source_code.chars().peekable();

        // Keywords are case-insensitive
        let keywords = HashMap::from([
            ("CHIP", TokenType::Chip),
            ("PARTS", TokenType::Parts),
            ("IN", TokenType::In),
            ("OUT", TokenType::Out),
            ("FOR", TokenType::For),
            ("IN", TokenType::In),
            ("TO", TokenType::To),
            ("GENERATE", TokenType::Generate),
        ]);

        Scanner {
            source_chars,
            line: 1,
            col: 1,
            keywords,
            peeked: None,
            path: source_path,
        }
    }

    pub fn peek(&mut self) -> Option<Token> {
        let token = self.scan_token();

        match token {
            None => None,
            Some(t) => {
                self.peeked = Some(t);
                self.peeked.clone()
            }
        }
    }

    pub fn scan_token(&mut self) -> Option<Token> {
        let mut token: Option<Token> = match self.peeked.clone() {
            None => None,
            Some(t) => {
                self.peeked = None;
                Some(t)
            }
        };

        while token.is_none() && self.source_chars.peek().is_some() {
            self.col += 1;
            token = match self.source_chars.next() {
                None => None,
                Some(c) => match c {
                    '{' => Some(Token {
                        token_type: TokenType::LeftCurly,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '}' => Some(Token {
                        token_type: TokenType::RightCurly,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '(' => Some(Token {
                        token_type: TokenType::LeftParen,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    ')' => Some(Token {
                        token_type: TokenType::RightParen,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    ';' => Some(Token {
                        token_type: TokenType::Semicolon,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    ':' => Some(Token {
                        token_type: TokenType::Colon,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    ',' => Some(Token {
                        token_type: TokenType::Comma,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '[' => Some(Token {
                        token_type: TokenType::LeftBracket,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    ']' => Some(Token {
                        token_type: TokenType::RightBracket,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '<' => Some(Token {
                        token_type: TokenType::LeftAngle,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '>' => Some(Token {
                        token_type: TokenType::RightAngle,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '=' => Some(Token {
                        token_type: TokenType::Equal,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '.' => Some(Token {
                        token_type: TokenType::Dot,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '+' => Some(Token {
                        token_type: TokenType::Plus,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '-' => Some(Token {
                        token_type: TokenType::Minus,
                        lexeme: c.to_string(),
                        line: self.line,
                        start: self.col,
                        path: self.path.clone(),
                    }),
                    '\n' => {
                        self.line += 1;
                        self.col = 0;
                        None
                    }
                    ' ' | '\t' | '\r' => None,
                    '/' => {
                        let followup: char = match self.source_chars.peek() {
                            None => {
                                return Some(Token {
                                    lexeme: c.to_string(),
                                    line: self.line,
                                    start: self.col,
                                    path: self.path.clone(),
                                    token_type: TokenType::Invalid,
                                });
                            }
                            Some(c2) => *c2,
                        };

                        if followup == '/' {
                            self.finish_single_comment();
                        } else if followup == '*' {
                            self.finish_multi_comment();
                        } else {
                            return Some(Token {
                                lexeme: c.to_string(),
                                line: self.line,
                                start: self.col,
                                path: self.path.clone(),
                                token_type: TokenType::Invalid,
                            });
                        }
                        None
                    }
                    _ => {
                        if c.is_alphabetic() || c == '_' {
                            Some(self.finish_identifier(c))
                        } else if c.is_numeric() {
                            Some(self.finish_number(c))
                        } else {
                            Some(Token {
                                lexeme: c.to_string(),
                                line: self.line,
                                start: self.col,
                                path: self.path.clone(),
                                token_type: TokenType::Invalid,
                            })
                        }
                    }
                },
            }
        }
        token
    }

    fn finish_single_comment(&mut self) {
        loop {
            let next = self.source_chars.next();
            match next {
                None => break,
                Some('\n') => {
                    self.line += 1;
                    self.col = 0;
                    break;
                }
                _ => {}
            }
        }
    }

    fn finish_multi_comment(&mut self) {
        loop {
            let next = self.source_chars.next();
            self.col += 1;

            match next {
                None => {
                    break;
                }
                Some('\n') => {
                    self.line += 1;
                    self.col = 0;
                }
                Some('*') => match self.source_chars.peek() {
                    None => {
                        break;
                    }
                    Some('/') => {
                        self.source_chars.next();
                        break;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn finish_number(&mut self, start: char) -> Token {
        let mut lexeme = start.to_string();

        while let Some(c) = self.source_chars.peek() {
            if c.is_numeric() {
                lexeme.push(*c);
                self.source_chars.next();
                self.col += 1;
            } else {
                break;
            }
        }

        Token {
            token_type: TokenType::Number,
            lexeme,
            line: self.line,
            start: self.col,
            path: self.path.clone(),
        }
    }

    fn finish_identifier(&mut self, start: char) -> Token {
        let mut lexeme = start.to_string();

        while let Some(c) = self.source_chars.peek() {
            if c.is_alphanumeric() || c == &'_' {
                lexeme.push(*c);
                self.source_chars.next();
                self.col += 1;
            } else {
                break;
            }
        }

        self.lexeme_to_identifier_or_keyword(lexeme, self.line, self.col)
    }

    fn lexeme_to_identifier_or_keyword(&self, lexeme: String, line: u32, col: usize) -> Token {
        let token_type = match self.keywords.get(lexeme.as_str()) {
            None => TokenType::Identifier,
            Some(t) => *t,
        };

        Token {
            token_type,
            lexeme,
            line,
            start: col,
            path: self.path.clone(),
        }
    }
}

impl<'a> Iterator for Scanner<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        self.scan_token()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    use std::env;
    use std::fs;
    use std::path::Path;

    #[test]
    fn test_nand2tetris_original_and_tokens() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let test_file = manifest_dir
            .join("resources")
            .join("tests")
            .join("nand2tetris")
            .join("original")
            .join("01")
            .join("And.hdl");

        // read a file into string
        let contents = fs::read_to_string(test_file).expect("Unable to read test file.");

        let scanner = Scanner::new(contents.as_str(), PathBuf::from(""));
        let actual_types: Vec<_> = scanner.map(|x| x.token_type).collect();

        let expected_types = vec![
            TokenType::Chip,
            TokenType::Identifier,
            TokenType::LeftCurly,
            TokenType::In,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Semicolon,
            TokenType::Out,
            TokenType::Identifier,
            TokenType::Semicolon,
            TokenType::Parts,
            TokenType::Colon,
            TokenType::RightCurly,
        ];

        assert_eq!(expected_types, actual_types);
    }

    #[test]
    fn test_nand2tetris_original_and16_tokens() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let test_file = manifest_dir
            .join("resources")
            .join("tests")
            .join("nand2tetris")
            .join("original")
            .join("01")
            .join("And16.hdl");

        // read a file into string
        let contents = fs::read_to_string(test_file).expect("Unable to read test file.");

        let scanner = Scanner::new(contents.as_str(), PathBuf::from(""));
        let actual_types: Vec<_> = scanner.map(|x| x.token_type).collect();

        let expected_types = vec![
            TokenType::Chip,
            TokenType::Identifier,
            TokenType::LeftCurly,
            TokenType::In,
            TokenType::Identifier,
            TokenType::LeftBracket,
            TokenType::Number,
            TokenType::RightBracket,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::LeftBracket,
            TokenType::Number,
            TokenType::RightBracket,
            TokenType::Semicolon,
            TokenType::Out,
            TokenType::Identifier,
            TokenType::LeftBracket,
            TokenType::Number,
            TokenType::RightBracket,
            TokenType::Semicolon,
            TokenType::Parts,
            TokenType::Colon,
            TokenType::RightCurly,
        ];

        assert_eq!(expected_types, actual_types);
    }

    #[test]
    fn test_nand2tetris_original_dmux() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let test_file = manifest_dir
            .join("resources")
            .join("tests")
            .join("nand2tetris")
            .join("original")
            .join("01")
            .join("DMux.hdl");

        // read a file into string
        let contents = fs::read_to_string(test_file).expect("Unable to read test file.");

        let scanner = Scanner::new(contents.as_str(), PathBuf::from(""));
        let actual_types: Vec<_> = scanner.map(|x| x.token_type).collect();

        let expected_types = vec![
            TokenType::Chip,
            TokenType::Identifier,
            TokenType::LeftCurly,
            TokenType::In,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Semicolon,
            TokenType::Out,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Semicolon,
            TokenType::Parts,
            TokenType::Colon,
            TokenType::RightCurly,
        ];

        assert_eq!(expected_types, actual_types);
    }

    #[test]
    fn test_nand2tetris_solutions_mux() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let test_file = manifest_dir
            .join("resources")
            .join("tests")
            .join("nand2tetris")
            .join("solutions")
            .join("Mux.hdl");

        // read a file into string
        let contents = fs::read_to_string(test_file).expect("Unable to read test file.");

        let scanner = Scanner::new(contents.as_str(), PathBuf::from(""));
        let actual_types: Vec<_> = scanner.map(|x| x.token_type).collect();

        let expected_types = vec![
            TokenType::Chip,
            TokenType::Identifier,
            TokenType::LeftCurly,
            TokenType::In,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Semicolon,
            TokenType::Out,
            TokenType::Identifier,
            TokenType::Semicolon,
            TokenType::Parts,
            TokenType::Colon,
            TokenType::Identifier,
            TokenType::LeftParen,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::RightParen,
            TokenType::Semicolon,
            TokenType::Identifier,
            TokenType::LeftParen,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::RightParen,
            TokenType::Semicolon,
            TokenType::Identifier,
            TokenType::LeftParen,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::RightParen,
            TokenType::Semicolon,
            TokenType::Identifier,
            TokenType::LeftParen,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::Identifier,
            TokenType::Equal,
            TokenType::Identifier,
            TokenType::RightParen,
            TokenType::Semicolon,
            TokenType::RightCurly,
        ];

        assert_eq!(expected_types, actual_types);
    }
}
