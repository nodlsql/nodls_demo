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
fn test_delete_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset myds relationship rs(tgtds)",
        "create dataset tgtds primary key(b,c)",
        "insert into myds values '{\"a\": 1}'",
        "insert into myds values '{\"a\": 2}'",
        "insert into tgtds values '{}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 11}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 12}'",
        "insert into myds.rs values null where a = 1",
        "insert into myds.rs values ('hi', 11) where a = 1",
        "insert into myds.rs values ('there', 12) where a = 2",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Update statements, select statement, expected rows selected
        // 1 - Initial rel
        (
            vec![],
            "select rs.* from myds",
            vec![
                "{}",
                "{\"b\":\"hi\",\"c\":11}",
                "{\"b\":\"there\",\"c\":12}",
            ],
        ),
        // 2 - Initial inverse rel
        (
            vec![],
            "select *, inverse(myds.rs).* from tgtds",
            vec![
                "{}, {\"a\":1,\"rs\":[\"null null\",\"hi 11\"]}",
                "{\"b\":\"hi\",\"c\":11}, {\"a\":1,\"rs\":[\"null null\",\"hi 11\"]}",
                "{\"b\":\"there\",\"c\":12}, {\"a\":2,\"rs\":\"there 12\"}",
            ],
        ),
        // 3.1 - Delete item a = 1
        (
            vec!["delete from myds where a = 1"],
            "select * from myds",
            vec!["{\"a\":2,\"rs\":\"there 12\"}"],
        ),
        // 3.2 - Verify the inverse rels have been updated
        (
            vec![],
            "select *, inverse(myds.rs).* from tgtds",
            vec![
                "{}, null",
                "{\"b\":\"hi\",\"c\":11}, null",
                "{\"b\":\"there\",\"c\":12}, {\"a\":2,\"rs\":\"there 12\"}",
            ],
        ),
        // 4 - Index delete test - delete tgtds item b = 'hi'
        (
            vec!["delete from tgtds where b = 'hi'"],
            "select * from tgtds where b > 'h'",
            vec!["{\"b\":\"there\",\"c\":12}"],
        ),
    ];
    let mut test_id = 0;
    for (upd_stmts, stmt, expected_rows) in suite {
        println!(
            "test_delete_stmt_exe: test {}: updates: {:?}, then '{}'",
            test_id, upd_stmts, stmt
        );
        for upd_stmt in upd_stmts {
            let res = sqlcontrols::stmt_exec(&mut ctxt, upd_stmt);
            assert!(
                res.is_ok(),
                "Failed to execute '{}': {:?}",
                upd_stmt,
                res.err()
            );
        }
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
        test_id += 1;
    }
}

#[test]
fn test_delete_rel_stmt_exe() {
    tracing_subscriber::fmt::init();
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset myds relationship rs(tgtds)",
        "create dataset tgtds primary key(b,c)",
        "insert into myds values '{\"a\": 1}'",
        "insert into myds values '{\"a\": 2}'",
        "insert into tgtds values '{}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 11}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 12}'",
        "insert into myds.rs values null where a = 1",
        "insert into myds.rs values ('hi', 11) where a = 1",
        "insert into myds.rs values ('there', 12) where a = 2",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Update statements, select statement, expected rows selected
        // 1 - Initial rel
        (
            vec![],
            "select rs.* from myds",
            vec![
                "{}",
                "{\"b\":\"hi\",\"c\":11}",
                "{\"b\":\"there\",\"c\":12}",
            ],
        ),
        (
            vec![],
            "select * from myds",
            vec![
                "{\"a\":1,\"rs\":[\"null null\",\"hi 11\"]}",
                "{\"a\":2,\"rs\":\"there 12\"}",
            ],
        ),
        // 2 - Initial inverse rel
        (
            vec![],
            "select *, inverse(myds.rs).* from tgtds",
            vec![
                "{}, {\"a\":1,\"rs\":[\"null null\",\"hi 11\"]}",
                "{\"b\":\"hi\",\"c\":11}, {\"a\":1,\"rs\":[\"null null\",\"hi 11\"]}",
                "{\"b\":\"there\",\"c\":12}, {\"a\":2,\"rs\":\"there 12\"}",
            ],
        ),
        // 3.1 - Delete successor rel c = 11
        (
            vec!["delete from myds.rs values ('hi', 11) where a = 1"],
            "select * from myds",
            vec![
                "{\"a\":1,\"rs\":\"null null\"}", // 'hi', 11 rel deleted
                "{\"a\":2,\"rs\":\"there 12\"}",
            ],
        ),
        (
            vec![],
            "select *, inverse(myds.rs).* from tgtds",
            vec![
                "{}, {\"a\":1,\"rs\":\"null null\"}",
                "{\"b\":\"hi\",\"c\":11}, null",            // inverse 'hi', 11 deleted
                "{\"b\":\"there\",\"c\":12}, {\"a\":2,\"rs\":\"there 12\"}",
            ],
        ),
    ];
    let mut test_id = 0;
    for (upd_stmts, stmt, expected_rows) in suite {
        println!(
            "test_delete_rel: test {}: updates: {:?}, then '{}'",
            test_id, upd_stmts, stmt
        );
        for upd_stmt in upd_stmts {
            let res = sqlcontrols::stmt_exec(&mut ctxt, upd_stmt);
            assert!(
                res.is_ok(),
                "Failed to execute '{}': {:?}",
                upd_stmt,
                res.err()
            );
        }
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
        test_id += 1;
    }
}
