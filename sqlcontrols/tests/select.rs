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

#[allow(unused_imports)]
use prost::Message;
use sqlexet::SqlExeTrait;

#[test]
fn test_select_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset job",
        "create dataset tgtds primary key(b,c)",
        "create dataset myds relationship rs(tgtds)",
        "insert into myds values '{\"a\": 2}'",
        "insert into myds values '{\"a\": 3}'",
        "insert into tgtds values '{}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 1}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 2}'",
        "insert into myds.rs values null where a = 2",
        "insert into myds.rs values ('hi', 1) where a = 2",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Statement, expected rows
        // 1 - Basics
        (
            "select * from dataset where name = 'job'",
            vec!["{\"name\":\"job\"}"],
        ),
        (
            "select * from myds",
            vec!["{\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}", "{\"a\":3}"],
        ),
        // 2 - Rels
        (
            "select rs.* from myds",
            vec!["{}", "{\"b\":\"hi\",\"c\":1}"],
        ),
        ("select rs from myds", vec!["{}", "{\"b\":\"hi\",\"c\":1}"]),
        (
            "select *, rs.* from myds",
            vec![
                "{\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}, {}",
                "{\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}, {\"b\":\"hi\",\"c\":1}",
                "{\"a\":3}, null",
            ],
        ),
        (
            "select a, rs.* from myds where rs.c = 1",
            vec!["2, {\"b\":\"hi\",\"c\":1}"],
        ),
        (
            "select a, rs.b, rs.c from myds",
            vec!["2, null, null", "2, hi, 1", "3, null, null"],
        ),
        (
            // TBD - not very useful, check if we want to support it
            "select * from tgtds where * = '{\"b\":\"hi\",\"c\":1}'",
            vec!["{\"b\":\"hi\",\"c\":1}"],
        ),
        // 4 - Aliases
        ("select tgtds.b from tgtds where tgtds.c = 1", vec!["hi"]),
        ("select m.b from tgtds m where m.c = 1", vec!["hi"]),
        ("select m.b from tgtds m where tgtds.c = 1", vec!["hi"]),
        // 5 - Joins
        (
            "select d.a, t.b from myds d, tgtds t where d.a = t.c",
            vec!["2, there"],
        ),
        (
            "select d.a, t.b from myds d, tgtds t",
            vec![
                "2, null", "2, hi", "2, there", "3, null", "3, hi", "3, there",
            ],
        ),
        (
            "select d.a, t.b from myds d, tgtds t where t.b = 'hi'",
            vec!["2, hi", "3, hi"],
        ),
        (
            "select d.a, t.b from myds d, tgtds t where d.a = 2",
            vec!["2, null", "2, hi", "2, there"],
        ),
    ];
    for (stmt, expected_rows) in suite {
        println!("test_select_stmt_exe - executing '{}'", stmt);
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
    }
}

#[test]
fn test_invrel_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset job",
        "create dataset tgtds primary key(b,c)",
        "create dataset myds relationship rs(tgtds)",
        "insert into myds values '{\"a\": 2}'",
        "insert into myds values '{\"a\": 3}'",
        "insert into tgtds values '{}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 1}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 2}'",
        "insert into myds.rs values null where a = 2",
        "insert into myds.rs values ('hi', 1) where a = 2",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(
            res.is_ok(),
            "Failed to initialize '{}': {:?}",
            stmt,
            res.err()
        );
    }
    let suite = vec![
        // Statement, expected rows
        (
            "select *, inverse(myds.rs).* from tgtds",
            vec![
                "{}, {\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}",
                "{\"b\":\"hi\",\"c\":1}, {\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}",
                "{\"b\":\"there\",\"c\":2}, null",
            ],
        ),
        (
            "select b from tgtds where inverse(myds.rs).a = 2",
            vec!["null", "hi"],
        ),
        (
            "select inverse(myds.rs).a from tgtds where b = 'hi'",
            vec!["2"],
        ),
        (
            "select inverse(myds.rs).* from tgtds where b = 'hi'",
            vec!["{\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}"],
        ),
        (
            "select b, inverse(myds.rs).a, inverse(myds.rs).* from tgtds where c = 1",
            vec!["hi, 2, {\"a\":2,\"rs\":[\"null null\",\"hi 1\"]}"],
        ),
        (
            "select b, inverse(myds.rs).a, inverse(myds.rs).rs.* from tgtds where c = 1",
            vec!["hi, 2, {}", "hi, 2, {\"b\":\"hi\",\"c\":1}"],
        ),
    ];
    for (stmt, expected_rows) in suite {
        println!("test_invrel_stmt_exe - executing '{}'", stmt);
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
    }
}
