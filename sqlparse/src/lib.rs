// Copyright 2026 No Despondency Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Note: we avoid depending directly on the lalrpop_util crate here so tests
// don't require the crate to be present. Parser errors will be converted to
// the local ParseError::Other(String) variant when the concrete parser error
// type isn't available.

pub mod ast;
pub mod lexer;
pub mod sqlstmt;

use thiserror::Error;

use lalrpop_util::ParseError;
use crate::lexer::{Tok, LexicalError};

// Re-export commonly-used items so consumers (tests) can access them as sqlparser::...
//pub use sqlstmt::ParseError;
pub use sqlstmt::SqlStmtExprParser;

// Local parse error type which wraps possible parser errors.
#[derive(Error, Debug)]
pub enum SqlParseError {
    #[error("Invalid token at location {location}")]
    InvalidToken {
        location: usize,
    },
    #[error("Unrecognized EOF at location {location}")]
    UnrecognizedEof {
        location: usize,
        expected: Vec<String>,
    },
    #[error("Unrecognized token {token:?}")]
    UnrecognizedToken {
        token: (usize, Tok, usize),
        expected: Vec<String>,
    },
    #[error("Extra token {token:?}")]
    ExtraToken {
        token: (usize, Tok, usize),
    },
    #[error("User error: {0:?}")]
    User(LexicalError),
    #[error("Other error: {0}")]
    Other(String),
}

/// Parse the input and return either an AST or a local SqlParseError.
pub fn parse_stmt(input: &str) -> Result<ast::SqlStmt, SqlParseError> {
    // Create a token iterator from the lexer and feed it to the generated parser
    let mut lexer = crate::lexer::Lexer::new(input);
    match sqlstmt::SqlStmtExprParser::new().parse(&mut lexer) {
        Ok(ast) => Ok(ast),
        Err(e) => match e {
            ParseError::InvalidToken { location } => Err(SqlParseError::InvalidToken { location }),
            ParseError::UnrecognizedEof { location, expected } => {
                Err(SqlParseError::UnrecognizedEof { location, expected })
            }
            ParseError::UnrecognizedToken { token, expected} => {
                Err(SqlParseError::UnrecognizedToken { token, expected})
            }
            ParseError::ExtraToken { token } => Err(SqlParseError::ExtraToken { token }),
            ParseError::User { error } => Err(SqlParseError::User(error)),
        },
    }
}
