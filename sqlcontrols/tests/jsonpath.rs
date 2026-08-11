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
use sqlcontrols::utils::SqlExecError;
use sqlexet::SqlExeTrait;

#[test]
#[ignore = "    "]
fn test_select_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset job",
        "create dataset tgtds primary key(b,c)",
        "create dataset myds relationship rs(tgtds)",
        "create dataset jpth",
        "insert into myds values '{\"a\": 2}'",
        "insert into myds values '{\"a\": 3}'",
        "insert into tgtds values '{}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 1}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 2}'",
        "insert into jpth values '{\"i\": [{\"a\": 1}, {\"a\": 2}]}'",
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
            "select $ from dataset where name = 'job'",
            vec!["{\"name\":\"job\"}"],
        ),
        ("select $ from myds", vec!["{\"a\":2}", "{\"a\":3}"]),
        ("select $.* from myds", vec!["2", "3"]),
        // 2 - Rels
        ("select rs.$.* from myds", vec!["null", "[\"hi\",1]"]),
        (
            "select $, rs.$.* from myds",
            vec![
                "{\"a\":2}, null",
                "{\"a\":2}, [\"hi\",1]",
                "{\"a\":3}, null",
            ],
        ),
        (
            "select $.a, rs.$.* from myds where rs.$.c = 1",
            vec!["2, [\"hi\",1]"],
        ),
        (
            "select m.$.a, m.rs.$.* from myds m where m.rs.$.c = 1",
            vec!["2, [\"hi\",1]"],
        ),
        (
            "select $.a, rs.$.b, rs.$.c from myds",
            vec!["2, null, null", "2, hi, 1", "3, null, null"],
        ),
        (
            // TBD - not very useful, check if we want to support it
            "select $ from tgtds where $ = '{\"b\":\"hi\",\"c\":1}'",
            vec!["{\"b\":\"hi\",\"c\":1}"],
        ),
        // 4 - Aliases
        (
            "select tgtds.$.b from tgtds where tgtds.$.c = 1",
            vec!["hi"],
        ),
        ("select m.$.b from tgtds m where m.$.c = 1", vec!["hi"]),
        ("select m.$.b from tgtds m where tgtds.$.c = 1", vec!["hi"]),
        // 5 - Joins
        (
            "select d.$.a, t.$.b from myds d, tgtds t where d.$.a = t.$.c",
            vec!["2, there"],
        ),
        (
            "select d.$.a, t.$.b from myds d, tgtds t",
            vec![
                "2, null", "2, hi", "2, there", "3, null", "3, hi", "3, there",
            ],
        ),
        (
            "select d.$.a, t.$.b from myds d, tgtds t where t.$.b = 'hi'",
            vec!["2, hi", "3, hi"],
        ),
        (
            "select d.$.a, t.$.b from myds d, tgtds t where d.$.a = 2",
            vec!["2, null", "2, hi", "2, there"],
        ),
        ("select i[0] from jpth", vec!["{\"a\":1}"]),
        ("select i[0,1] from jpth", vec!["[{\"a\":1},{\"a\":2}]"]),
        ("select i[*] from jpth", vec!["[{\"a\":1},{\"a\":2}]"]),
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
fn test_filter_stmt_exec() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset jpth",
        "insert into jpth values '{
           \"int\": [{\"a\": 1}, {\"a\": 2}],
           \"str\": {\"b\": \"hi\"},
           \"bl\": {\"c\": true},
           \"nl\": {\"d\": null},
           \"num\": {\"e\": 12.34}
         }'",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Statement, expected rows
        ("select int[0,1]?(@.a >= 2) from jpth", vec!["{\"a\":2}"]),
        ("select str[*]?(@.b == \"hi\") from jpth", vec!["{\"b\":\"hi\"}"]),
        ("select bl[*]?(@.c == true) from jpth", vec!["{\"c\":true}"]),
        ("select nl[*]?(@.d == null) from jpth", vec!["{\"d\":null}"]),
        ("select num[*]?(@.e >= 12) from jpth", vec!["{\"e\":12.34}"]),
        // Doesn't work with jbparse yet
        // ("select num[*]?(@.e >= 12.1) from jpth", vec!["{\"e\":12.34}"]),
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
fn test_invalid_jsonpath() {
    let mut ctxt = demoexe::DemoContextT::new();
    let stmt = "create dataset jpth";
    let _ = sqlcontrols::stmt_exec(&mut ctxt, stmt);

    let stmt = "select $.*?(@.b == 12.34) from jpth";
    let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
    assert!(res.is_err(), "Expected error for '{}', but got {:?}", stmt, res.ok());
    println!("test_invalid_jsonpath - executing '{}', got res: {:?}", stmt, res);

    let err = res.err().unwrap();
    assert!(matches!(err, SqlExecError::TranslateError(_)), "Expected SqlTranslateError error for '{}', but got {:?}", stmt, err);
}