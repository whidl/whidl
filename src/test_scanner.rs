use std::collections::HashMap;
use std::path::PathBuf;
use std::str::Chars;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum TokenType {
    OutputFile,
    CompareTo,
    OutputList,
    Set,
    Identifier,
    BinaryFormatSpecifier,
    DecimalFormatSpecifier,
    HexFormatSpecifier,
    StringFormatSpecifier,
    Number,
    Dot,
    Comma,
    Semicolon,
    Tick,
    Tock,
    Output,
    Eval,
    LeftAngle,
    RightAngle,
    Eof,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub line: u32,
    pub path: PathBuf,
}

pub struct TestScanner<'a> {
    source_chars: std::iter::Peekable<Chars<'a>>,
    pub line: u32,
    keywords: HashMap<&'a str, TokenType>,
    peeked: Option<Token>,
    pub path: PathBuf,
}

impl<'a> TestScanner<'a> {
    pub fn new(source_code: &str, source_path: PathBuf) -> TestScanner {
        let source_chars = source_code.chars().peekable();

        let keywords = HashMap::from([
            ("output-file", TokenType::OutputFile),
            ("compare-to", TokenType::CompareTo),
            ("output-list", TokenType::OutputList),
            ("set", TokenType::Set),
            ("tick", TokenType::Tick),
            ("tock", TokenType::Tock),
            ("output", TokenType::Output),
            ("eval", TokenType::Eval),
        ]);

        TestScanner {
            source_chars,
            line: 1,
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
            token = match self.source_chars.next() {
                None => None,
                Some(c) => match c {
                    ';' => Some(Token {
                        token_type: TokenType::Semicolon,
                        lexeme: c.to_string(),
                        line: self.line,
                        path: self.path.clone(),
                    }),
                    ',' => Some(Token {
                        token_type: TokenType::Comma,
                        lexeme: c.to_string(),
                        line: self.line,
                        path: self.path.clone(),
                    }),
                    '.' => Some(Token {
                        token_type: TokenType::Dot,
                        lexeme: c.to_string(),
                        line: self.line,
                        path: self.path.clone(),
                    }),
                    '<' => Some(Token {
                        token_type: TokenType::LeftAngle,
                        lexeme: c.to_string(),
                        line: self.line,
                        path: self.path.clone(),
                    }),
                    '>' => Some(Token {
                        token_type: TokenType::RightAngle,
                        lexeme: c.to_string(),
                        line: self.line,
                        path: self.path.clone(),
                    }),
                    '%' => {
                        let followup: char = match self.source_chars.peek() {
                            None => panic!(),
                            Some(c) => *c,
                        };
                        let token_type = match followup {
                            'B' => TokenType::BinaryFormatSpecifier,
                            'D' => TokenType::DecimalFormatSpecifier,
                            'X' => TokenType::HexFormatSpecifier,
                            'S' => TokenType::StringFormatSpecifier,
                            _ => panic!(),
                        };
                        self.source_chars.next();
                        let lexeme = String::from("%") + &followup.to_string();
                        Some(Token {
                            token_type,
                            lexeme,
                            line: self.line,
                            path: self.path.clone(),
                        })
                    }
                    '\n' => {
                        self.line += 1;
                        None
                    }
                    ' ' | '\t' | '\r' => None,
                    '/' => {
                        let followup: char = match self.source_chars.peek() {
                            None => panic!(),
                            Some(c2) => *c2,
                        };

                        if followup == '/' {
                            self.finish_single_comment();
                        } else if followup == '*' {
                            self.finish_multi_comment();
                        } else {
                            panic!();
                        }
                        None
                    }
                    _ => {
                        if c.is_alphabetic() {
                            Some(self.finish_identifier(c))
                        } else if c.is_numeric() || c == '-' {
                            Some(self.finish_number(c))
                        } else {
                            panic!("Unexpected character: {}", c)
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
                    break;
                }
                _ => {}
            }
        }
    }

    fn finish_multi_comment(&mut self) {
        loop {
            let next = self.source_chars.next();

            match next {
                None => panic!(),
                Some('\n') => {
                    self.line += 1;
                }
                Some('*') => match self.source_chars.peek() {
                    None => panic!("Unterminated comment."),
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
            } else {
                break;
            }
        }

        Token {
            token_type: TokenType::Number,
            lexeme,
            line: self.line,
            path: self.path.clone(),
        }
    }

    fn finish_identifier(&mut self, start: char) -> Token {
        let mut lexeme = start.to_string();

        while let Some(c) = self.source_chars.peek() {
            if c.is_alphanumeric() || c == &'-' || c == &'.' {
                lexeme.push(*c);
                self.source_chars.next();
            } else {
                break;
            }
        }

        self.lexeme_to_identifier_or_keyword(lexeme, self.line)
    }

    fn lexeme_to_identifier_or_keyword(&self, lexeme: String, line: u32) -> Token {
        let token_type = match self.keywords.get(lexeme.as_str()) {
            None => TokenType::Identifier,
            Some(t) => *t,
        };

        Token {
            token_type,
            lexeme,
            line,
            path: self.path.clone(),
        }
    }
}

impl<'a> Iterator for TestScanner<'a> {
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
    fn test_nand2tetris_solutions_and_tokens() {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let test_file = manifest_dir
            .join("resources")
            .join("tests")
            .join("nand2tetris")
            .join("solutions")
            .join("And.tst");

        // read a file into string
        let contents = fs::read_to_string(test_file).expect("Unable to read test file.");

        let scanner = TestScanner::new(contents.as_str(), PathBuf::from(""));
        let actual_types: Vec<_> = scanner.map(|x| x.token_type).collect();

        let expected_types = vec![
            TokenType::Identifier,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::OutputFile,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::CompareTo,
            TokenType::Identifier,
            TokenType::Comma,
            TokenType::OutputList,
            TokenType::Identifier,
            TokenType::BinaryFormatSpecifier,
            TokenType::Number,
            TokenType::Dot,
            TokenType::Number,
            TokenType::Dot,
            TokenType::Number,
            TokenType::Identifier,
            TokenType::BinaryFormatSpecifier,
            TokenType::Number,
            TokenType::Dot,
            TokenType::Number,
            TokenType::Dot,
            TokenType::Number,
            TokenType::Identifier,
            TokenType::BinaryFormatSpecifier,
            TokenType::Number,
            TokenType::Dot,
            TokenType::Number,
            TokenType::Dot,
            TokenType::Number,
            TokenType::Semicolon,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Eval,
            TokenType::Comma,
            TokenType::Output,
            TokenType::Semicolon,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Eval,
            TokenType::Comma,
            TokenType::Output,
            TokenType::Semicolon,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Eval,
            TokenType::Comma,
            TokenType::Output,
            TokenType::Semicolon,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Set,
            TokenType::Identifier,
            TokenType::Number,
            TokenType::Comma,
            TokenType::Eval,
            TokenType::Comma,
            TokenType::Output,
            TokenType::Semicolon,
        ];

        assert_eq!(expected_types, actual_types);
    }
}
