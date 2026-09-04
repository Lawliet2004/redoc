use crate::ast::*;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Number(f64),
    String(String),
    Identifier(String),
    CellRef {
        sheet: Option<String>,
        row: u32,
        col: u32,
    },
    RangeRef {
        sheet: Option<String>,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    },
    Plus,
    Minus,
    Star,
    Slash,
    Ampersand,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    LParen,
    RParen,
    Comma,
    Colon,
    EOF,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>, FormulaError> {
    let mut chars = input.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&ch) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }

        match ch {
            '+' => {
                chars.next();
                tokens.push(Token::Plus);
            }
            '-' => {
                chars.next();
                tokens.push(Token::Minus);
            }
            '*' => {
                chars.next();
                tokens.push(Token::Star);
            }
            '/' => {
                chars.next();
                tokens.push(Token::Slash);
            }
            '&' => {
                chars.next();
                tokens.push(Token::Ampersand);
            }
            '(' => {
                chars.next();
                tokens.push(Token::LParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            ':' => {
                chars.next();
                tokens.push(Token::Colon);
            }
            '=' => {
                chars.next();
                tokens.push(Token::Eq);
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Lte);
                } else if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::Neq);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Gte);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '"' {
                        closed = true;
                        break;
                    }
                    s.push(c);
                }
                if !closed {
                    return Err(FormulaError::Value);
                }
                tokens.push(Token::String(s));
            }
            '0'..='9' => {
                let mut num_str = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        num_str.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let val: f64 = num_str.parse().map_err(|_| FormulaError::Value)?;
                tokens.push(Token::Number(val));
            }
            'A'..='Z' | 'a'..='z' | '_' | '$' => {
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '!' || c == '$' || c == '.' {
                        ident.push(c);
                        chars.next();
                    } else {
                        break;
                    }
                }

                // A parenthesized identifier is always a function name, even
                // when it looks like an A1 reference (for example LOG10).
                // Resolve this before cell/range parsing so digit-suffixed
                // Excel functions cannot be mistaken for empty cells.
                if chars.peek() == Some(&'(') {
                    tokens.push(Token::Identifier(ident.to_uppercase()));
                } else if let Some(token) = parse_cell_or_ident(&ident, &mut chars) {
                    tokens.push(token);
                } else {
                    tokens.push(Token::Identifier(ident.to_uppercase()));
                }
            }
            _ => {
                chars.next();
            }
        }
    }

    tokens.push(Token::EOF);
    Ok(tokens)
}

fn parse_cell_or_ident(
    ident: &str,
    chars: &mut std::iter::Peekable<std::str::Chars>,
) -> Option<Token> {
    let (sheet, cell_part) = if ident.contains('!') {
        let parts: Vec<&str> = ident.split('!').collect();
        (Some(parts[0].to_string()), parts[1])
    } else {
        (None, ident)
    };

    if let Some((row, col)) = parse_a1_reference(cell_part) {
        // Check if colon follows for RangeRef
        if chars.peek() == Some(&':') {
            chars.next(); // consume colon
                          // peek next word
            let mut end_part = String::new();
            while let Some(&c) = chars.peek() {
                if c.is_alphanumeric() || c == '$' {
                    end_part.push(c);
                    chars.next();
                } else {
                    break;
                }
            }
            if let Some((end_row, end_col)) = parse_a1_reference(&end_part) {
                return Some(Token::RangeRef {
                    sheet,
                    start_row: row,
                    start_col: col,
                    end_row,
                    end_col,
                });
            }
        }
        return Some(Token::CellRef { sheet, row, col });
    }

    None
}

