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
fn test_inpredicate_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset myds primary key(b)",
        "insert into myds values '{\"b\": null}'",
        "insert into myds values '{\"b\": \"there\", \"c\": null}'",
        "insert into myds values '{\"c\": 11}'",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Statement, expected rows
        // No index
        (
            "select * from myds where c in (11, null)",
            vec!["{\"b\":\"there\",\"c\":null}", "{\"c\":11}"],
        ),
        (
            "select * from myds where c not in (11, null)",
            vec!["{\"b\":null}"],
        ),
        (
            "select * from myds where c in (null)",
            vec!["{\"b\":\"there\",\"c\":null}"],
        ),
        (
            "select * from myds where c not in (null)",
            vec!["{\"b\":null}", "{\"c\":11}"],
        ),
        // With index
        (
            "select * from myds where b in ('there', null)",
            vec!["{\"b\":\"there\",\"c\":null}", "{\"b\":null}"],
        ),
        (
            "select * from myds where b not in ('there', null)",
            vec!["{\"c\":11}"],
        ),
        (
            "select * from myds where b in ('there')",
            vec!["{\"b\":\"there\",\"c\":null}"],
        ),
        (
            "select * from myds where b not in ('there')",
            vec!["{\"b\":null}", "{\"c\":11}"],
        ),
    ];
    for (stmt, expected_rows) in suite {
        println!("test_inpredicate_stmt_exe - executing '{}'", stmt);
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
    }
}

#[test]
fn test_regexp_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset myds primary key(b)",
        "insert into myds values '{\"b\": \"there\"}'",
        "insert into myds values '{\"b\": \"th.*at\"}'",
        "insert into myds values '{\"c\": 11}'",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Statement, expected rows
        (
            "select * from myds where b like '%re'",
            vec!["{\"b\":\"there\"}"],
        ),
        (
            "select * from myds where b not like 'th_re'",
            vec!["{\"b\":\"th.*at\"}", "{\"c\":11}"],
        ),
        (
            "select * from myds where b regexp 'th.*re'",
            vec!["{\"b\":\"there\"}"],
        ),
        (
            "select * from myds where b not regexp 'th.*re'",
            vec!["{\"b\":\"th.*at\"}", "{\"c\":11}"],
        ),
        (
            "select * from myds where b regexp 'that'",
            vec![],
        ),
        (
            "select * from myds where c regexp '11'",
            vec![],
        ),
    ];
    for (stmt, expected_rows) in suite {
        println!("test_regexp_stmt_exe - executing '{}'", stmt);
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
    }
}
