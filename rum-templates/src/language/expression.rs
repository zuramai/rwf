//! Parse and evaluate expressions.
//!
//! Expressions are operations that when evaluated prdouce a single value.'

use super::{
    super::lexer::{Token, TokenWithContext, Tokenize, Value},
    super::Context,
    super::Error,
    Op, Term,
};

use std::iter::{Iterator, Peekable};

/// Expression parser and executor.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    // Binary expression, i.e. expression with two terms and one operation, e.g. `5 + 10`.
    // The expression is recusive, so you can have something like `(5 + 6) / (1 - 5)`.
    Binary {
        left: Box<Expression>,
        op: Op,
        right: Box<Expression>,
    },

    /// Unary expression, containing one operation and one term.
    /// e.g. `!true`
    Unary {
        op: Op,
        operand: Box<Expression>,
    },

    // Base case for recursive expression parsing, which evaluates to the value
    // of the term, e.g. `5` evalutes to `5` or `variable_name` evalutes to whatever
    // the variable is set to in the context.
    Term {
        term: Term,
    },

    // A list of expressions, e.g.
    // `[1, 2, variable, "hello world"]`
    //
    // The list is dynamically evaluated at runtime, so it can contain variables
    // and constants, as long as the variables are in scope.
    List {
        terms: Vec<Expression>,
    },

    // A function call on an expression, with optional arguments, e.g. `5.to_i`.
    Function {
        term: Box<Expression>,
        name: String,
        args: Vec<Expression>,
    },
}

impl Expression {
    /// Create new constant term.
    pub fn constant(value: Value) -> Self {
        Self::Term {
            term: Term::constant(value),
        }
    }

    /// Create new variable term.
    pub fn variable(variable: String) -> Self {
        Self::Term {
            term: Term::variable(variable),
        }
    }

    /// Evaluate the expression given the context.
    /// Returns a single value as the result.
    pub fn evaluate(&self, context: &Context) -> Result<Value, Error> {
        match self {
            Expression::Term { term } => term.evaluate(context),
            Expression::Binary { left, op, right } => {
                let left = left.evaluate(context)?;
                let right = right.evaluate(context)?;
                op.evaluate_binary(&left, &right)
            }
            Expression::Unary { op, operand } => {
                let operand = operand.evaluate(context)?;
                op.evaluate_unary(&operand)
            }
            Expression::List { terms } => {
                let mut list = vec![];
                for term in terms {
                    list.push(term.evaluate(context)?);
                }
                Ok(Value::List(list))
            }
            Expression::Function { term, name, args } => {
                let value = term.evaluate(context)?;
                let args = args
                    .iter()
                    .map(|arg| arg.evaluate(context))
                    .collect::<Result<Vec<Value>, Error>>()?;

                Ok(value.call(name, &args)?)
            }
        }
    }

