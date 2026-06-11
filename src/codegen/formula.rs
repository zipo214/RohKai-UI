/// Pure-Rust infix expression parser and Rust-code emitter for the Formula Widget.
///
/// Grammar (precedence low→high):
///   expr    → sum
///   sum     → product (('+' | '-') product)*
///   product → unary (('*' | '/' | '%') unary)*
///   unary   → '-' unary | power
///   power   → atom ('^' unary)?
///   atom    → NUMBER | IDENT | IDENT '(' args ')' | '(' expr ')'
///   args    → expr (',' expr)*
///
/// Variables in the expression are free identifiers (not function calls).
/// They map to `self.{name} as f64` in generated Rust code.
use std::collections::HashSet;

// ---------------------------------------------------------------------------
// Tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    LParen,
    RParen,
    Comma,
    Eof,
    Invalid(char),
}

fn tokenize(src: &str) -> Vec<Tok> {
    let mut chars = src.chars().peekable();
    let mut out = Vec::new();
    while let Some(&ch) = chars.peek() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {
                chars.next();
            }
            '0'..='9' | '.' => {
                let mut buf = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        buf.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(buf.parse::<f64>().map_or(Tok::Invalid('.'), Tok::Num));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut buf = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        buf.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push(Tok::Ident(buf));
            }
            '+' => {
                chars.next();
                out.push(Tok::Plus);
            }
            '-' => {
                chars.next();
                out.push(Tok::Minus);
            }
            '*' => {
                chars.next();
                out.push(Tok::Star);
            }
            '/' => {
                chars.next();
                out.push(Tok::Slash);
            }
            '%' => {
                chars.next();
                out.push(Tok::Percent);
            }
            '^' => {
                chars.next();
                out.push(Tok::Caret);
            }
            '(' => {
                chars.next();
                out.push(Tok::LParen);
            }
            ')' => {
                chars.next();
                out.push(Tok::RParen);
            }
            ',' => {
                chars.next();
                out.push(Tok::Comma);
            }
            c => {
                chars.next();
                out.push(Tok::Invalid(c));
            }
        }
    }
    out.push(Tok::Eof);
    out
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FormulaNode {
    Num(f64),
    Var(String),
    Call { name: String, args: Vec<FormulaNode> },
    Binary { op: BinOp, lhs: Box<FormulaNode>, rhs: Box<FormulaNode> },
    Neg(Box<FormulaNode>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
}

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct FormulaError(pub String);

impl std::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    tokens: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.tokens[self.pos]
    }

    fn bump(&mut self) -> Tok {
        let t = self.tokens[self.pos].clone();
        if self.pos + 1 < self.tokens.len() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, expected: &Tok) -> bool {
        if self.peek() == expected {
            self.bump();
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Result<FormulaNode, FormulaError> {
        self.parse_sum()
    }

    fn parse_sum(&mut self) -> Result<FormulaNode, FormulaError> {
        let mut lhs = self.parse_product()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_product()?;
            lhs = FormulaNode::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_product(&mut self) -> Result<FormulaNode, FormulaError> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Rem,
                _ => break,
            };
            self.bump();
            let rhs = self.parse_unary()?;
            lhs = FormulaNode::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> Result<FormulaNode, FormulaError> {
        if self.eat(&Tok::Minus) {
            let inner = self.parse_unary()?;
            return Ok(FormulaNode::Neg(Box::new(inner)));
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<FormulaNode, FormulaError> {
        let base = self.parse_atom()?;
        if self.eat(&Tok::Caret) {
            let exp = self.parse_unary()?;
            return Ok(FormulaNode::Binary {
                op: BinOp::Pow,
                lhs: Box::new(base),
                rhs: Box::new(exp),
            });
        }
        Ok(base)
    }

    fn parse_atom(&mut self) -> Result<FormulaNode, FormulaError> {
        match self.peek().clone() {
            Tok::Num(v) => {
                self.bump();
                Ok(FormulaNode::Num(v))
            }
            Tok::Ident(name) => {
                self.bump();
                if self.eat(&Tok::LParen) {
                    // function call
                    let mut args = Vec::new();
                    if !matches!(self.peek(), Tok::RParen) {
                        args.push(self.parse_expr()?);
                        while self.eat(&Tok::Comma) {
                            args.push(self.parse_expr()?);
                        }
                    }
                    if !self.eat(&Tok::RParen) {
                        return Err(FormulaError("expected ')' after function arguments".into()));
                    }
                    Ok(FormulaNode::Call { name, args })
                } else {
                    Ok(FormulaNode::Var(name))
                }
            }
            Tok::LParen => {
                self.bump();
                let inner = self.parse_expr()?;
                if !self.eat(&Tok::RParen) {
                    return Err(FormulaError("expected ')'".into()));
                }
                Ok(inner)
            }
            Tok::Eof => Err(FormulaError("unexpected end of expression".into())),
            Tok::Invalid(c) => Err(FormulaError(format!("unexpected character '{c}'"))),
            t => Err(FormulaError(format!("unexpected token {t:?}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Public parse entry point
// ---------------------------------------------------------------------------

/// Parse an infix formula expression.  Returns `Err` if the expression is
/// syntactically invalid.
pub fn parse_formula(expr: &str) -> Result<FormulaNode, FormulaError> {
    let tokens = tokenize(expr);
    let mut p = Parser { tokens, pos: 0 };
    let node = p.parse_expr()?;
    if !matches!(p.peek(), Tok::Eof) {
        return Err(FormulaError("unexpected token after expression".into()));
    }
    Ok(node)
}

// ---------------------------------------------------------------------------
// Variable collection
// ---------------------------------------------------------------------------

/// Collect the set of free variable names referenced in the formula.
pub fn collect_variables(node: &FormulaNode) -> Vec<String> {
    let mut set = HashSet::new();
    collect_vars_inner(node, &mut set);
    let mut v: Vec<String> = set.into_iter().collect();
    v.sort();
    v
}

fn collect_vars_inner(node: &FormulaNode, out: &mut HashSet<String>) {
    match node {
        FormulaNode::Num(_) => {}
        FormulaNode::Var(name) => {
            out.insert(name.clone());
        }
        FormulaNode::Call { args, .. } => {
            for a in args {
                collect_vars_inner(a, out);
            }
        }
        FormulaNode::Binary { lhs, rhs, .. } => {
            collect_vars_inner(lhs, out);
            collect_vars_inner(rhs, out);
        }
        FormulaNode::Neg(inner) => collect_vars_inner(inner, out),
    }
}

// ---------------------------------------------------------------------------
// Rust code emission
// ---------------------------------------------------------------------------

/// Emit a Rust expression string for the formula.
///
/// Variable references are emitted as `self.{name} as f64` so the formula
/// operates entirely in `f64` precision. The result is `as f32` at the call
/// site if needed.
///
/// Supported functions: abs, sqrt, cbrt, floor, ceil, round, sin, cos, tan,
/// asin, acos, atan, atan2, ln, log2, log10, exp, min, max, clamp, signum,
/// hypot, pow.
pub fn emit_formula_rust(node: &FormulaNode) -> String {
    match node {
        FormulaNode::Num(v) => {
            if *v == v.floor() && v.abs() < 1e15_f64 {
                format!("{v}_f64")
            } else {
                format!("{:?}_f64", v)
            }
        }
        FormulaNode::Var(name) => format!("(self.{name} as f64)"),
        FormulaNode::Call { name, args } => emit_call(name, args),
        FormulaNode::Binary { op, lhs, rhs } => {
            let l = emit_formula_rust(lhs);
            let r = emit_formula_rust(rhs);
            match op {
                BinOp::Add => format!("({l} + {r})"),
                BinOp::Sub => format!("({l} - {r})"),
                BinOp::Mul => format!("({l} * {r})"),
                BinOp::Div => format!("({l} / {r})"),
                BinOp::Rem => format!("({l} % {r})"),
                BinOp::Pow => format!("({l}).powf({r})"),
            }
        }
        FormulaNode::Neg(inner) => format!("(-{})", emit_formula_rust(inner)),
    }
}

fn emit_call(name: &str, args: &[FormulaNode]) -> String {
    let a = |i: usize| args.get(i).map(emit_formula_rust).unwrap_or_else(|| "0_f64".to_owned());
    match name {
        "abs" => format!("{}.abs()", a(0)),
        "sqrt" => format!("{}.sqrt()", a(0)),
        "cbrt" => format!("{}.cbrt()", a(0)),
        "floor" => format!("{}.floor()", a(0)),
        "ceil" => format!("{}.ceil()", a(0)),
        "round" => format!("{}.round()", a(0)),
        "sin" => format!("{}.sin()", a(0)),
        "cos" => format!("{}.cos()", a(0)),
        "tan" => format!("{}.tan()", a(0)),
        "asin" => format!("{}.asin()", a(0)),
        "acos" => format!("{}.acos()", a(0)),
        "atan" => format!("{}.atan()", a(0)),
        "atan2" => format!("{}.atan2({})", a(0), a(1)),
        "ln" => format!("{}.ln()", a(0)),
        "log2" => format!("{}.log2()", a(0)),
        "log10" => format!("{}.log10()", a(0)),
        "exp" => format!("{}.exp()", a(0)),
        "min" => format!("{}.min({})", a(0), a(1)),
        "max" => format!("{}.max({})", a(0), a(1)),
        "clamp" => format!("{}.clamp({}, {})", a(0), a(1), a(2)),
        "signum" => format!("{}.signum()", a(0)),
        "hypot" => format!("{}.hypot({})", a(0), a(1)),
        "pow" => format!("{}.powf({})", a(0), a(1)),
        other => {
            let arg_list = args.iter().map(emit_formula_rust).collect::<Vec<_>>().join(", ");
            format!("{other}({arg_list})")
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_addition() {
        let node = parse_formula("a + b").unwrap();
        assert!(matches!(node, FormulaNode::Binary { op: BinOp::Add, .. }));
    }

    #[test]
    fn parses_nested_arithmetic() {
        let node = parse_formula("(a + b) * 2").unwrap();
        assert!(matches!(node, FormulaNode::Binary { op: BinOp::Mul, .. }));
    }

    #[test]
    fn parses_function_call() {
        let node = parse_formula("sqrt(x)").unwrap();
        assert!(matches!(node, FormulaNode::Call { .. }));
    }

    #[test]
    fn parses_power_operator() {
        let node = parse_formula("x ^ 2").unwrap();
        assert!(matches!(node, FormulaNode::Binary { op: BinOp::Pow, .. }));
    }

    #[test]
    fn parses_unary_negation() {
        let node = parse_formula("-x").unwrap();
        assert!(matches!(node, FormulaNode::Neg(_)));
    }

    #[test]
    fn rejects_empty_expression() {
        assert!(parse_formula("").is_err());
    }

    #[test]
    fn rejects_unmatched_paren() {
        assert!(parse_formula("(a + b").is_err());
    }

    #[test]
    fn collects_variables() {
        let node = parse_formula("a + b * sqrt(c)").unwrap();
        let vars = collect_variables(&node);
        assert_eq!(vars, vec!["a", "b", "c"]);
    }

    #[test]
    fn collects_no_vars_from_literal() {
        let node = parse_formula("3.14 * 2").unwrap();
        let vars = collect_variables(&node);
        assert!(vars.is_empty());
    }

    #[test]
    fn emits_rust_addition() {
        let node = parse_formula("a + b").unwrap();
        let rust = emit_formula_rust(&node);
        assert_eq!(rust, "((self.a as f64) + (self.b as f64))");
    }

    #[test]
    fn emits_rust_sqrt_call() {
        let node = parse_formula("sqrt(width * height)").unwrap();
        let rust = emit_formula_rust(&node);
        assert_eq!(
            rust,
            "((self.width as f64) * (self.height as f64)).sqrt()"
        );
    }

    #[test]
    fn emits_rust_power() {
        let node = parse_formula("x ^ 2").unwrap();
        let rust = emit_formula_rust(&node);
        assert_eq!(rust, "((self.x as f64)).powf(2_f64)");
    }

    #[test]
    fn emits_rust_negation() {
        let node = parse_formula("-x").unwrap();
        let rust = emit_formula_rust(&node);
        assert_eq!(rust, "(-(self.x as f64))");
    }

    #[test]
    fn emits_rust_nested_calls() {
        let node = parse_formula("max(abs(a), 1)").unwrap();
        let rust = emit_formula_rust(&node);
        assert_eq!(rust, "(self.a as f64).abs().max(1_f64)");
    }

    #[test]
    fn emits_multiarg_clamp() {
        let node = parse_formula("clamp(x, 0, 100)").unwrap();
        let rust = emit_formula_rust(&node);
        assert_eq!(rust, "(self.x as f64).clamp(0_f64, 100_f64)");
    }
}
