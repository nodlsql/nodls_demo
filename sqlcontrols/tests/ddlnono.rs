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
use sqloptimize::utils::SqlTranslateError;

#[test]
fn test_ddl_translate_err() {
    // Inits
    let mut ctxt = demoexe::DemoContextT::new();
    for (inits, stmt, errtype) in [
        // inits, statement, expected
        (
            // 1. Target ds not found
            vec![],
            "create dataset myds relationship rs(tgtds)",
            SqlTranslateError::DatasetNotFound("tgtds".to_string()),
        ),
        (
            // 2. Primary key not found for target ds
            vec!["create dataset tgtds"],
            "create dataset myds relationship rs(tgtds)",
            SqlTranslateError::PrimaryKeyNotFound("tgtds".to_string()),
        ),
    ] {
        println!("test_ddl_translate_err - executing '{}'", stmt);
        for init in inits {
            let res = sqlcontrols::stmt_exec(&mut ctxt, init);
            assert!(res.is_ok(), "test_ddl_translate_err - executing '{}'", init);
        }
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        match res {
            Ok(_) => panic!("test_ddl_translate_err - executed '{}'", stmt),
            Err(e) => match e {
                SqlExecError::TranslateError(t) => {
                    println!(
                        "test_ddl_translate_err - error executing '{}': {:?}",
                        stmt, t
                    );
                    assert_eq!(t, errtype);
                }
                _ => {
                    panic!(
                        "test_ddl_translate_err - error executing '{}': {:?}",
                        stmt, e
                    );
                }
            },
        }
    }
}

#[test]
fn test_ddl_exec_err() {
    for (inits, stmt, errtype) in [
        // inits, statement, expected
        (
            // 1. Rel already exists
            vec![
                "create dataset tgtds primary key(a)",
                "create dataset myds relationship rs(tgtds)",
            ],
            "alter dataset myds add relationship rs(tgtds)",
            "Relationship rs already exists",
        ),
        (
            // 2. Rel not found
            vec![
                "create dataset tgtds primary key(a)",
                "create dataset myds relationship rs(tgtds)",
            ],
            "alter dataset myds drop relationship nors",
            "Failed to find rel 'nors' in dataset 'myds'",
        ),
         (
            // 3. Name collision with dataset name 
            vec![
                "create dataset tgtds primary key(a)",
                "create dataset myds",
            ],
            "alter dataset myds add relationship myds(tgtds)",
            "Relationship name 'myds' collides with dataset name",
        ),
    ] {
        // Inits
        let mut ctxt = demoexe::DemoContextT::new();
        println!("test_ddl_exec_err - executing '{}'", stmt);
        for init in inits {
            let res = sqlcontrols::stmt_exec(&mut ctxt, init);
            assert!(res.is_ok(), "test_ddl_exec_err - executing '{}'", init);
        }
        let res = sqlcontrols::stmt_exec(&mut ctxt, stmt);
        match res {
            Ok(_) => panic!("test_ddl_exec_err - executed '{}'", stmt),
            Err(e) => match e {
                SqlExecError::ExecutionError(t) => {
                    println!("test_ddl_exec_err - executing '{}': {:?}", stmt, t);
                    assert_eq!(t, errtype);
                }
                _ => {
                    panic!("test_ddl_exec_err - error executing '{}': {:?}", stmt, e);
                }
            },
        }
    }
}
