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
fn test_update_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset tgtds primary key(b,c)",
        "insert into tgtds values '{}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 11}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 12}'",
    ] {
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
    }
    let suite = vec![
        // Update statements, select statement, expected rows selected
        // 1 - Initial content
        (
            vec![],
            "select * from tgtds",
            vec![
                "{}",
                "{\"b\":\"hi\",\"c\":11}",
                "{\"b\":\"there\",\"c\":12}",
            ],
        ),
        // 2 - Update
        (
            vec!["update tgtds set c = 22, b = 'hii' where c = 11"],
            "select * from tgtds",
            vec![
                "{}",
                "{\"b\":\"there\",\"c\":12}",
                "{\"b\":\"hii\",\"c\":22}",
            ],
        ),
        // Check index scan still works
        (
            vec![],
            "select * from tgtds where b > 'a'",
            vec!["{\"b\":\"hii\",\"c\":22}", "{\"b\":\"there\",\"c\":12}"], // reordered by index scan
        ),
    ];
    let mut test_id = 0;
    for (upd_stmts, stmt, expected_rows) in suite {
        println!(
            "test_update_stmt_exe: test {}: updates: {:?}, then '{}'",
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
