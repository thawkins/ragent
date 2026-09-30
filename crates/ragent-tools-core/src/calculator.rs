//! Math expression calculator tool.
//!
//! Provides [`CalculatorTool`], which evaluates a purely arithmetic expression
//! with a small in-crate recursive-descent parser/evaluator.
//!
//! SEC-ragent-tools-core-003 (SECTASKS T-021): the tool previously ran
//! `python3 -c "import math; print(<expr>)"`, i.e. it was an arbitrary Python
//! interpreter reachable from an LLM-supplied string (`__import__('os')` etc.).
//! The evaluator below has no interpreter, no I/O, no imports, and no
//! statement syntax: it accepts numbers, the operators `+ - * / % ^`, unary
//! `+`/`-`, parentheses, and a fixed whitelist of `math` functions/constants.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Maximum expression length accepted, to bound parser work.
const MAX_EXPRESSION_LEN: usize = 4096;

/// Maximum parser nesting depth (parentheses / unary chains).
const MAX_DEPTH: usize = 64;

/// Evaluate a math expression and return the result.
pub struct CalculatorTool;

#[async_trait::async_trait]
impl Tool for CalculatorTool {
    fn name(&self) -> &'static str {
        "calculator"
    }

    fn description(&self) -> &'static str {
        "Evaluate a mathematical expression and return the computed result. \
         Required parameter: `expression` (string). Supports real arithmetic \
         with `+ - * / % ^`, unary `+`/`-`, parentheses, the constants `pi` \
         and `e`, and the functions `sqrt`, `abs`, `floor`, `ceil`, `round`, \
         `exp`, `ln`, `log10`, `log2`, `sin`, `cos`, `tan`, `asin`, `acos`, \
         `atan`, `sinh`, `cosh`, `tanh`, `min`, `max`, and `pow`. Examples: \
         `'2^32'`, `'sqrt(2)'`, `'max(3, 7) * -2'`. It is an arithmetic \
         evaluator only - there are no variables, statements, or imports."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "REQUIRED. Mathematical expression to evaluate (e.g. '2^32 + sqrt(2)')"
                }
            },
            "required": ["expression"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        // A pure arithmetic evaluator has no side effects: it neither runs a
        // subprocess nor touches the filesystem or network.
        "none"
    }

    async fn execute(&self, input: Value, _ctx: &ToolContext) -> Result<ToolOutput> {
        let expr = input["expression"]
            .as_str()
            .context("Missing required 'expression' parameter")?;

        let value = evaluate_expression(expr)?;
        let answer = format_number(value);

        Ok(ToolOutput {
            content: format!("{expr} = {answer}"),
            metadata: Some(json!({
                "expression": expr,
                "result": answer,
            })),
        })
    }
}

/// Parse and evaluate `expression`, returning `NaN`/`inf` for undefined maths.
///
/// Errors carry the reason (unknown name, arity mismatch, syntax) so the model
/// can correct the expression.
pub fn evaluate_expression(expression: &str) -> Result<f64> {
    if expression.len() > MAX_EXPRESSION_LEN {
        bail!("expression is longer than the {MAX_EXPRESSION_LEN} character limit");
    }
    let mut parser = Parser {
        chars: expression.chars().collect(),
        pos: 0,
        depth: 0,
    };
    let value = parser.parse_sum()?;
    parser.skip_whitespace();
    if parser.pos != parser.chars.len() {
        bail!(
            "unexpected trailing input at position {}: '{}'",
            parser.pos,
            parser.remaining_snippet()
        );
    }
    Ok(value)
}