    /// Parse a single term.
    /// Consumes tokens from the iterator.
    fn term(iter: &mut Peekable<impl Iterator<Item = TokenWithContext>>) -> Result<Self, Error> {
        let next = iter.next().ok_or(Error::Eof("term next"))?;
        let term = match next.token() {
            Token::Not => {
                let term = Self::term(iter)?;
                Expression::Unary {
                    op: Op::Not,
                    operand: Box::new(term),
                }
            }

            Token::Minus => {
                let term = Self::term(iter)?;
                Expression::Unary {
                    op: Op::Sub,
                    operand: Box::new(term),
                }
            }

            Token::Plus => {
                let term = Self::term(iter)?;
                Expression::Unary {
                    op: Op::Add,
                    operand: Box::new(term),
                }
            }

            Token::RoundBracketStart => {
                let mut count = 1;
                let mut expr = vec![];

                // Count the brackets. The term is finished when the number of opening brackets
                // match the number of closing brackets.
                while count > 0 {
                    let next = iter.peek().ok_or(Error::Eof("round bracket start"))?;

                    match next.token() {
                        Token::RoundBracketStart => {
                            count += 1;
                            expr.push(iter.next().ok_or(Error::Eof("round bracket start 1"))?);
                        }
                        Token::RoundBracketEnd => {
                            count -= 1;

                            // If it's not the closing bracket, push it in for recursive parsing later.
                            if count > 0 {
                                expr.push(iter.next().ok_or(Error::Eof("round bracket end"))?);
                            } else {
                                // Drop the closing bracket, the expression is over.
                                let _ = iter.next().ok_or(Error::Eof("round bracket end 1"))?;
                            }
                        }
                        Token::BlockEnd => return Err(Error::ExpressionSyntax(next.clone())),

                        _ => {
                            expr.push(iter.next().ok_or(Error::Eof("round bracket push"))?);
                        }
                    }
                }

                // Recursively parse the expression in paranthesis.
                Self::function(Self::parse(&mut expr.into_iter().peekable())?, iter)?
            }

            token => {
                let expr = match token {
                    Token::Variable(name) => Self::variable(name),
                    Token::Value(value) => Self::constant(value),
                    Token::SquareBracketStart => {
                        let mut terms = vec![];

                        loop {
                            let next = iter.next().ok_or(Error::Eof("term token"))?;
                            match next.token() {
                                Token::SquareBracketEnd => break,
                                Token::Comma => continue,
                                Token::Value(value) => terms.push(Expression::constant(value)),
                                Token::Variable(variable) => {
                                    terms.push(Expression::variable(variable))
                                }
                                _ => return Err(Error::ExpressionSyntax(next)),
                            }
                        }

                        Expression::List { terms }
                    }

                    _ => return Err(Error::ExpressionSyntax(next)),
                };

                Self::function(expr, iter)?
            }
        };

        Ok(term)
    }

    /// Parse a function expression, e.g. `5.to_string`.
    fn function(
        mut expr: Self,
        iter: &mut Peekable<impl Iterator<Item = TokenWithContext>>,
    ) -> Result<Self, Error> {
        loop {
            let next = iter.peek().map(|t| t.token());

            expr = match next {
                Some(Token::Dot) => {
                    let _ = iter.next().ok_or(Error::Eof("accessor dot"))?;

                    let name = iter.next().ok_or(Error::Eof("accessor name"))?;

                    match name.token() {
                        Token::Variable(name) => {
                            let arg = iter.peek().map(|t| t.token());

                            // Parse arguments if any.
                            let args = match arg {
                                Some(Token::RoundBracketStart) => {
                                    let mut buffer = vec![];
                                    let mut args = vec![];
                                    let _ = iter.next().ok_or(Error::Eof("function args start"));

                                    loop {
                                        let next = match iter.next() {
                                            Some(next) => next,
                                            None => break,
                                        };

                                        match next.token() {
                                            Token::RoundBracketEnd | Token::Comma => {
                                                // Arguments are also expressions, so we can accept variables, constants and other functions.
                                                args.push(Self::parse(
                                                    &mut std::mem::take(&mut buffer)
                                                        .into_iter()
                                                        .peekable(),
                                                )?);
                                            }

                                            // Buffer tokens until we have the full argument value (until we hit a comma).
                                            _token => {
                                                buffer.push(next);
                                            }
                                        }
                                    }

                                    args
                                }

                                _ => {
                                    vec![]
                                }
                            };

                            Expression::Function {
                                term: Box::new(expr),
                                name,
                                args,
                            }
                        }

                        // Array access notation, e.g. `list.0`
                        Token::Value(Value::Integer(n)) => Expression::Function {
                            term: Box::new(expr),
                            name: n.to_string(),
                            args: vec![],
                        },

                        _ => return Err(Error::ExpressionSyntax(name.clone())),
                    }
                }

                Some(_) | None => return Ok(expr),
            };
        }
    }

