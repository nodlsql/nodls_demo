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
        "create dataset myds index dix(a)",
        "insert into myds values '{\"a\": 1}'",
        "insert into myds values '{\"a\": 2, \"b\": 20}'",
        "insert into myds values '{\"a\": 2, \"b\": 21}'",
        "insert into myds values '{\"a\": 3}'",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Update statements, select statement, expected rows selected
        (
            vec![],
            "select * from myds where a > 1",
            vec!["{\"a\":2,\"b\":20}", "{\"a\":2,\"b\":21}", "{\"a\":3}"],
        ),
        (
            vec!["delete from myds where a = 1"],
            "select * from myds where a > 1",
            vec!["{\"a\":2,\"b\":20}", "{\"a\":2,\"b\":21}", "{\"a\":3}"],
        ),
        (
            vec!["delete from myds where a = 2 and b = 20"],
            "select * from myds where a > 1",
            vec!["{\"a\":2,\"b\":21}", "{\"a\":3}"],
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