/// Format a computed value without a trailing `.0` on exact integers.
fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// Recursive-descent parser over the expression's characters.
struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && self.chars[self.pos].is_whitespace() {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn remaining_snippet(&self) -> String {
        self.chars[self.pos..].iter().take(16).collect()
    }

    fn consume(&mut self, expected: char) -> bool {
        self.skip_whitespace();
        if self.peek() == Some(expected) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// `sum := product (('+' | '-') product)*`
    fn parse_sum(&mut self) -> Result<f64> {
        let mut value = self.parse_product()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('+') => {
                    self.pos += 1;
                    value += self.parse_product()?;
                }
                Some('-') => {
                    self.pos += 1;
                    value -= self.parse_product()?;
                }
                _ => return Ok(value),
            }
        }
    }

    /// `product := unary (('*' | '/' | '%') unary)*`
    fn parse_product(&mut self) -> Result<f64> {
        let mut value = self.parse_unary()?;
        loop {
            self.skip_whitespace();
            match self.peek() {
                Some('*') => {
                    self.pos += 1;
                    value *= self.parse_unary()?;
                }
                Some('/') => {
                    self.pos += 1;
                    let divisor = self.parse_unary()?;
                    value /= divisor;
                }
                Some('%') => {
                    self.pos += 1;
                    let divisor = self.parse_unary()?;
                    value %= divisor;
                }
                _ => return Ok(value),
            }
        }
    }

    /// `unary := ('+' | '-') unary | power`
    fn parse_unary(&mut self) -> Result<f64> {
        self.skip_whitespace();
        match self.peek() {
            Some('-') => {
                self.pos += 1;
                Ok(-self.parse_unary()?)
            }
            Some('+') => {
                self.pos += 1;
                self.parse_unary()
            }
            _ => self.parse_power(),
        }
    }

    /// `power := atom ('^' unary)?` - right-associative.
    fn parse_power(&mut self) -> Result<f64> {
        let base = self.parse_atom()?;
        self.skip_whitespace();
        if self.peek() == Some('^') {
            self.pos += 1;
            let exponent = self.parse_unary()?;
            return Ok(base.powf(exponent));
        }
        Ok(base)
    }

    /// `atom := number | name ( '(' args ')' )? | '(' sum ')'`
    fn parse_atom(&mut self) -> Result<f64> {
        self.skip_whitespace();
        if self.depth >= MAX_DEPTH {
            bail!("expression nests deeper than the {MAX_DEPTH} level limit");
        }
        match self.peek() {
            Some('(') => {
                self.pos += 1;
                self.depth += 1;
                let value = self.parse_sum()?;
                if !self.consume(')') {
                    bail!("missing closing ')'");
                }
                self.depth -= 1;
                Ok(value)
            }
            Some(c) if c.is_ascii_digit() || c == '.' => self.parse_number(),
            Some(c) if c.is_ascii_alphabetic() || c == '_' => self.parse_name(),
            Some(other) => bail!("unexpected character '{other}'"),
            None => bail!("unexpected end of expression"),
        }
    }

    fn parse_number(&mut self) -> Result<f64> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let literal: String = self.chars[start..self.pos].iter().collect();
        literal
            .parse::<f64>()
            .with_context(|| format!("invalid number literal '{literal}'"))
    }

    fn parse_name(&mut self) -> Result<f64> {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let name: String = self.chars[start..self.pos].iter().collect();

        self.skip_whitespace();
        if self.peek() == Some('(') {
            self.pos += 1;
            self.depth += 1;
            let args = self.parse_args()?;
            self.depth -= 1;
            return apply_function(&name, &args);
        }
        match name.as_str() {
            "pi" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            "tau" => Ok(std::f64::consts::TAU),
            other => bail!(
                "unknown name '{other}' - only arithmetic operators, pi/e/tau, \
                 and the documented math functions are supported"
            ),
        }
    }

    /// Parse a comma-separated argument list up to the closing `)`.
    fn parse_args(&mut self) -> Result<Vec<f64>> {
        let mut args = Vec::new();
        self.skip_whitespace();
        if self.consume(')') {
            return Ok(args);
        }
        loop {
            args.push(self.parse_sum()?);
            self.skip_whitespace();
            if self.consume(',') {
                continue;
            }
            if self.consume(')') {
                return Ok(args);
            }
            bail!("expected ',' or ')' in function arguments");
        }
    }
}

/// Apply one of the whitelisted functions to its arguments.
fn apply_function(name: &str, args: &[f64]) -> Result<f64> {
    /// Require an exact argument count.
    fn arity(name: &str, args: &[f64], expected: usize) -> Result<()> {
        if args.len() != expected {
            bail!("'{name}' takes {expected} argument(s), got {}", args.len());
        }
        Ok(())
    }
    /// Require at least `expected` arguments (for `min`/`max`).
    fn at_least(name: &str, args: &[f64], expected: usize) -> Result<()> {
        if args.len() < expected {
            bail!(
                "'{name}' takes at least {expected} argument(s), got {}",
                args.len()
            );
        }
        Ok(())
    }

    match name {
        "sqrt" => {
            arity(name, args, 1)?;
            Ok(args[0].sqrt())
        }
        "abs" => {
            arity(name, args, 1)?;
            Ok(args[0].abs())
        }
        "floor" => {
            arity(name, args, 1)?;
            Ok(args[0].floor())
        }
        "ceil" => {
            arity(name, args, 1)?;
            Ok(args[0].ceil())
        }
        "round" => {
            arity(name, args, 1)?;
            Ok(args[0].round())
        }
        "exp" => {
            arity(name, args, 1)?;
            Ok(args[0].exp())
        }
        "ln" => {
            arity(name, args, 1)?;
            Ok(args[0].ln())
        }
        "log10" => {
            arity(name, args, 1)?;
            Ok(args[0].log10())
        }
        "log2" => {
            arity(name, args, 1)?;
            Ok(args[0].log2())
        }
        "sin" => {
            arity(name, args, 1)?;
            Ok(args[0].sin())
        }
        "cos" => {
            arity(name, args, 1)?;
            Ok(args[0].cos())
        }
        "tan" => {
            arity(name, args, 1)?;
            Ok(args[0].tan())
        }
        "asin" => {
            arity(name, args, 1)?;
            Ok(args[0].asin())
        }
        "acos" => {
            arity(name, args, 1)?;
            Ok(args[0].acos())
        }
        "atan" => {
            arity(name, args, 1)?;
            Ok(args[0].atan())
        }
        "sinh" => {
            arity(name, args, 1)?;
            Ok(args[0].sinh())
        }
        "cosh" => {
            arity(name, args, 1)?;
            Ok(args[0].cosh())
        }
        "tanh" => {
            arity(name, args, 1)?;
            Ok(args[0].tanh())
        }
        "pow" => {
            arity(name, args, 2)?;
            Ok(args[0].powf(args[1]))
        }
        "min" => {
            at_least(name, args, 1)?;
            Ok(args.iter().copied().fold(f64::INFINITY, f64::min))
        }
        "max" => {
            at_least(name, args, 1)?;
            Ok(args.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        }
        other => bail!("unknown function '{other}'"),
    }
}
