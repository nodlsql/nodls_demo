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

use sqlparser::ast;
use sqlparser::{parse_stmt, SqlParseError};

#[test]
fn test_jpath_expressions() {
    let tests = [
        // query, expected path, expected jsonpath
        ("select a.b from y", "a.b", ""),
        ("select $.* from y", "", "$.*"),
        ("select $.a from y", "", "$.a"),
        ("select a[1] from y", "", "a[1]"),
        ("select a[1,2] from y", "", "a[1,2]"),
    ];
    for (input, expected, jsonpath) in &tests {
        let res = parse_stmt(input);
        assert!(
            res.is_ok(),
            "Jsonpath expressions failed - input: {} error: {:?}",
            input,
            res.err().unwrap()
        );

        let stmt = &res.unwrap();

        // Get projection from select statement
        if let ast::SqlStmt::Select(select_stmt) = stmt {
            let projection = &select_stmt.proj_list;
            for member in projection {
                if let ast::MemberPart::Path(path_segs) = &member.part {
                    let path: String = path_segs
                        .segments
                        .iter()
                        .map(|seg| seg.name.clone())
                        .collect::<Vec<String>>()
                        .join(".");
                    let json_path: String = path_segs.jsonpath.join(".");
                    println!(
                        "Jsonpath expressions - input: '{}' path: '{}' jsonpath: '{}'",
                        input, path, json_path
                    );
                    assert_eq!(json_path, *jsonpath);
                    assert_eq!(path, *expected);
                }
            }
        } else {
            panic!("Expected a select statement, but got: {:?}", stmt);
        }
    }
}
