use crate::error::{ErrorKind, N2VError};
use crate::test_scanner::{TestScanner, Token, TokenType};
use std::path::PathBuf;

/// The Parse Tree for an HDL Chip.
///
#[derive(Clone)]
pub struct TestScript {
    pub path: Option<PathBuf>,
    pub hdl_file: PathBuf,
    pub output_file: PathBuf,
    pub compare_file: PathBuf,
    pub output_list: Vec<OutputFormat>,
    pub steps: Vec<Step>,
    pub generics: Vec<usize>,
}

#[derive(Clone)]
pub enum Instruction {
    Set(String, InputValue), // (port name, port value)
    Eval,
    Output,
    Tick,
    Tock,
}

#[derive(Clone)]
pub struct InputValue {
    pub number_system: NumberSystem,
    pub value: String,
}

#[derive(Clone)]
pub struct Step {
    pub instructions: Vec<Instruction>,
}

#[derive(Clone)]
pub struct OutputFormat {
    pub port_name: String,
    pub number_system: NumberSystem,
    pub space_before: usize,
    pub output_columns: usize,
    pub space_after: usize,
}

#[derive(Clone, Eq, PartialEq)]
pub enum NumberSystem {
    Binary,
    Decimal,
    Hex,
    String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub value: String,
    pub path: PathBuf, // Set to None if chip not read from disk, e.g. NAND and DFF.
    pub line: u32,
}

impl From<Token> for Identifier {
    fn from(t: Token) -> Self {
        if t.token_type != TokenType::Identifier {
            panic!("Attempt to create Identifier from non-identifier token");
        }

        Identifier {
            value: t.lexeme,
            path: t.path,
            line: t.line,
        }
    }
}

pub struct TestParser<'a, 'b> {
    pub scanner: &'a mut TestScanner<'b>,
}

impl<'a, 'b> TestParser<'a, 'b> {
    pub fn parse(&mut self) -> Result<TestScript, N2VError> {
        self.test_script()
    }

    fn consume(&mut self, tt: TokenType) -> Result<Token, N2VError> {
        let t = self.scanner.next();
        match &t {
            None => Err(N2VError {
                msg: format!("Early end of file expected {:?}", tt),
                kind: ErrorKind::TestParseError(Token {
                    lexeme: String::from(""),
                    path: self.scanner.path.clone(),
                    line: self.scanner.line,
                    token_type: TokenType::Eof,
                }),
            }),
            Some(t) => {
                if t.token_type == tt {
                    Ok(t.clone())
                } else {
                    Err(N2VError {
                        msg: format!(
                            "Expected token type {:?}, found {:?} ({})",
                            tt, t.token_type, t.lexeme
                        ),
                        kind: ErrorKind::TestParseError(t.clone()),
                    })
                }
            }
        }
    }

    fn test_script(&mut self) -> Result<TestScript, N2VError> {
        // Load cannot be a keyword because it is used as a port name.
        if self.consume(TokenType::Identifier).unwrap().lexeme != "load" {
            panic!("Expected load.");
        }

        let generics = self.generics()?;

        let hdl_file = PathBuf::from(self.consume(TokenType::Identifier).unwrap().lexeme);
        self.consume(TokenType::Comma)?;

        self.consume(TokenType::OutputFile)?;
        let output_file = PathBuf::from(self.consume(TokenType::Identifier).unwrap().lexeme);
        self.consume(TokenType::Comma)?;

        self.consume(TokenType::CompareTo)?;
        let compare_file = PathBuf::from(self.consume(TokenType::Identifier).unwrap().lexeme);
        self.consume(TokenType::Comma)?;

        let output_list = self.output_list()?;

        let steps = self.steps()?;

        // match in ports (can out ports come before in ports?)
        // match out ports
        Ok(TestScript {
            path: Some(self.scanner.path.clone()),
            hdl_file,
            compare_file,
            output_file,
            output_list,
            steps,
            generics,
        })
    }

    fn steps(&mut self) -> Result<Vec<Step>, N2VError> {
        let mut res: Vec<Step> = Vec::new();
        loop {
            if self.scanner.peek().is_none() {
                break;
            }
            let mut instructions: Vec<Instruction> = Vec::new();
            loop {
                let token = self.scanner.next();
                match token {
                    Some(Token {
                        token_type: TokenType::Set,
                        ..
                    }) => {
                        instructions.push(self.set());
                    }
                    Some(Token {
                        token_type: TokenType::Eval,
                        ..
                    }) => {
                        instructions.push(self.eval());
                    }
                    Some(Token {
                        token_type: TokenType::Output,
                        ..
                    }) => {
                        instructions.push(self.output());
                    }
                    Some(Token {
                        token_type: TokenType::Tick,
                        ..
                    }) => {
                        instructions.push(Instruction::Tick);
                    }
                    Some(Token {
                        token_type: TokenType::Tock,
                        ..
                    }) => {
                        instructions.push(Instruction::Tock);
                    }
                    _ => {
                        panic!("Unknown instruction type {:?}.", token);
                    }
                }
                if self.scanner.peek().unwrap().token_type == TokenType::Comma {
                    self.consume(TokenType::Comma)?;
                } else {
                    self.consume(TokenType::Semicolon)?;
                    break;
                }
            }
            res.push(Step { instructions });
        }

        Ok(res)
    }

