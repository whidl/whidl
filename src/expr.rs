//! AST for expressions in HDL programs.
//! HDL Expressions are limited to addition and subtraction operators.
//! The `Max` operator is for supporting "MAXIMUM" in synthesized VHDL expressions.
//! `Max` cannot be used in HDL. Quartus Lite does not support VHDL 2008... ugh.

use std::cmp::Ordering;
use std::collections::HashMap;

use serde::Serialize;

use crate::error::{ErrorKind, N2VError};
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
        (&self) + (&rhs)
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
        (&self) - (&rhs)
    }
}

impl std::ops::Sub<&GenericWidth> for &GenericWidth {
    type Output = GenericWidth;

    fn sub(self, rhs: &GenericWidth) -> GenericWidth {
        // Handle case where we can actually perform the subtraction.
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

impl std::fmt::Display for GenericWidth {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            GenericWidth::Terminal(i) => {
                write!(f, "{}", i)
            }
            GenericWidth::Expr(op, a, b) => match op {
                Op::Add => {
                    write!(f, "({} + {})", a, b)
                }
                Op::Sub => {
                    write!(f, "({} - {})", a, b)
                }
                Op::Max => {
                    write!(f, "MAXIMUM({}, {})", a, b)
                }
            },
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
            kind: ErrorKind::NonNumeric,
        })
    }
}

