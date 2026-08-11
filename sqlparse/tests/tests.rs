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

use sqlparser::lexer::Lexer;
use sqlparser::lexer::Tok;
use sqlparser::{parse_stmt, SqlParseError};
//use sqlparser::sqlstmt::SqlStmtExprParser;

#[test]
fn lexer_token_sequence() {
    // Test tokens including comma, dot, and <> (NotEq)
    let src = "SELECT a, b FROM tbl WHERE a <> 1";
    let toks: Vec<(usize, Tok, usize)> = Lexer::new(src).collect();

    // Expect sequence: Select, Ident(a), Comma, Ident(b), From, Ident(tbl), Where, Ident(a), NotEq, Ident("1"), Eof
    let types: Vec<&str> = toks
        .iter()
        .map(|(_, t, _)| match t {
            Tok::Create => "Create",
            Tok::Drop => "Drop",
            Tok::Describe => "Describe",
            Tok::Alter => "Alter",
            Tok::Add => "Add",
            Tok::Dataset => "Dataset",
            Tok::Relationship => "Relationship",
            Tok::Inverse => "Inverse",
            Tok::Primary => "Primary",
            Tok::Key => "Key",
            Tok::Unique => "Unique",
            Tok::Index => "Index",
            Tok::As => "As",
            Tok::Is => "Is",
            Tok::Not => "Not",
            Tok::Insert => "Insert",
            Tok::Delete => "Delete",
            Tok::Update => "Update",
            Tok::Set => "Set",
            Tok::Into => "Into",
            Tok::Values => "Values",
            Tok::Select => "Select",
            Tok::From => "From",
            Tok::Where => "Where",
            Tok::And => "And",
            Tok::Or => "Or",
            Tok::Eq => "Eq",
            Tok::EqEq => "EqEq",
            Tok::In => "In",
            Tok::Like => "Like",
            Tok::Regexp => "Regexp",
            Tok::Ne => "NotEq",
            Tok::Gt => "Gt",
            Tok::Lt => "Lt",
            Tok::Ge => "Ge",
            Tok::Le => "Le",
            Tok::Dot => "Dot",
            Tok::Comma => "Comma",
            Tok::Dollar => "Dollar",
            Tok::Arobas => "Arobas",
            Tok::QuestionMark => "QuestionMark",
            Tok::Ident(_) => "Ident",
            Tok::Bool(_) => "Bool",
            Tok::Null => "Null",
            Tok::Number(_) => "Number",
            Tok::Integer(_) => "Integer",
            Tok::SingleQuotedString(_) => "SingleQuotedString",
            Tok::DoubleQuotedString(_) => "DoubleQuotedString",
            Tok::Eof => "Eof",
            Tok::Plus => "+",
            Tok::Minus => "-",
            Tok::Multiply => "*",
            Tok::Divide => "/",
            Tok::OpenBracket => "(",
            Tok::CloseBracket => ")",
            Tok::OpenSqBracket => "[",
            Tok::CloseSqBracket => "]",
        })
        .collect();

    let expected = vec![
        "Select", "Ident", "Comma", "Ident", "From", "Ident", "Where", "Ident", "NotEq", "Integer",
    ];
    assert_eq!(types, expected);
}

#[test]
fn lexer_number_and_quoted_string() {
    let src = "SELECT 'hello', 12345 FROM tbl";
    let toks: Vec<(usize, Tok, usize)> = Lexer::new(src).collect();

    let types: Vec<&str> = toks
        .iter()
        .map(|(_, t, _)| match t {
            Tok::Select => "Select",
            Tok::From => "From",
            Tok::Where => "Where",
            Tok::And => "And",
            Tok::Or => "Or",
            Tok::Eq => "Eq",
            Tok::EqEq => "EqEq",
            Tok::Ne => "Ne",
            Tok::In => "In",
            Tok::Gt => "Gt",
            Tok::Lt => "Lt",
            Tok::Dot => "Dot",
            Tok::Comma => "Comma",
            Tok::Ident(_) => "Ident",
            Tok::Integer(_) => "Integer",
            Tok::Number(_) => "Number",
            Tok::SingleQuotedString(_) => "QuotedString",
            Tok::Eof => "Eof",
            _ => "Other",
        })
        .collect();

    // Expect: Select, QuotedString, Comma, Number, From, Ident(tbl), Eof
    let expected = vec!["Select", "QuotedString", "Comma", "Integer", "From", "Ident"];
    assert_eq!(types, expected);
}

