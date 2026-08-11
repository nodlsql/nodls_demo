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

use bytes::Bytes;
use prost::Message;
use sqlinsts;
use sqlinsts::SqlStmtPb;
use sqloptimize::utils::pretty_print_plan;
use sqlparser::ast;

#[test]
fn test_encode_idataset() {
    let msg = sqlinsts::IDatasetPb {
        dataset_id: 42,
        key_val_idx: 7,
        name: "a.b.c".to_string(),
    };
    println!("IDatasetPb: {:?}", msg);

    // Serialize the message
    let mut buf = Vec::new();
    msg.encode(&mut buf).unwrap();

    println!("Serialized: {:?}", buf);

    // Deserialize the message into the generated concrete type
    let decoded = sqlinsts::IDatasetPb::decode(Bytes::copy_from_slice(&buf)).unwrap();
    println!("Deserialized: {:?}", decoded);
}

#[test]
fn test_encode_sqlplan() {
    //encode_icompare();
    let icomp = sqlinsts::IComparePb {
        comp: sqlinsts::CompOperatorPb::Eq as i32,
        left_val_idx: 11,
        right_val_idx: 22,
        right_val_cnt: 1,
    };
    let inst = sqlinsts::SqlInstPb {
        inst: Some(sqlinsts::sql_inst_pb::Inst::Comp(icomp)),
    };
    let val = sqlinsts::SqlValuePb {
        is_constant: true,
        data: Some(sqlinsts::sql_value_pb::Data::Int64Value(42)),
    };

    let mut sqlplan = sqlinsts::SqlPlanPb {
        sqlstmt: SqlStmtPb::Select.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 1,
    };
    sqlplan.insts.push(inst);
    sqlplan.values.push(val);
    println!(
        "SqlPlanPb with 1 IComparePb in values, max_value_idx={}, plan={:?}",
        sqlplan.max_value_idx, sqlplan
    );

    // Serialize the message
    let mut buf = Vec::new();
    sqlplan.encode(&mut buf).unwrap();

    println!("Serialized: {:?}", buf);

    // Deserialize the message into the generated concrete type
    let decoded = sqlinsts::SqlPlanPb::decode(Bytes::copy_from_slice(&buf)).unwrap();
    println!("Deserialized: {:?}", decoded);
}

#[test]
fn test_pretty_print_sqlplan() {
    // Build a tiny plan with one instruction and one value
    let icomp = sqlinsts::IComparePb {
        comp: sqlinsts::CompOperatorPb::Eq as i32,
        left_val_idx: 1,
        right_val_idx: 2,
        right_val_cnt: 1,
    };
    let inst = sqlinsts::SqlInstPb {
        inst: Some(sqlinsts::sql_inst_pb::Inst::Comp(icomp)),
    };
    let val = sqlinsts::SqlValuePb {
        is_constant: true,
        data: Some(sqlinsts::sql_value_pb::Data::Int64Value(42)),
    };

    let plan = sqlinsts::SqlPlanPb {
        sqlstmt: SqlStmtPb::Select.into(),
        insts: vec![inst],
        values: vec![val],
        max_value_idx: 2,
    };

    let s = pretty_print_plan(&plan);
    println!("Pretty-printed plan:\n{}", s);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 2, "expected exactly one line per inst/value");
    assert!(
        lines[0].starts_with("    "),
        "first line should be indented by 4 spaces"
    );
    assert!(lines[0].contains("ICompare"));
    assert!(lines[1].starts_with("    "));
    assert!(lines[1].contains("val[0]: Int64(42)"));
    println!("Pretty-printed plan:\n{}", s);
}

#[test]
fn test_translate_expr() {
    let term_expr = ast::Member {
        part: ast::MemberPart::Tree(
            Box::new(ast::Member {
                part: ast::MemberPart::Value(ast::ConstValue::Number("10".to_string())),
            }),
            ast::ArithOperator::Plus,
            Box::new(ast::Member {
                part: ast::MemberPart::Path(ast::PathSegments {
                    segments: vec![ast::PathSegment {
                        name: "a".to_string(),
                        target_ds: "".to_string(),
                    }],
                    jsonpath: vec![],
                }),
            }),
        ),
    };
    let const_expr = ast::Member {
        part: ast::MemberPart::Value(ast::ConstValue::Number("20".to_string())),
    };
    let mut pred_list = vec![ast::Predicate {
        left: term_expr,
        comp_operator: ast::CompOperator::Eq,
        right: const_expr,
    }];
    let mut sqlplan = sqlinsts::SqlPlanPb {
        sqlstmt: SqlStmtPb::Select.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 0,
    };
    // Use the dataset_id if available
    let idataset = sqlinsts::IDatasetPb {
        name: "test_dataset".to_string(),
        dataset_id: 111,
        key_val_idx: 0,
    };
    let inst = sqlinsts::SqlInstPb {
        inst: Some(sqlinsts::sql_inst_pb::Inst::Dataset(idataset)),
    };
    sqlplan.insts.push(inst);

    let from_list = vec![ast::FromListItem {
        ds_name: "a".to_string(),
        alias: None,
    }];

    let res = sqlplan::translate_predicates(&mut sqlplan, &mut pred_list, &from_list);

    let ast_sql_stmt = ast::SqlStmt::Select(ast::SelectStmt {
        proj_list: vec![],
        from_list: from_list,
        predicate_list: pred_list,
    });
    println!("Predicate ast:\n{}", ast_sql_stmt.print_tree());
    println!("Translated SQL plan:\n{}", pretty_print_plan(&sqlplan));
    println!("Translation result: {:?}", res);

    // Expected:
    //    IDataset name=test_dataset key_val_idx=0 dataset_id=111
    //    IDatapath pathstr=a jpath=[] parent_path=["a"] ds_name=a alias= key_val_idx=-1 val_idx=1 phase=0 rels=[]
    //    IExpr op=0 lval=0 rval=1 resval=2
    //    ICompare comp=0 left_val_idx=2 right_val_idx=3
    //    val[0]: Int64(10)
    //    val[1]: Placeholder
    //    val[2]: Placeholder
    //    val[3]: Int64(20)
    assert_eq!(
        sqlplan.insts.len(),
        4,
        "expected 4 instructions in the plan"
    );
    assert!(matches!(
        sqlplan.insts[0].inst,
        Some(sqlinsts::sql_inst_pb::Inst::Dataset(_))
    ));
    assert!(matches!(
        sqlplan.insts[1].inst,
        Some(sqlinsts::sql_inst_pb::Inst::Dpath(_))
    ));
    assert!(matches!(
        sqlplan.insts[3].inst,
        Some(sqlinsts::sql_inst_pb::Inst::Comp(_))
    ));
    assert_eq!(sqlplan.values.len(), 4, "expected 4 values in the plan");
    assert!(matches!(
        sqlplan.values[0].data,
        Some(sqlinsts::sql_value_pb::Data::Int64Value(10))
    ));
    assert!(matches!(sqlplan.values[1].data, None));
    assert!(matches!(sqlplan.values[2].data, None));
    assert!(matches!(
        sqlplan.values[3].data,
        Some(sqlinsts::sql_value_pb::Data::Int64Value(20))
    ));
}