/// Evaluates a width expression based on the current state of variables.
pub fn eval_expr(expr: &GenericWidth, state: &HashMap<String, GenericWidth>) -> GenericWidth {
    let res = match expr {
        GenericWidth::Terminal(t) => eval_terminal(t, state),
        GenericWidth::Expr(Op::Add, t1, t2) => eval_expr(t1, state) + eval_expr(t2, state),
        GenericWidth::Expr(Op::Sub, t1, t2) => eval_expr(t1, state) - eval_expr(t2, state),
        GenericWidth::Expr(Op::Max, t1, t2) => eval_max(eval_expr(t1, state), eval_expr(t2, state)),
    };

    // normalize (constant + var) to (var + constant)
    // Makes pattern matching easier later so that we can collapse constants
    if let GenericWidth::Expr(Op::Add, lhs, rhs) = &res {
        if let GenericWidth::Terminal(Terminal::Num(_)) = &**lhs {
            if let GenericWidth::Terminal(Terminal::Var(_)) = &**rhs {
                return GenericWidth::Expr(Op::Add, rhs.clone(), lhs.clone());
            }
        }
    };

    // N is var, C and D are constants
    // (N + C) + D) = N + (C + D)
    // (N - C) + D) = N + (D - C)   if D > C
    // (N - C) + D) = N - (C - D)   if C > D
    // (N - C) + D) = N             if C = D
    if let GenericWidth::Expr(Op::Add, lhs, rhs) = &res {
        if let GenericWidth::Expr(op, lhs_lhs, lhs_rhs) = &**lhs {
            if let n @ GenericWidth::Terminal(Terminal::Var(x)) = &**lhs_lhs {
                if let c @ GenericWidth::Terminal(Terminal::Num(c_num)) = &**lhs_rhs {
                    if let d @ GenericWidth::Terminal(Terminal::Num(d_num)) = &**rhs {
                        let collapse_expr = match *op {
                            Op::Sub => match d_num.cmp(c_num) {
                                Ordering::Greater => GenericWidth::Expr(
                                    Op::Sub,
                                    Box::new(d.clone()),
                                    Box::new(c.clone()),
                                ),
                                Ordering::Less => GenericWidth::Expr(
                                    Op::Sub,
                                    Box::new(c.clone()),
                                    Box::new(d.clone()),
                                ),
                                Ordering::Equal => {
                                    return GenericWidth::Terminal(Terminal::Var(x.clone()));
                                }
                            },
                            Op::Add => GenericWidth::Expr(
                                Op::Add,
                                Box::new(c.clone()),
                                Box::new(d.clone()),
                            ),
                            Op::Max => panic!(), // MAX should already have been evaluated or produced an error.
                        };
                        let collapsed_expr = eval_expr(&collapse_expr, state);

                        let outer_op = match op {
                            Op::Sub => match d_num.cmp(c_num) {
                                Ordering::Greater => Op::Add,
                                Ordering::Less => Op::Sub,
                                Ordering::Equal => panic!(),
                            },
                            Op::Add => Op::Add,
                            Op::Max => panic!(),
                        };
                        let finished = GenericWidth::Expr(
                            outer_op,
                            Box::new(n.clone()),
                            Box::new(collapsed_expr),
                        );
                        return finished;
                    }
                }
            }
        }
    };

    // (N - C) - D) = N - (D + C)
    // (N + C) - D) = N + (C - D)   if C > D
    // (N + C) - D) = N - (D - C)   if C < D
    // (N + C) - D) = N             if C = D
    if let GenericWidth::Expr(Op::Sub, lhs, rhs) = &res {
        if let GenericWidth::Expr(op, lhs_lhs, lhs_rhs) = &**lhs {
            if let n @ GenericWidth::Terminal(Terminal::Var(x)) = &**lhs_lhs {
                if let c @ GenericWidth::Terminal(Terminal::Num(c_num)) = &**lhs_rhs {
                    if let d @ GenericWidth::Terminal(Terminal::Num(d_num)) = &**rhs {
                        let collapse_expr = match *op {
                            Op::Add => match c_num.cmp(d_num) {
                                Ordering::Greater => GenericWidth::Expr(
                                    Op::Sub,
                                    Box::new(c.clone()),
                                    Box::new(d.clone()),
                                ),
                                Ordering::Less => GenericWidth::Expr(
                                    Op::Sub,
                                    Box::new(d.clone()),
                                    Box::new(c.clone()),
                                ),
                                Ordering::Equal => {
                                    return GenericWidth::Terminal(Terminal::Var(x.clone()));
                                }
                            },
                            Op::Sub => GenericWidth::Expr(
                                Op::Add,
                                Box::new(c.clone()),
                                Box::new(d.clone()),
                            ),
                            Op::Max => panic!(), // MAX should already have been evaluated or produced an error.
                        };
                        let collapsed_expr = eval_expr(&collapse_expr, state);
                        let outer_op = match op {
                            Op::Add => match c_num.cmp(d_num) {
                                Ordering::Greater => Op::Add,
                                Ordering::Less => Op::Sub,
                                Ordering::Equal => panic!(),
                            },
                            Op::Sub => Op::Sub,
                            Op::Max => panic!(),
                        };
                        let finished = GenericWidth::Expr(
                            outer_op,
                            Box::new(n.clone()),
                            Box::new(collapsed_expr),
                        );
                        return finished;
                    }
                }
            }
        }
    };

    res
}

// Returns true if a and b have the same variable name, ignoring
// file name and file line.
fn same_variable_name(a: &Identifier, b: &Identifier) -> bool {
    a.value == b.value
}

