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
use rust_decimal::Decimal;
use sqlcontrols::utils;
use sqlexet::SqlExeTrait;
use std::{ffi::c_uint, i64};

use sqlinsts::{sql_value_pb::Data, CompOperatorPb, SqlValuePb};
use sqloptimize::utils::pretty_print_plan;

#[test]
fn test_translate_sql_stmt() {
    // Inits
    let dataset_desc = jbparse::DatasetDesc {
        name: "job".to_string(),
        _id: 10001,
        rels: vec![],
        indexes: vec![],
    };
    let json_value = jsonb::to_owned_jsonb(&dataset_desc).unwrap();
    let data_binary = json_value.to_vec();
    let data_size: c_uint = data_binary.len() as c_uint;

    // Create dataset descriptor in demo context
    let mut ctxt = demoexe::DemoContextT::new();
    ctxt.write_dataset(0, 4195, "job", &data_binary, data_size);

    // Generate AST from SQL statement
    let input = "select vibe from job where vibe.mood='good'";
    let res = sqlparser::parse_stmt(input).unwrap();
    if let sqlparser::ast::SqlStmt::Select(_) = res {
        // expected
        println!("Parsed AST: {}", res.print_tree());
    } else {
        panic!("Expected a SELECT statement");
    }
    let mut stmt = res;

    // Translate AST to SQL plan
    let sqlplan = sqlplan::translate(&mut ctxt, &mut stmt).unwrap();

    let s = pretty_print_plan(&sqlplan);
    let lines: Vec<&str> = s.lines().collect();
    assert_eq!(lines.len(), 9, "expected exactly one line per inst/value");
    assert!(lines[0].contains("IDataset"));
    assert!(lines[1].contains("IDatapath"));
    assert!(lines[2].contains("ICompare"));
    assert!(lines[3].contains("IDatapath"));
    assert!(lines[4].contains("IProj"));
    assert!(lines[5].contains("val[0]: None"));
    assert!(lines[6].contains("val[1]: None"));
    assert!(lines[7].contains("val[2]: String(\"good\")"));
    assert!(lines[8].contains("val[3]: None"));
}

#[test]
fn test_compare_values() {
    let val1 = SqlValuePb {
        data: Some(Data::Int64Value(10)),
        is_constant: true,
    };
    let val2 = SqlValuePb {
        data: Some(Data::Int64Value(20)),
        is_constant: true,
    };
    let val3 = SqlValuePb {
        data: Some(Data::Int64Value(10)),
        is_constant: true,
    };
    let val_none = SqlValuePb {
        data: None,
        is_constant: true,
    };
    let val_null = SqlValuePb {
        data: Some(Data::NullValue(true)),
        is_constant: true,
    };

    assert!(utils::compare_values(&val1, &val2, CompOperatorPb::Lt));
    assert!(utils::compare_values(&val1, &val3, CompOperatorPb::Eq));
    assert!(utils::compare_values(&val1, &val_none, CompOperatorPb::Gt));
    assert!(utils::compare_values(&val_none, &val1, CompOperatorPb::Lt));
    assert!(utils::compare_values(
        &val_none,
        &val_none,
        CompOperatorPb::Eq
    ));
    assert!(utils::compare_values(&val_null, &val1, CompOperatorPb::Lt));
    assert!(utils::compare_values(
        &val_null,
        &val_null,
        CompOperatorPb::Eq
    ));
    assert!(utils::compare_values(
        &val_none,
        &val_null,
        CompOperatorPb::Lt
    ));
    assert!(utils::compare_values(
        &val_null,
        &val_none,
        CompOperatorPb::Gt
    ));

    assert!(utils::compare_values(&val1, &val_none, CompOperatorPb::Ne));
    assert!(utils::compare_values(&val_none, &val1, CompOperatorPb::Ne));
    assert!(!utils::compare_values(
        &val_none,
        &val_none,
        CompOperatorPb::Ne
    ));
    assert!(!utils::compare_values(
        &val_null,
        &val_null,
        CompOperatorPb::Ne
    ));
    assert!(utils::compare_values(
        &val_none,
        &val_null,
        CompOperatorPb::Ne
    ));
    assert!(utils::compare_values(
        &val_null,
        &val_none,
        CompOperatorPb::Ne
    ));
}