/// Parse `A1`, `Sheet!A1`, or `Sheet!A1:B2` into sheet + range bounds.
pub fn parse_a1_range(s: &str) -> Option<(Option<String>, u32, u32, u32, u32)> {
    let trimmed = s.trim();
    let (sheet, rest) = if let Some(idx) = trimmed.rfind('!') {
        let sheet_part = trimmed[..idx].trim();
        let sheet_name = sheet_part
            .strip_prefix('\'')
            .and_then(|s| s.strip_suffix('\''))
            .unwrap_or(sheet_part);
        (Some(sheet_name.to_string()), trimmed[idx + 1..].trim())
    } else {
        (None, trimmed)
    };

    if let Some((start, end)) = rest.split_once(':') {
        let (start_row, start_col) = parse_a1_reference(start)?;
        let (end_row, end_col) = parse_a1_reference(end)?;
        Some((sheet, start_row, start_col, end_row, end_col))
    } else if let Some((row, col)) = parse_a1_reference(rest) {
        Some((sheet, row, col, row, col))
    } else {
        None
    }
}

pub fn parse_a1_reference(s: &str) -> Option<(u32, u32)> {
    let mut col_str = String::new();
    let mut row_str = String::new();

    for c in s.chars() {
        if c == '$' {
            continue;
        }
        if c.is_ascii_alphabetic() {
            if !row_str.is_empty() {
                return None;
            }
            col_str.push(c.to_ascii_uppercase());
        } else if c.is_ascii_digit() {
            row_str.push(c);
        } else {
            return None;
        }
    }

    if col_str.is_empty() || row_str.is_empty() {
        return None;
    }

    let mut col: u32 = 0;
    for c in col_str.chars() {
        col = col * 26 + ((c as u32) - ('A' as u32) + 1);
    }
    let row: u32 = row_str.parse().ok()?;

    Some((row, col))
}