    /// Recusively parse the expression.
    pub fn parse(
        iter: &mut Peekable<impl Iterator<Item = TokenWithContext>>,
    ) -> Result<Self, Error> {
        // Get the left term, if one exists.
        // TODO: support unary operations.
        let left = Self::term(iter)?;

        // Check if we have another operator.
        let next = match iter.peek() {
            Some(next) => next,
            None => return Ok(left),
        };

        match Op::from_token(next.token()) {
            Some(op) => {
                // We have another operator. Consume the token.
                let _ = iter.next().ok_or(Error::Eof("parse token"))?;

                // Get the right term. This is a binary op.
                let right = Self::term(iter)?;

                // Check if there's another operator.
                let next = iter.peek();

                match next.map(|t| t.token()) {
                    // Expression is over.
                    Some(Token::BlockEnd) | None => Ok(Expression::Binary {
                        left: Box::new(left),
                        op,
                        right: Box::new(right),
                    }),

                    // We have an operator.
                    Some(token) => match Op::from_token(token) {
                        Some(second_op) => {
                            // Consume the token.
                            let _ = iter.next().ok_or(Error::Eof("parse second op"))?;

                            // Get the right term.
                            let right2 = Expression::parse(iter)?;

                            // Check operator precendence.
                            if second_op < op {
                                let expr = Expression::Binary {
                                    left: Box::new(right),
                                    right: Box::new(right2),
                                    op: second_op,
                                };

                                Ok(Expression::Binary {
                                    left: Box::new(left),
                                    right: Box::new(expr),
                                    op,
                                })
                            } else {
                                let left = Expression::Binary {
                                    left: Box::new(left),
                                    right: Box::new(right),
                                    op,
                                };

                                Ok(Expression::Binary {
                                    left: Box::new(left),
                                    right: Box::new(right2),
                                    op: second_op,
                                })
                            }
                        }

                        // Not an op, so syntax error.
                        None => Err(Error::ExpressionSyntax(next.unwrap().clone())),
                    },
                }
            }

            None => return Ok(left),
        }
    }
}

/// Train to evaluate expressions easily. Implemented for `&str` and `String`.
pub trait Evaluate {
    /// Evaluate the expression with the given context.
    fn evaluate(&self, context: &Context) -> Result<Value, Error>;

    /// Evaluate the expression with an empty context.
    /// Useful for testing and expressions that don't use variables.
    fn evaluate_default(&self) -> Result<Value, Error> {
        self.evaluate(&Context::default())
    }
}

impl Evaluate for &str {
    fn evaluate(&self, context: &Context) -> Result<Value, Error> {
        let tokens = self.tokenize()?[1..].to_vec(); // Skip code block start.
        let expr = Expression::parse(&mut tokens.into_iter().peekable())?;
        expr.evaluate(context)
    }
}

impl Evaluate for String {
    fn evaluate(&self, context: &Context) -> Result<Value, Error> {
        self.as_str().evaluate(context)
    }
}