    fn set(&mut self) -> Instruction {
        let port = self.consume(TokenType::Identifier).unwrap().lexeme;
        let format = self.scanner.peek().unwrap().token_type;
        let number_system = match format {
            TokenType::Number => NumberSystem::Decimal,
            TokenType::BinaryFormatSpecifier => {
                self.scanner.next();
                NumberSystem::Binary
            }
            TokenType::HexFormatSpecifier => {
                self.scanner.next();
                NumberSystem::Hex
            }
            _ => {
                panic!("Unexpected format specifier.");
            }
        };
        let value = self.consume(TokenType::Number).unwrap().lexeme;

        Instruction::Set(
            port,
            InputValue {
                number_system,
                value,
            },
        )
    }

    fn eval(&mut self) -> Instruction {
        Instruction::Eval
    }

    fn output(&mut self) -> Instruction {
        Instruction::Output
    }

    fn output_list(&mut self) -> Result<Vec<OutputFormat>, N2VError> {
        let mut res = Vec::new();

        self.consume(TokenType::OutputList)?;

        loop {
            let next = self.scanner.next();
            match &next {
                Some(
                    t @ Token {
                        token_type: TokenType::Identifier,
                        ..
                    },
                ) => {
                    let port_name = t;
                    let number_system = match self.scanner.next() {
                        Some(Token {
                            token_type: TokenType::BinaryFormatSpecifier,
                            ..
                        }) => NumberSystem::Binary,
                        Some(Token {
                            token_type: TokenType::DecimalFormatSpecifier,
                            ..
                        }) => NumberSystem::Decimal,
                        Some(Token {
                            token_type: TokenType::HexFormatSpecifier,
                            ..
                        }) => NumberSystem::Hex,
                        Some(Token {
                            token_type: TokenType::StringFormatSpecifier,
                            ..
                        }) => NumberSystem::String,
                        _ => {
                            panic!()
                        }
                    };
                    let space_before = self
                        .consume(TokenType::Number)
                        .unwrap()
                        .lexeme
                        .parse()
                        .unwrap();
                    self.consume(TokenType::Dot)?;
                    let output_columns = self
                        .consume(TokenType::Number)
                        .unwrap()
                        .lexeme
                        .parse()
                        .unwrap();
                    self.consume(TokenType::Dot)?;
                    let space_after = self
                        .consume(TokenType::Number)
                        .unwrap()
                        .lexeme
                        .parse()
                        .unwrap();

                    res.push(OutputFormat {
                        port_name: port_name.lexeme.clone(),
                        number_system,
                        space_before,
                        output_columns,
                        space_after,
                    });
                }
                Some(Token {
                    token_type: TokenType::Semicolon,
                    ..
                }) => {
                    break;
                }
                _ => {
                    panic!("Expected format specifier.")
                }
            }
        }
        Ok(res)
    }

    fn generics(&mut self) -> Result<Vec<usize>, N2VError> {
        let mut res = Vec::new();

        if self.scanner.peek().unwrap().token_type != TokenType::LeftAngle {
            return Ok(Vec::new());
        }
        self.consume(TokenType::LeftAngle)?;

        loop {
            let next = self.scanner.next();
            match &next {
                Some(
                    t @ Token {
                        token_type: TokenType::Number,
                        ..
                    },
                ) => {
                    // Convert to number.
                    let val: usize = t.lexeme.parse().unwrap();
                    res.push(val);
                }
                Some(Token {
                    token_type: TokenType::Comma,
                    ..
                }) => {
                    continue;
                }
                Some(Token {
                    token_type: TokenType::RightAngle,
                    ..
                }) => {
                    return Ok(res);
                }
                _ => {
                    panic!("Expected number, comma, or right angle, found {:?}", next);
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::env;
    use std::fs;
    use std::path::Path;

    fn read_hdl(path: &std::path::Path) -> String {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let test_file = manifest_dir.join("resources").join("tests").join(path);

        fs::read_to_string(test_file).expect("Unable to read test file.")
    }

    #[test]
    fn test_nand2tetris_solution_and() {
        let path = PathBuf::from("nand2tetris/solutions/And.tst");
        let contents = read_hdl(&path);
        let mut scanner = TestScanner::new(contents.as_str(), path);
        let mut parser = TestParser {
            scanner: &mut scanner,
        };
        parser.parse().expect("Parse failure");
    }

    #[test]
    fn test_nand2tetris_solution_not16() {
        let path = PathBuf::from("nand2tetris/solutions/Not16.tst");
        let contents = read_hdl(&path);
        let mut scanner = TestScanner::new(contents.as_str(), path);
        let mut parser = TestParser {
            scanner: &mut scanner,
        };
        parser.parse().expect("Parse failure");
    }
}
