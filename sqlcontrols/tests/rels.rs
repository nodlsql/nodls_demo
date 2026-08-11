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
fn test_rel_stmt_exe() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for stmt in [
        "create dataset job",
        "create dataset tgtds primary key(b,c), relationship trs(myds)",
        "create dataset myds primary key(a), relationship rs(tgtds)",
        "insert into myds values '{\"a\": 2}'",
        "insert into myds values '{\"a\": 3}'",
        "insert into tgtds values '{\"b\": \"hi\", \"c\": 1}'",
        "insert into tgtds values '{\"b\": \"there\", \"c\": 2}'",
        "insert into myds.rs values ('hi', 1) where a = 2",
        "insert into tgtds.trs values (2) where b = 'hi'",
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
        // Show rel summary with '*' projection
        (
            "select *, trs.* from tgtds",
            vec![
                "{\"b\":\"hi\",\"c\":1,\"trs\":2}, {\"a\":2,\"rs\":\"hi 1\"}",
                "{\"b\":\"there\",\"c\":2}, null",
            ],
        ),
        // Skip rel summary
        (
            "select t, t.trs from tgtds t",
            vec![
                "{\"b\":\"hi\",\"c\":1}, {\"a\":2}",
                "{\"b\":\"there\",\"c\":2}, null",
            ],
        ),
        // Show rel summary with inverse '*' projection
        (
            "select *, inverse(myds.rs).* from tgtds",
            vec![
                "{\"b\":\"hi\",\"c\":1,\"trs\":2}, {\"a\":2,\"rs\":\"hi 1\"}",
                "{\"b\":\"there\",\"c\":2}, null",
            ],
        ),
        // Inverse going back to self
        (
            "select *, inverse(myds.rs).rs.* from tgtds",
            vec![
                "{\"b\":\"hi\",\"c\":1,\"trs\":2}, {\"b\":\"hi\",\"c\":1,\"trs\":2}",
                "{\"b\":\"there\",\"c\":2}, null",
            ],
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