#[test]
fn test_decimal_ops() {
    let test_cases = vec![
        // d1, d2, op, expected result
        (
            Decimal::new(12345, 2),
            Decimal::new(67890, 3),
            sqlinsts::OperPb::Add,
            Decimal::new(191340, 3),
        ), // 123.45 and 67.890
        (
            Decimal::new(-12345, 2),
            Decimal::new(67890, 3),
            sqlinsts::OperPb::Add,
            Decimal::new(-55560, 3),
        ), // negative and positive decimal
        (
            Decimal::new(-12345, 2),
            Decimal::new(67890, 3),
            sqlinsts::OperPb::Div,
            Decimal::new(-1818382677861246133, 18),
        ), // division result -1.818382677861246133
    ];
    for (d1, d2, op, expected) in test_cases {
        let val1 = SqlValuePb {
            data: Some(Data::DecimalValue(sqlinsts::DecimalValuePb {
                number: d1.mantissa() as i64,
                scale: d1.scale() as u32,
            })),
            is_constant: true,
        };
        let val2 = SqlValuePb {
            data: Some(Data::DecimalValue(sqlinsts::DecimalValuePb {
                number: d2.mantissa() as i64,
                scale: d2.scale() as u32,
            })),
            is_constant: true,
        };
        let result = utils::evaluate_expr(&val1, &val2, op);
        if let Data::DecimalValue(res) = result {
            println!(
                "Result for {:?} {:?} {:?}: number: {}, scale: {}",
                d1, d2, op, res.number, res.scale
            );
            let res_decimal = Decimal::new(res.number as i64, res.scale as u32);
            assert_eq!(
                res_decimal, expected,
                "Unexpected result for {:?} {:?} {:?}",
                d1, d2, op
            );
        } else {
            panic!(
                "Expected a DecimalValue result for {:?} {:?} {:?}",
                d1, d2, op
            );
        }
    }
}

#[test]
fn test_overflow_ops() {
    let val1 = SqlValuePb {
        data: Some(Data::Int64Value(i64::MAX)),
        is_constant: true,
    };
    let val2 = SqlValuePb {
        data: Some(Data::Int64Value(1)),
        is_constant: true,
    };
    let result = utils::evaluate_expr(&val1, &val2, sqlinsts::OperPb::Add);
    if let Data::NullValue(_) = result {
        // Expected a NullValue result for overflow add
    } else {
        panic!("Expected a NullValue result for overflow add");
    }
    let val2 = SqlValuePb {
        data: Some(Data::DecimalValue(sqlinsts::DecimalValuePb {
            number: i64::MAX,
            scale: 0,
        })),
        is_constant: true,
    };
    let result = utils::evaluate_expr(&val1, &val2, sqlinsts::OperPb::Add);
    if let Data::NullValue(_) = result {
        // Expected a NullValue result for overflow add
    } else {
        panic!("Expected a NullValue result for overflow add");
    }
    let result = utils::evaluate_expr(&val2, &val1, sqlinsts::OperPb::Add);
    if let Data::NullValue(_) = result {
        // Expected a NullValue result for overflow add
    } else {
        panic!("Expected a NullValue result for overflow add");
    }
}

#[test]
fn test_string_concat() {
    use sqlinsts::sql_value_pb::Data;
    use sqlinsts::SqlValuePb;

    let test_data = vec![
       (Data::StringValue("Hello".to_string()), Data::StringValue(" world!".to_string()), "Hello world!".to_string()),
        (Data::StringValue("Hello".to_string()), Data::Int64Value(123), "Hello123".to_string()),
        (Data::StringValue("Hello".to_string()), Data::DecimalValue(sqlinsts::DecimalValuePb { number: 12345, scale: 2 }), "Hello123.45".to_string()),
        (Data::Int64Value(123), Data::StringValue(" world!".to_string()), "123 world!".to_string()),
        (Data::DecimalValue(sqlinsts::DecimalValuePb { number: 12345, scale: 2 }), Data::StringValue(" world!".to_string()), "123.45 world!".to_string()),
       (Data::StringValue("Hello ".to_string()), Data::BoolValue(true), "Hello true".to_string()),
       (Data::StringValue("Hello ".to_string()), Data::BoolValue(false), "Hello false".to_string()),
       (Data::StringValue("Hello ".to_string()), Data::NullValue(true), "Hello null".to_string()),
    ];

    for (d1, d2, expected) in test_data {
        let val1 = SqlValuePb {
            data: Some(d1),
            is_constant: true,
        };
        let val2 = SqlValuePb {
            data: Some(d2),
            is_constant: true,
        };
        let result = utils::evaluate_expr(&val1, &val2, sqlinsts::OperPb::Add);
        if let Data::StringValue(s) = result {
            assert_eq!(s, expected, "Unexpected string concatenation result");
        } else {
            panic!("Expected a StringValue for: {:?} + {:?} got: {:?}", val1.data, val2.data, result);
        }
    }
}
