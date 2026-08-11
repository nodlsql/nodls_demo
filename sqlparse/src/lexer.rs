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

use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Create,
    Drop,
    Describe,
    Alter,
    Add,
    Relationship,
    Inverse,
    Primary,
    Key,
    Unique,
    Index,
    Insert,
    Delete,
    Update,
    Set,
    Into,
    Values,
    Select,
    From,
    As,
    Is,
    In,
    Like,
    Regexp,
    Not,
    Where,
    And,
    Or,
    Eq,
    EqEq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    Plus,
    Minus,
    Multiply,
    Divide,
    Dot,
    Comma,
    Dollar,
    Arobas,
    QuestionMark,
    OpenBracket,
    CloseBracket,
    OpenSqBracket,
    CloseSqBracket,
    Dataset,       // Query schema instances - "select .. from dataset"
    Ident(String), // For identifiers and keywords not recognized as reserved words
    Null,
    Bool(bool),
    Number(String),
    Integer(String),
    SingleQuotedString(String),
    DoubleQuotedString(String),
    Eof,
}

#[derive(Debug)]
pub enum LexicalError {
    // Not possible
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub column: usize,
}

pub struct Lexer<'input> {
    //input: &'input str,
    chars: Peekable<Chars<'input>>,
    offset: usize,
}

impl<'input> Lexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Lexer {
            //input,
            chars: input.chars().peekable(),
            offset: 0,
        }
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next();
        if let Some(ch) = c {
            self.offset += ch.len_utf8();
        }
        c
    }

    fn peek(&mut self) -> Option<&char> {
        self.chars.peek()
    }

    fn read_ident(&mut self, first: char) -> String {
        let mut s = String::new();
        s.push(first);
        while let Some(&ch) = self.peek() {
            if ch.is_alphanumeric() || ch == '_' {
                s.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    // TBD - this is not invoked if number starts with a dot, would have to
    // TBD - also plus similar logic for Dot token in switch below.
    fn read_number(&mut self, first: char) -> String {
        let mut s = String::new();
        // Allow only one dot
        let mut dot_seen = if first == '.' { true } else { false };
        // Return empty string if no digits after dot
        // TBD - not necessary here, can put first char as dot in '.' switch case
        if first == '.' {
            if let Some(&ch) = self.peek() {
                if !ch.is_ascii_digit() {
                    return s;
                }
            } else {
                // end of input
                return s;
            }
        }
        s.push(first);
        while let Some(&ch) = self.peek() {
            if ch.is_ascii_digit() || (ch == '.' && !dot_seen) {
                if ch == '.' {
                    dot_seen = true;
                }
                s.push(ch);
                self.bump();
            } else {
                break;
            }
        }
        s
    }

    fn read_quoted_string(&mut self, quote_char: char) -> String {
        // Assumes opening quote has already been consumed.
        let mut s = String::new();
        while let Some(&ch) = self.peek() {
            self.bump();
            if ch == quote_char {
                // closing quote found; stop without including it
                break;
            } else {
                s.push(ch);
            }
        }
        s
    }
}

impl<'input> Iterator for Lexer<'input> {
    type Item = (usize, Tok, usize);

    fn next(&mut self) -> Option<Self::Item> {
        // skip whitespace
        while let Some(&ch) = self.peek() {
            if ch.is_whitespace() {
                self.bump();
                continue;
            }
            break;
        }

        let start = self.offset;
        let ch = self.bump();

        let tok = match ch {
            None => Tok::Eof,
            Some('(') => Tok::OpenBracket,
            Some(')') => Tok::CloseBracket,
            Some('[') => Tok::OpenSqBracket,
            Some(']') => Tok::CloseSqBracket,
            Some(',') => Tok::Comma,
            Some('.') => Tok::Dot,
            Some('$') => Tok::Dollar,
            Some('@') => Tok::Arobas,
            Some('?') => Tok::QuestionMark,
            Some('+') => Tok::Plus,
            Some('-') => Tok::Minus,
            Some('*') => Tok::Multiply,
            Some('/') => Tok::Divide,
            Some('=') => {
                if let Some(&'=') = self.peek() {
                    self.bump();
                    Tok::EqEq
                } else {
                    Tok::Eq
                }
            }
            Some('>') => {
                if let Some(&'=') = self.peek() {
                    self.bump();
                    Tok::Ge
                } else {
                    Tok::Gt
                }
            }
            Some('<') => {
                // check for '<>' not-equal
                if let Some(&'>') = self.peek() {
                    self.bump();
                    Tok::Ne
                } else if let Some(&'=') = self.peek() {
                    self.bump();
                    Tok::Le
                } else {
                    Tok::Lt
                }
            }
            Some(c) if c.is_alphabetic() => {
                let s = self.read_ident(c);
                match s.to_ascii_lowercase().as_str() {
                    "create" => Tok::Create,
                    "drop" => Tok::Drop,
                    "describe" => Tok::Describe,
                    "alter" => Tok::Alter,
                    "add" => Tok::Add,
                    //"set" => Tok::Set,
                    "dataset" => Tok::Dataset,
                    "relationship" => Tok::Relationship,
                    "inverse" => Tok::Inverse,
                    "primary" => Tok::Primary,
                    "key" => Tok::Key,
                    "unique" => Tok::Unique,
                    "index" => Tok::Index,
                    "insert" => Tok::Insert,
                    "delete" => Tok::Delete,
                    "update" => Tok::Update,
                    "set" => Tok::Set,
                    "into" => Tok::Into,
                    "values" => Tok::Values,
                    "select" => Tok::Select,
                    "from" => Tok::From,
                    "where" => Tok::Where,
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "true" => Tok::Bool(true),
                    "false" => Tok::Bool(false),
                    "null" => Tok::Null,
                    "as" => Tok::As,
                    "is" => Tok::Is,
                    "in" => Tok::In,
                    "like" => Tok::Like,
                    "regexp" => Tok::Regexp,
                    "not" => Tok::Not,
                    other => Tok::Ident(other.to_string()),
                }
            }
            Some(c) if c.is_ascii_digit() || c == '.' => {
                let s = self.read_number(c);
                if s.contains('.') {
                    Tok::Number(s)
                } else {
                    Tok::Integer(s)
                }
            }
            Some('\'') => {
                // read until closing single quote
                let s = self.read_quoted_string('\'');
                Tok::SingleQuotedString(s)
            }
             Some('\"') => {
                // read until closing single quote
                let s = self.read_quoted_string('\"');
                Tok::DoubleQuotedString(s)
            }
            Some(other) => {
                // Unknown character/token - treat as identifier of single char to allow error handling in parser
                Tok::Ident(other.to_string())
            }
        };

        let end = self.offset;
        // debug printing removed for quieter test output

        if tok == Tok::Eof {
            None
        } else {
            Some((start, tok, end))
        }
    }
}