pub fn parse_formula(input: &str) -> Result<Expr, FormulaError> {
    let text = input.trim();
    let expr_text = if let Some(stripped) = text.strip_prefix('=') {
        stripped
    } else {
        text
    };
    let tokens = tokenize(expr_text)?;
    let mut parser = Parser::new(tokens);
    parser.parse_expr(0)
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&Token::EOF)
    }

    fn next_token(&mut self) -> Token {
        let t = self.tokens.get(self.pos).cloned().unwrap_or(Token::EOF);
        self.pos += 1;
        t
    }

    fn parse_expr(&mut self, min_bp: u8) -> Result<Expr, FormulaError> {
        let mut lhs = match self.next_token().clone() {
            Token::Number(n) => Expr::Literal(FormulaValue::Number(n)),
            Token::String(s) => Expr::Literal(FormulaValue::String(s)),
            Token::CellRef { sheet, row, col } => Expr::CellRef { sheet, row, col },
            Token::RangeRef {
                sheet,
                start_row,
                start_col,
                end_row,
                end_col,
            } => Expr::RangeRef {
                sheet,
                start_row,
                start_col,
                end_row,
                end_col,
            },
            Token::Identifier(name) => {
                if self.peek() == &Token::LParen {
                    self.next_token(); // consume (
                    let mut args = Vec::new();
                    if self.peek() != &Token::RParen {
                        loop {
                            args.push(self.parse_expr(0)?);
                            if self.peek() == &Token::Comma {
                                self.next_token();
                            } else {
                                break;
                            }
                        }
                    }
                    if self.peek() == &Token::RParen {
                        self.next_token();
                    }
                    Expr::FunctionCall { name, args }
                } else {
                    match name.to_ascii_uppercase().as_str() {
                        "TRUE" => Expr::Literal(FormulaValue::Boolean(true)),
                        "FALSE" => Expr::Literal(FormulaValue::Boolean(false)),
                        _ => Expr::Name(name),
                    }
                }
            }
            Token::LParen => {
                let e = self.parse_expr(0)?;
                if self.peek() == &Token::RParen {
                    self.next_token();
                }
                e
            }
            Token::Minus => {
                let rhs = self.parse_expr(10)?;
                Expr::Binary {
                    left: Box::new(Expr::Literal(FormulaValue::Number(0.0))),
                    op: BinaryOp::Sub,
                    right: Box::new(rhs),
                }
            }
            _ => return Err(FormulaError::Value),
        };

        loop {
            let (l_bp, r_bp, op) = match self.peek() {
                Token::Plus => (1, 2, BinaryOp::Add),
                Token::Minus => (1, 2, BinaryOp::Sub),
                Token::Star => (3, 4, BinaryOp::Mul),
                Token::Slash => (3, 4, BinaryOp::Div),
                Token::Ampersand => (1, 2, BinaryOp::Concat),
                Token::Eq => (1, 2, BinaryOp::Eq),
                Token::Neq => (1, 2, BinaryOp::Neq),
                Token::Lt => (1, 2, BinaryOp::Lt),
                Token::Lte => (1, 2, BinaryOp::Lte),
                Token::Gt => (1, 2, BinaryOp::Gt),
                Token::Gte => (1, 2, BinaryOp::Gte),
                _ => break,
            };

            if l_bp < min_bp {
                break;
            }

            self.next_token();
            let rhs = self.parse_expr(r_bp)?;
            lhs = Expr::Binary {
                left: Box::new(lhs),
                op,
                right: Box::new(rhs),
            };
        }

        Ok(lhs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_a1_reference_with_dollar() {
        assert_eq!(parse_a1_reference("$A$1"), Some((1, 1)));
        assert_eq!(parse_a1_reference("$A1"), Some((1, 1)));
        assert_eq!(parse_a1_reference("A$1"), Some((1, 1)));
        assert_eq!(parse_a1_reference("A1"), Some((1, 1)));
        assert_eq!(parse_a1_reference("$B$10"), Some((10, 2)));
    }

    #[test]
    fn test_parse_formula_dollar_references() {
        let expr = parse_formula("=$A$1 + B$2 + $C3").unwrap();
        assert_eq!(
            expr,
            Expr::Binary {
                left: Box::new(Expr::Binary {
                    left: Box::new(Expr::CellRef {
                        sheet: None,
                        row: 1,
                        col: 1,
                    }),
                    op: BinaryOp::Add,
                    right: Box::new(Expr::CellRef {
                        sheet: None,
                        row: 2,
                        col: 2,
                    }),
                }),
                op: BinaryOp::Add,
                right: Box::new(Expr::CellRef {
                    sheet: None,
                    row: 3,
                    col: 3,
                }),
            }
        );

        let range_expr = parse_formula("=SUM($A$1:$B$10)").unwrap();
        assert_eq!(
            range_expr,
            Expr::FunctionCall {
                name: "SUM".to_string(),
                args: vec![Expr::RangeRef {
                    sheet: None,
                    start_row: 1,
                    start_col: 1,
                    end_row: 10,
                    end_col: 2,
                }],
            }
        );
    }

    #[test]
    fn test_parse_boolean_literals() {
        assert_eq!(
            parse_formula("=TRUE").unwrap(),
            Expr::Literal(FormulaValue::Boolean(true))
        );
        assert_eq!(
            parse_formula("=false").unwrap(),
            Expr::Literal(FormulaValue::Boolean(false))
        );
        assert_eq!(
            parse_formula("=True").unwrap(),
            Expr::Literal(FormulaValue::Boolean(true))
        );
        assert_eq!(
            parse_formula("=FALSE").unwrap(),
            Expr::Literal(FormulaValue::Boolean(false))
        );

        // Expression with boolean literal
        let expr = parse_formula("=IF(TRUE, 1, 2)").unwrap();
        assert_eq!(
            expr,
            Expr::FunctionCall {
                name: "IF".to_string(),
                args: vec![
                    Expr::Literal(FormulaValue::Boolean(true)),
                    Expr::Literal(FormulaValue::Number(1.0)),
                    Expr::Literal(FormulaValue::Number(2.0)),
                ],
            }
        );
    }

    #[test]
    fn test_parse_dotted_function_names() {
        assert_eq!(
            parse_formula("=STDEV.P(A1:B1)").unwrap(),
            Expr::FunctionCall {
                name: "STDEV.P".to_string(),
                args: vec![Expr::RangeRef {
                    sheet: None,
                    start_row: 1,
                    start_col: 1,
                    end_row: 1,
                    end_col: 2,
                }],
            }
        );
    }

    #[test]
    fn test_parse_named_expression() {
        assert_eq!(
            parse_formula("=amount").unwrap(),
            Expr::Name("AMOUNT".to_string())
        );
    }
}
