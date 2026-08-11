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
fn test_isnull_stmt_exe() {
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
        // select statement, expected rows selected
        // 1 - No index usage
        (
            "select * from myds where c = null",
            vec![
                "{\"b\":\"there\",\"c\":null}",
            ],
        ),
        (
            "select * from myds where c <> null",
            vec![
                "{\"b\":null}",
                "{\"c\":11}",
            ],
        ),
        (
            "select * from myds where c is null",
            vec![
                "{\"b\":null}",
            ],
        ),
        (
            "select * from myds where c is not null",
            vec![
                "{\"b\":\"there\",\"c\":null}",
                "{\"c\":11}",
            ],
        ),
        // 2 - Indexed segment usage
        (
            "select * from myds where b = null",
            vec![
                "{\"b\":null}",
            ],
        ),
        (
            "select * from myds where b <> null",
            vec![
                "{\"b\":\"there\",\"c\":null}",
                "{\"c\":11}",
            ],
        ),
        (
            "select * from myds where b is null",
            vec![
                "{\"c\":11}",
            ],
        ),
        (
            "select * from myds where b is not null",
            vec![
                "{\"b\":null}",
                "{\"b\":\"there\",\"c\":null}",
            ],
        ),
    ];
    let mut test_id = 0;
    for (stmt, expected_rows) in suite {
        println!(
            "test_isnull_stmt_exe: test {}: then '{}'",
            test_id, stmt
        );
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        assert!(res.is_ok(), "Failed to execute '{}': {:?}", stmt, res.err());
        let rows = res.unwrap();
        assert_eq!(rows, expected_rows, "Unexpected result for '{}'", stmt);
        test_id += 1;
    }
}