const VALID_STMTS: [&str; 7] = [
    // Parsed AST: Select(SelectStmt {
    //     proj_list: [Member { val_idx: 0, member: Path([PathSegment { name: "x" }]) }],
    //     from_list: [Member { val_idx: 0, member: Path([PathSegment { name: "y" }]) }],
    //     predicate_list: [] })
    "select x from y",
    // proj_list: [Member { val_idx: 0, member: Path([PathSegment { name: "a" }]) }, Member { val_idx: 0, member: Path([PathSegment { name: "b" }]) }]
    "select a,b from c",
    // proj_list: [Member { val_idx: 0, member: Path([PathSegment { name: "a" }, PathSegment { name: "b" }]) }
    "select a.b from d",
    // predicate_list: [Predicate {
    //     left: Member { val_idx: 0, member: Path([PathSegment { name: "c" }]) },
    //     right: Member { val_idx: 0, member: Path([PathSegment { name: "d" }]) },
    //     comp_operator: Equal }]
    "select a from b where c=d",
    // predicate_list: ... right: Member { val_idx: 0, member: Value(QuotedString("d")) }
    "select a from b where c='d'",
    // TODO: should have AND operator in predicate_list
    // predicate_list: [ Predicate { left: ..., comp_operator: Equal }, Predicate { left: ..., comp_operator: Equal } ]
    "select a from b where c=d and e=f",
    // from_list: [Member { val_idx: 0, member: Path([PathSegment { name: "b" }, PathSegment { name: "c" }]) }, Member { val_idx: 0, ... }]
    "select a",
];

#[test]
fn test_valid_select_statements() {
    let inputs = VALID_STMTS;
    for input in &inputs {
        let res = parse_stmt(input);
        if res.is_err() {
            // Print the token stream for debugging
            let mut tok_lexer = Lexer::new(input);
            let toks: Vec<(usize, sqlparser::lexer::Tok, usize)> =
                std::iter::from_fn(|| tok_lexer.next()).collect();
            println!(
                "test_valid_select_statements - Input: '{}'\nTokens: {:?}\nRes: {:?}",
                input, toks, res
            );
        }
        assert!(
            res.is_ok(),
            "Expected valid input to parse successfully: {}",
            input
        );

        println!("Input: '{}'\nParsed AST: {:?}\n", input, res.unwrap());
    }
}

#[test]
fn test_valid_create_ds_stmt() {
    let inputs = [
        "create dataset Person",
        "create dataset Employee primary key (id)",
        "create dataset Employee primary key (id, name)",
        "create dataset Employee primary key (id, name), relationship works_at(Company), relationship friends(Employee)",
        // TBD: multiple pkey should be rejected at runtime
        "create dataset Employee primary key (id, name), primary key (id, name)",
    ];
    for input in &inputs {
        let res = parse_stmt(input);
        if res.is_err() {
            // Print the token stream for debugging
            let mut tok_lexer = Lexer::new(input);
            let toks: Vec<(usize, sqlparser::lexer::Tok, usize)> =
                std::iter::from_fn(|| tok_lexer.next()).collect();
            println!(
                "Create dataset - Input: '{}'\nTokens: {:?}\nRes: {:?}",
                input, toks, res
            );
        }
        assert!(
            res.is_ok(),
            "Create dataset - Expected valid input to parse successfully: {}",
            input
        );

        println!(
            "Create dataset - Input: '{}'\nParsed AST: {:?}\n",
            input,
            res.unwrap()
        );
    }
}