#[cfg(test)]
mod test {
    use super::super::super::Context;
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_if_const() -> Result<(), Error> {
        assert_eq!(r#"<% 1 == 2 %>"#.evaluate_default()?, Value::Boolean(false));
        assert_eq!(r#"<% 1 == 1 %>"#.evaluate_default()?, Value::Boolean(true));

        Ok(())
    }

    #[test]
    fn test_list() -> Result<(), Error> {
        let mut context = Context::default();
        context.set("variable", "world")?;
        context.set("list", vec![1, 2, 3])?;

        let t1 = r#"<% [1, 2, "hello", 3.13, variable] %>"#.evaluate(&context)?;
        assert_eq!(
            t1,
            Value::List(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::String("hello".into()),
                Value::Float(3.13),
                Value::String("world".into()),
            ])
        );

        let t2 = "<% [1, 2, 3] * 2 %>".evaluate_default()?;
        assert_eq!(
            t2,
            Value::List(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
            ])
        );

        assert_eq!("<% [1, 2, 3].0 %>".evaluate_default()?, Value::Integer(1));

        assert_eq!(r#"<% list.0 %>"#.evaluate(&context)?, Value::Integer(1));

        Ok(())
    }

    #[test]
    fn test_hash() -> Result<(), Error> {
        let mut context = Context::default();
        context.set(
            "hash",
            Value::Hash(HashMap::from([("key".to_string(), Value::Integer(5))])),
        )?;

        let t1 = "<% (hash.key * 2.5) - 2.5 %>".evaluate(&context)?;
        assert_eq!(t1, Value::Float(10.0));

        Ok(())
    }

    #[test]
    fn test_call() -> Result<(), Error> {
        assert_eq!(
            "<% 54.5.to_string %>".evaluate_default()?,
            Value::String("54.5".into())
        );
        assert_eq!(
            r#"<% "one".upcase %>"#.evaluate_default()?,
            Value::String("ONE".into())
        );
        assert_eq!(
            r#"<% ("one" + "two" + "three" ).upcase %>"#.evaluate_default()?,
            Value::String("ONETWOTHREE".into())
        );
        assert_eq!(
            r#"<% " one".upcase.trim %>"#.evaluate_default()?,
            Value::String("ONE".into())
        );

        let mut context = Context::default();
        context.set("variable", "hello")?;

        assert_eq!(
            "<% (((variable.upcase * 2) * 1).downcase).upcase %>".evaluate(&context)?,
            Value::String("HELLOHELLO".into())
        );

        Ok(())
    }

    #[test]
    fn test_math() -> Result<(), Error> {
        assert_eq!("<% 2 * 0.5 %>".evaluate_default()?, Value::Float(1.0));
        assert_eq!(
            "<% 2 * 2 + 3 * 5 %>".evaluate_default()?,
            Value::Integer(19)
        );
        assert_eq!("<% 1.5 * 3 + 25 %>".evaluate_default()?, Value::Float(29.5));
        assert_eq!(
            "<% (1 + 5) * 0.25 %>".evaluate_default()?,
            Value::Float(1.5)
        );

        Ok(())
    }

    #[test]
    fn test_unary() -> Result<(), Error> {
        assert_eq!(
            "<% !false == true && true %>".evaluate_default()?,
            Value::Boolean(true)
        );

        let mut context = Context::default();
        context.set("variable", 5)?;
        assert_eq!(
            "<% -variable * 1.5 %>".evaluate(&context)?,
            Value::Float(-7.5)
        );
        Ok(())
    }

    #[test]
    fn test_parenthesis() -> Result<(), Error> {
        let t1 = "<% ((1 + 2) + (-1 - -1)) * 5 + (25 - 5) %>";
        assert_eq!(t1.evaluate_default()?, Value::Integer(35));

        Ok(())
    }

    #[test]
    fn test_syntactic_sugar() -> Result<(), Error> {
        let t1 = r#"<% "copy" * 3 + 1 * "copy" %>"#.evaluate_default()?;
        assert_eq!(t1, Value::String("copycopycopycopy".into()));

        let t2 = r#"<% "where is the love" - "where is the " %>"#.evaluate_default()?;
        assert_eq!(t2, Value::String("love".into()));

        Ok(())
    }

    #[test]
    #[ignore]
    fn test_list_flatten() -> Result<(), Error> {
        let mut context = Context::default();
        context["test"] = Value::List(vec![
            Value::List(vec![Value::Integer(1), Value::Integer(2)]),
            Value::List(vec![Value::Integer(3), Value::Integer(4)]),
        ]);

        let t1 = "<% test.flatten %>".evaluate(&context)?;
        assert_eq!(
            t1,
            Value::List(vec![
                Value::Integer(1),
                Value::Integer(2),
                Value::Integer(3),
                Value::Integer(4)
            ])
        );

        Ok(())
    }
}