fn eval_max(t1: GenericWidth, t2: GenericWidth) -> GenericWidth {
    // Constant compared with constant
    if let GenericWidth::Terminal(Terminal::Num(n1)) = t1 {
        if let GenericWidth::Terminal(Terminal::Num(n2)) = t2 {
            return GenericWidth::Terminal(Terminal::Num(std::cmp::max(n1, n2)));
        }
    }

    // N, N -> LHS (EQ)
    if let GenericWidth::Terminal(Terminal::Var(n1)) = &t1 {
        if let GenericWidth::Terminal(Terminal::Var(n2)) = &t2 {
            if same_variable_name(n1, n2) {
                return t1;
            }
        }
    }

    // N, D -> LHS
    if let GenericWidth::Terminal(Terminal::Var(_)) = t1 {
        if let GenericWidth::Terminal(Terminal::Num(_)) = t2 {
            return t1;
        }
    }
    // D, N -> RHS
    if let GenericWidth::Terminal(Terminal::Num(_)) = t1 {
        if let GenericWidth::Terminal(Terminal::Var(_)) = t2 {
            return t2;
        }
    }
    // N op C, D -> LHS
    if let GenericWidth::Expr(op, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Terminal(Terminal::Var(_)) = &**lhs_lhs {
            if let GenericWidth::Terminal(Terminal::Num(_)) = &**lhs_rhs {
                if let GenericWidth::Terminal(Terminal::Num(_)) = &t2 {
                    if op == &Op::Add || op == &Op::Sub {
                        return t1;
                    }
                }
            }
        }
    }
    // D, N op C -> RHS
    if let GenericWidth::Expr(op, rhs_lhs, rhs_rhs) = &t2 {
        if let GenericWidth::Terminal(Terminal::Var(_)) = &**rhs_lhs {
            if let GenericWidth::Terminal(Terminal::Num(_)) = &**rhs_rhs {
                if let GenericWidth::Terminal(Terminal::Num(_)) = &t1 {
                    if op == &Op::Add || op == &Op::Sub {
                        return t2;
                    }
                }
            }
        }
    }

    // N, N - C     -> LHS
    if let GenericWidth::Expr(Op::Sub, rhs_lhs, rhs_rhs) = &t2 {
        if let GenericWidth::Terminal(Terminal::Var(n1)) = &**rhs_lhs {
            if let GenericWidth::Terminal(Terminal::Num(_)) = &**rhs_rhs {
                if let GenericWidth::Terminal(Terminal::Var(n2)) = &t1 {
                    if same_variable_name(n1, n2) {
                        return t1;
                    }
                }
            }
        }
    }

    // N - C, N      -> RHS
    if let GenericWidth::Expr(Op::Sub, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Terminal(Terminal::Var(n1)) = &**lhs_lhs {
            if let GenericWidth::Terminal(Terminal::Num(_)) = &**lhs_rhs {
                if let GenericWidth::Terminal(Terminal::Var(n2)) = &t2 {
                    if same_variable_name(n1, n2) {
                        return t2;
                    }
                }
            }
        }
    }

    // N, N + C     -> RHS
    if let GenericWidth::Expr(Op::Add, rhs_lhs, rhs_rhs) = &t2 {
        if let GenericWidth::Terminal(Terminal::Var(n1)) = &**rhs_lhs {
            if let GenericWidth::Terminal(Terminal::Num(_)) = &**rhs_rhs {
                if let GenericWidth::Terminal(Terminal::Var(n2)) = &t1 {
                    if same_variable_name(n1, n2) {
                        return t2;
                    }
                }
            }
        }
    }

    // N + C, N     -> LHS
    if let GenericWidth::Expr(Op::Add, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Terminal(Terminal::Var(n1)) = &**lhs_lhs {
            if let GenericWidth::Terminal(Terminal::Num(_)) = &**lhs_rhs {
                if let GenericWidth::Terminal(Terminal::Var(n2)) = &t2 {
                    if same_variable_name(n1, n2) {
                        return t1;
                    }
                }
            }
        }
    }

    if let GenericWidth::Expr(Op::Add, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Expr(Op::Add, rhs_lhs, rhs_rhs) = &t2 {
            if let GenericWidth::Terminal(Terminal::Var(n1)) = &**lhs_lhs {
                if let GenericWidth::Terminal(Terminal::Num(c)) = &**lhs_rhs {
                    if let GenericWidth::Terminal(Terminal::Var(n2)) = &**rhs_lhs {
                        if let GenericWidth::Terminal(Terminal::Num(d)) = &**rhs_rhs {
                            if same_variable_name(n1, n2) {
                                // N + C, N + D -> LHS if C = D and C == 0
                                if c == &0 && d == &0 {
                                    return GenericWidth::Terminal(Terminal::Var(n1.clone()));
                                }

                                match c.cmp(d) {
                                    // N + C, N + D -> LHS if C > D
                                    Ordering::Greater => {
                                        return t1;
                                    }
                                    // N + C, N + D -> RHS if D > C
                                    Ordering::Less => {
                                        return t2;
                                    }
                                    // N + C, N + D -> EQ if D = C
                                    Ordering::Equal => {
                                        return t1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if let GenericWidth::Expr(Op::Sub, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Expr(Op::Add, rhs_lhs, rhs_rhs) = &t2 {
            if let GenericWidth::Terminal(Terminal::Var(n1)) = &**lhs_lhs {
                if let GenericWidth::Terminal(Terminal::Num(c)) = &**lhs_rhs {
                    if let GenericWidth::Terminal(Terminal::Var(n2)) = &**rhs_lhs {
                        if let GenericWidth::Terminal(Terminal::Num(d)) = &**rhs_rhs {
                            if same_variable_name(n1, n2) {
                                // N - C, N + D -> EQ if C = D  and C = 0
                                if c == &0 && d == &0 {
                                    return GenericWidth::Terminal(Terminal::Var(n1.clone()));
                                }
                                // N - C, N + D -> RHS if C > D
                                // N - C, N + D -> RHS if C < D
                                // N - C, N + D -> RHS if C = D and C != 0
                                return t2;
                            }
                        }
                    }
                }
            }
        }
    }

    if let GenericWidth::Expr(Op::Add, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Expr(Op::Sub, rhs_lhs, rhs_rhs) = &t2 {
            if let GenericWidth::Terminal(Terminal::Var(n1)) = &**lhs_lhs {
                if let GenericWidth::Terminal(Terminal::Num(c)) = &**lhs_rhs {
                    if let GenericWidth::Terminal(Terminal::Var(n2)) = &**rhs_lhs {
                        if let GenericWidth::Terminal(Terminal::Num(d)) = &**rhs_rhs {
                            if same_variable_name(n1, n2) {
                                // N + C, N - D -> LHS if C = D and C == 0
                                if c == &0 && d == &0 {
                                    return GenericWidth::Terminal(Terminal::Var(n1.clone()));
                                }
                                // N + C, N - D -> LHS if C > C h
                                // N + C, N - D -> LHS if D > C
                                // N + C, N - D -> LHS if C = D and C != 0
                                return t1;
                            }
                        }
                    }
                }
            }
        }
    }

    if let GenericWidth::Expr(Op::Sub, lhs_lhs, lhs_rhs) = &t1 {
        if let GenericWidth::Expr(Op::Sub, rhs_lhs, rhs_rhs) = &t2 {
            if let GenericWidth::Terminal(Terminal::Var(n1)) = &**lhs_lhs {
                if let GenericWidth::Terminal(Terminal::Num(c)) = &**lhs_rhs {
                    if let GenericWidth::Terminal(Terminal::Var(n2)) = &**rhs_lhs {
                        if let GenericWidth::Terminal(Terminal::Num(d)) = &**rhs_rhs {
                            if same_variable_name(n1, n2) {
                                // N - C, N - D -> LHS if C = D and C == 0
                                if c == &0 && d == &0 {
                                    return GenericWidth::Terminal(Terminal::Var(n1.clone()));
                                }

                                match c.cmp(d) {
                                    // N - C, N - D -> RHS if C > D
                                    Ordering::Greater => {
                                        return t2;
                                    }
                                    // N - C, N - D -> LHS if D > C
                                    Ordering::Less => {
                                        return t1;
                                    }
                                    // N - C, N - D -> EQ if C = D
                                    Ordering::Equal => {
                                        return t1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // We don't know what to do. For example MAX(X, Y) is impossible.
    // Ideally this would be disallowed in the HDL before we hit this panic.
    panic!("I don't know how to simplify MAX({}, {}).", t1, t2);
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_expr_simplify_2_add_2() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
        );
        let expected = GenericWidth::Terminal(Terminal::Num(4));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_2_sub_2() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
        );
        let expected = GenericWidth::Terminal(Terminal::Num(0));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_add_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_assoc_norm() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_plus_1_plus_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_1_plus_n_plus_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_minus_2_plus_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(2))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_minus_1_plus_2() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_minus_2_minus_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(2))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(3))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_minus_1_plus_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_expr_simplify_n_plus_1_minus_1() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N, D -> LHS
    #[test]
    fn test_expr_simplify_max_n_d() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // D, N -> RHS
    #[test]
    fn test_expr_simplify_max_d_n() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N op C, D -> LHS
    #[test]
    fn test_expr_simplify_max_n_plus_c_d() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N, N
    #[test]
    fn test_expr_simplify_max_n_n() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // D, N op C -> RHS
    #[test]
    fn test_expr_simplify_max_d_n_plus_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N, N - C     -> LHS
    #[test]
    fn test_expr_simplify_max_n_n_minus_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N     -> RHS
    #[test]
    fn test_expr_simplify_max_n_minus_c_n() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N     -> LHS
    #[test]
    fn test_expr_simplify_max_n_plus_c_n() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N, N + C     -> RHS
    #[test]
    fn test_expr_simplify_max_n_n_plus_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N + D -> LHS if C > D
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_plus_d_big_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(2))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(2))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N + D -> RHS if D > C
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_plus_d_big_d() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(4))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(55))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(55))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N + D -> EQ if D = C
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_plus_d_equal_nonzero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(4))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(4))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(4))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N - D -> LHS if C = D and C == 0
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_plus_d_equal_zero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
        );
        let actual = eval_expr(&input, &state);
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        assert_eq!(actual, expected);
    }

    // N - C, N + D -> RHS if C > D
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_plus_d_big_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(4))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(1))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(1))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N + D -> RHS if C < D
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_plus_d_big_d() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(5))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(99))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(99))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N + D -> RHS if C = D and C != 0
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_plus_d_equal_nonzero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(3))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N + D -> EQ if C = D  and C = 0
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_plus_d_equal_zero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N - D -> LHS if C = D and C == 0
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_minus_d_equal_zero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N - D -> LHS if C > D
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_minus_d_big_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(5))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(5))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N - D -> LHS if D > C
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_minus_d_big_d() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(5))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(10))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(5))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N + C, N - D -> LHS if C = D and C != 0
    #[test]
    fn test_expr_simplify_max_n_plus_c_n_minus_d_equal_nonzero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Add,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(5))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(5))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Add,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(5))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N - D -> LHS if C = D and C == 0
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_minus_d_equal_zero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(0))),
            )),
        );
        let expected = GenericWidth::Terminal(Terminal::Var(Identifier::from("N")));
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N - D -> LHS if C = D and C != 0
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_minus_d_equal_nonzero() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(3))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N - D -> RHS if C > D
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_minus_d_big_c() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(13))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(3))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }

    // N - C, N - D -> LHS if D > C
    #[test]
    fn test_expr_simplify_max_n_minus_c_n_minus_d_big_d() {
        let state = HashMap::new();
        let input = GenericWidth::Expr(
            Op::Max,
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(3))),
            )),
            Box::new(GenericWidth::Expr(
                Op::Sub,
                Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
                Box::new(GenericWidth::Terminal(Terminal::Num(13))),
            )),
        );
        let expected = GenericWidth::Expr(
            Op::Sub,
            Box::new(GenericWidth::Terminal(Terminal::Var(Identifier::from("N")))),
            Box::new(GenericWidth::Terminal(Terminal::Num(3))),
        );
        let actual = eval_expr(&input, &state);
        assert_eq!(actual, expected);
    }
}