#[test]
fn test_math_expressions() {
    let inputs = [
        "select 1 + 1",
        "select 2 * 3",
        "select 4 / 2",
        "select 5 - 3",
        "select (1 + 2) * 3",
        "select a + b from c where d - e > 10",
    ];
    for input in &inputs {
        let res = parse_stmt(input);
        if res.is_err() {
            // Print the token stream for debugging
            //let mut tok_lexer = Lexer::new(input);
            //let toks: Vec<(usize, sqlparser::lexer::Tok, usize)> =
            //  std::iter::from_fn(|| tok_lexer.next()).collect();
        }
        assert!(
            res.is_ok(),
            "Math expressions - Expected valid input to parse successfully: {}",
            input
        );

        println!("Math expressions - Input: '{}'", input);
        let stmt = &res.unwrap();
        println!("{}", stmt.print_tree());
    }
}

const UNRECOGNIZED_EOF_STMTS: [&str; 2] = ["select a from", "select a from b where"];

#[test]
fn test_unrecognized_eof() {
    let inputs = UNRECOGNIZED_EOF_STMTS;
    for input in &inputs {
        let res = parse_stmt(input);
        if let Ok(_) = res {
            // Print the token stream for debugging
            let mut tok_lexer = Lexer::new(input);
            let toks: Vec<(usize, sqlparser::lexer::Tok, usize)> =
                std::iter::from_fn(|| tok_lexer.next()).collect();
            println!(
                "Expected error for incomplete input but parsed successfully. Input: '{}'\nTokens: {:?}\nRes: {:?}",
                input, toks, res
            );
        }
        assert!(
            res.is_err(),
            "Expected error for incomplete input: {}",
            input
        );
        if let Err(err) = res {
            println!("Returned error. Input: '{}'\nerror: {:?}", input, err);
            match err {
                SqlParseError::UnrecognizedEof { location, expected } => {
                    println!(
                        "Test UnrecognizedEof: Input: '{}'\nLocation: {}, Expected: {:?}\n",
                        input, location, expected
                    );
                }
                _ => panic!(
                    "Expected UnrecognizedEof error. Input: '{}'\ngot: {:?}",
                    input, err
                ),
            }
        }
    }
}

const UNRECOGNIZED_TOKEN_STMTS: [&str; 4] = [
    "select from c",
    "select a,b c",
    "select a from b 2",
    "select a from b.c",
];

// UnrecognizedToken { start: 7, token: "from", end: 11, expected: ["r#\"[a-zA-Z_][a-zA-Z0-9_]*\"#"] }
#[test]
fn test_unrecognized_token() {
    let inputs = UNRECOGNIZED_TOKEN_STMTS;
    for input in &inputs {
        let res = parse_stmt(input);
        assert!(res.is_err(), "Expected error for invalid input: {}", input);
        if let Err(err) = res {
            match err {
                SqlParseError::UnrecognizedToken { token, expected } => {
                    // Print the token stream for debugging
                    let mut tok_lexer = Lexer::new(input);
                    let toks: Vec<(usize, sqlparser::lexer::Tok, usize)> =
                        std::iter::from_fn(|| tok_lexer.next()).collect();
                    println!(
                        "Input: '{}' Token: {:?} Expected: {:?}",
                        input, token, expected
                    );
                    println!("Input: '{}'\nTokens: {:?}\n", input, toks);
                }
                _ => panic!(
                    "Expected UnrecognizedToken error. Input: '{}'\ngot: {:?}",
                    input, err
                ),
            }
        }
    }
}

const INVALID_TOKEN_STMTS: [&str; 2] = [
    // TBD: lexer should error on invalid character
    "select # from c",
    // TBD: lexer should error on invalid character
    "select a from 4",
];

#[test]
#[ignore = "TBD: should fail on invalid tokens"]
fn test_invalid_token() {
    let inputs = INVALID_TOKEN_STMTS;
    for input in &inputs {
        let res = parse_stmt(input);
        println!(
            "\nInvalidToken result. Input: '{}'\nresult: {:?}",
            input, res
        );

        assert!(res.is_err(), "Expected error for invalid input: {}", input);
        if let Err(err) = res {
            match err {
                SqlParseError::InvalidToken { location } => {
                    println!(
                        "Test InvalidToken: Input: '{}'\nLocation: {}\n",
                        input, location
                    );
                }
                _ => panic!(
                    "Expected InvalidToken error. Input: '{}'\ngot: {:?}",
                    input, err
                ),
            }
        }
    }
}
