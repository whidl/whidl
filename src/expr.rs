use std::collections::HashMap;

use serde::Serialize;

use crate::error::N2VError;
use crate::parser::Identifier;

// This is the type that can be used for
// - bus indices
// - start,end in range loops
// - port widths
#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
pub enum GenericWidth {
    Expr(Op, Box<GenericWidth>, Box<GenericWidth>),
    Terminal(Terminal),
}

impl GenericWidth {
    pub fn is_numeric(&self) -> bool {
        matches!(self, GenericWidth::Terminal(Terminal::Num(_)))
    }
}
impl std::ops::Add<GenericWidth> for GenericWidth {
    type Output = GenericWidth;

    fn add(self, rhs: GenericWidth) -> GenericWidth {
        // Handle case where we can actually perform the addition.
        if let GenericWidth::Terminal(Terminal::Num(x)) = self {
            if let GenericWidth::Terminal(Terminal::Num(y)) = rhs {
                return GenericWidth::Terminal(Terminal::Num(x + y));
            }
        }

        GenericWidth::Expr(Op::Add, Box::new(self), Box::new(rhs))
    }
}

impl std::ops::Add<&GenericWidth> for &GenericWidth {
    type Output = GenericWidth;

    fn add(self, rhs: &GenericWidth) -> GenericWidth {
        // Handle case where we can actually perform the addition.
        if let GenericWidth::Terminal(Terminal::Num(x)) = self {
            if let GenericWidth::Terminal(Terminal::Num(y)) = rhs {
                return GenericWidth::Terminal(Terminal::Num(x + y));
            }
        }

        GenericWidth::Expr(Op::Add, Box::new(self.clone()), Box::new(rhs.clone()))
    }
}

impl std::ops::Sub<GenericWidth> for GenericWidth {
    type Output = GenericWidth;

    fn sub(self, rhs: GenericWidth) -> GenericWidth {
        // Handle case where we can actually perform the addition.
        if let GenericWidth::Terminal(Terminal::Num(x)) = self {
            if let GenericWidth::Terminal(Terminal::Num(y)) = rhs {
                return GenericWidth::Terminal(Terminal::Num(x - y));
            }
        }

        GenericWidth::Expr(Op::Sub, Box::new(self), Box::new(rhs))
    }
}

impl std::ops::Sub<&GenericWidth> for &GenericWidth {
    type Output = GenericWidth;

    fn sub(self, rhs: &GenericWidth) -> GenericWidth {
        // Handle case where we can actually perform the addition.
        if let GenericWidth::Terminal(Terminal::Num(x)) = self {
            if let GenericWidth::Terminal(Terminal::Num(y)) = rhs {
                return GenericWidth::Terminal(Terminal::Num(x - y));
            }
        }

        GenericWidth::Expr(Op::Sub, Box::new(self.clone()), Box::new(rhs.clone()))
    }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Serialize)]
pub enum Terminal {
    Var(Identifier),
    Num(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize)]
pub enum Op {
    Add,
    Sub,
    Max,
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Op::Add => {
                write!(f, "+")
            }
            Op::Sub => {
                write!(f, "-")
            }
            Op::Max => {
                write!(f, "MAXIMUM")
            }
        }
    }
}

impl std::fmt::Display for GenericWidth {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GenericWidth::Terminal(i) => {
                write!(f, "{}", i)
            }
            GenericWidth::Expr(op, a, b) => {
                write!(f, "{} {} {}", a, op, b)
            }
        }
    }
}

impl std::fmt::Display for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Terminal::Var(i) => {
                write!(f, "{}", i.value)
            }
            Terminal::Num(i) => {
                write!(f, "{}", i)
            }
        }
    }
}

/// Evaluates an expression and ensures the final result is numeric, i.e. all variables removed.
pub fn eval_expr_numeric(
    expr: &GenericWidth,
    state: &HashMap<String, usize>,
) -> Result<usize, N2VError> {
    let general_state: HashMap<String, GenericWidth> = state
        .iter()
        .map(|(k, v)| (k.clone(), GenericWidth::Terminal(Terminal::Num(*v))))
        .collect();

    let res = eval_expr(expr, &general_state);

    if let GenericWidth::Terminal(Terminal::Num(x)) = res {
        Ok(x)
    } else {
        Err(N2VError {
            msg: format!("Expression {} is non-numeric", expr),
            line: None,
            path: None,
        })
    }
}

/// Evaluates a width expression based on the current state of variables.
pub fn eval_expr(expr: &GenericWidth, state: &HashMap<String, GenericWidth>) -> GenericWidth {
    match expr {
        GenericWidth::Terminal(t) => eval_terminal(t, state),
        GenericWidth::Expr(Op::Add, t1, t2) => eval_expr(t1, state) + eval_expr(t2, state),
        GenericWidth::Expr(Op::Sub, t1, t2) => eval_expr(t1, state) - eval_expr(t2, state),
        GenericWidth::Expr(Op::Max, t1, t2) => eval_max(eval_expr(t1, state), eval_expr(t2, state)),
    }
}

fn eval_max(t1: GenericWidth, t2: GenericWidth) -> GenericWidth {
    if let GenericWidth::Terminal(Terminal::Num(n1)) = t1 {
        if let GenericWidth::Terminal(Terminal::Num(n2)) = t2 {
            return GenericWidth::Terminal(Terminal::Num(std::cmp::max(n1, n2)));
        }
    }

    // No op - we don't know.
    GenericWidth::Expr(Op::Max, Box::new(t1), Box::new(t2))
}

fn eval_terminal(terminal: &Terminal, state: &HashMap<String, GenericWidth>) -> GenericWidth {
    match terminal {
        Terminal::Num(_) => GenericWidth::Terminal(terminal.clone()),
        Terminal::Var(v) => match state.get(&v.value) {
            None => GenericWidth::Terminal(terminal.clone()),
            Some(x) => x.clone(),
        },
    }
}

// Substitutes for a variable if variable name matches.
pub fn replace_expr(w: &GenericWidth, m: &String, r: &GenericWidth) -> GenericWidth {
    match &w {
        GenericWidth::Terminal(t) => replace_term(t, m, r),
        GenericWidth::Expr(op, w1, w2) => GenericWidth::Expr(
            *op,
            Box::new(replace_expr(w1, m, r)),
            Box::new(replace_expr(w2, m, r)),
        ),
    }
}

// Substitutes a terminal if variable name matches.
fn replace_term(t: &Terminal, m: &String, r: &GenericWidth) -> GenericWidth {
    match &t {
        Terminal::Num(_) => GenericWidth::Terminal(t.clone()),
        Terminal::Var(v) => {
            if &v.value == m {
                r.clone()
            } else {
                GenericWidth::Terminal(t.clone())
            }
        }
    }
}
