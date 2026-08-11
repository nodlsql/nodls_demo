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

use indexcapn::indexkey_capnp::{index_page, key_component};
use indexsrch::{compare_keys, ScanOutput, SearchRes};
use rust_decimal::Decimal;
use sqlinsts::{sql_value_pb::Data, CompOperatorPb, DecimalValuePb, SqlValuePb};
use std::cell::RefCell;
use std::cmp::Ordering;

macro_rules! get_index_page_reader {
    ($serialized_page:ident) => {
        // Deserialize the existing index_page
        if let Ok(reader) = ::capnp::serialize::read_message(
            &mut std::io::Cursor::new($serialized_page),
            ::capnp::message::ReaderOptions::new(),
        ) {
            reader
        } else {
            panic!("Failed to read message");
        }
    };
}

#[test]
fn test_compare_str_key() {
    let string_values = vec![
        // sql_value, index_value, expected_result
        ("apple".to_string(), "banana".to_string(), Ordering::Less),
        ("banana".to_string(), "apple".to_string(), Ordering::Greater),
        ("cherry".to_string(), "cherry".to_string(), Ordering::Equal),
    ];
    for (val0, val1, expected) in string_values {
        let sql_val = SqlValuePb {
            data: Some(Data::StringValue(val0)),
            ..Default::default()
        };
        let idx_sql_val = SqlValuePb {
            data: Some(Data::StringValue(val1)),
            ..Default::default()
        };

        // Create a new index_page Leaf message
        let serialized_page = indexkey::create_page(true);
        let serialized_page = match indexkey::insert_key_from_values(
            &serialized_page,
            1,
            &vec![RefCell::new(idx_sql_val)],
            0,
            false,
        ) {
            Ok(buf) => buf,
            Err(e) => panic!("Failed to append key to index node: {:?}", e),
        };

        // Get the node reader and the first entry reader
        let reader = get_index_page_reader!(serialized_page);
        let reader = reader.get_root::<index_page::Reader>().unwrap();
        let entries = indexkey::get_index_key_entries(&reader).unwrap();
        let entry = entries.get(0);
        let components = entry.get_cmpts().unwrap();

        // Compare the keys
        let res = compare_keys(&vec![RefCell::new(sql_val)], &components);
        assert_eq!(res, expected);
    }
}

#[test]
fn test_compare_decimal_key() {
    let string_values = vec![
        // sql_value, index_value, expected_result
        (Decimal::new(1, 0), Decimal::new(2, 0), Ordering::Less),
        (Decimal::new(2, 0), Decimal::new(1, 0), Ordering::Greater),
        (Decimal::new(3, 0), Decimal::new(3, 0), Ordering::Equal),
        (Decimal::new(10, 0), Decimal::new(4, 0), Ordering::Greater),
        (Decimal::new(5, 0), Decimal::new(10, 0), Ordering::Less),
        (Decimal::new(0, 1), Decimal::new(10, 0), Ordering::Less),
    ];
    for (val0, val1, expected) in string_values {
        let number = val0.mantissa() as i64;
        let scale = val0.scale() as u32;
        let sql_val = SqlValuePb {
            data: Some(Data::DecimalValue(DecimalValuePb { number, scale })),
            ..Default::default()
        };
        let number = val1.mantissa() as i64;
        let scale = val1.scale() as u32;
        let idx_sql_val = SqlValuePb {
            data: Some(Data::DecimalValue(DecimalValuePb { number, scale })),
            ..Default::default()
        };
        let sql_cmp_vals = vec![RefCell::new(sql_val)];
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];
        // Create a new index_page Leaf message
        let serialized_page = indexkey::create_page(true);
        let serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, 0, false).unwrap();

        // Get the node reader and the first entry reader
        let reader = get_index_page_reader!(serialized_page);
        let reader = reader.get_root::<index_page::Reader>().unwrap();
        let entries = indexkey::get_index_key_entries(&reader).unwrap();
        let entry = entries.get(0);
        let components = entry.get_cmpts().unwrap();

        // Compare the keys
        let res = compare_keys(&sql_cmp_vals, &components);
        assert_eq!(res, expected);
    }
}
#[test]
fn test_compare_compound_key() {
    let string_values = vec![
        // sql_value, index_value, expected_result
        (111, 222, Ordering::Less),
        (222, 111, Ordering::Greater),
        (333, 333, Ordering::Equal),
    ];
    for (val0, val1, expected) in string_values {
        let sql_head = SqlValuePb {
            data: Some(Data::StringValue("fixed".to_string())),
            is_constant: false,
        };
        let sql_val = SqlValuePb {
            data: Some(Data::Int64Value(val0)),
            ..Default::default()
        };
        let idx_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(val1)),
            ..Default::default()
        };
        let sql_cmp_vals = vec![RefCell::new(sql_head.clone()), RefCell::new(sql_val)];
        let idx_sql_vals = vec![RefCell::new(sql_head), RefCell::new(idx_sql_val)];
        // Create a new index_page Leaf message
        let serialized_page = indexkey::create_page(true);
        let serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, 0, false).unwrap();

        // Get the node reader and the first entry reader
        let reader = get_index_page_reader!(serialized_page);
        let reader = reader.get_root::<index_page::Reader>().unwrap();
        let entries = indexkey::get_index_key_entries(&reader).unwrap();
        let entry = entries.get(0);
        let components = entry.get_cmpts().unwrap();

        // Compare the keys
        let res = compare_keys(&sql_cmp_vals, &components);
        assert_eq!(res, expected);
    }
}

#[derive(Debug, Clone)]
enum NullKeyFlag {
    NullKey,
    NotNullKey,
}

#[test]
fn test_compare_null_key() {
    let values = vec![
        // sql_value, index_value, expected_result
        (
            vec![NullKeyFlag::NullKey],
            vec![NullKeyFlag::NotNullKey],
            Ordering::Less,
        ),
        (
            vec![NullKeyFlag::NotNullKey],
            vec![NullKeyFlag::NullKey],
            Ordering::Greater,
        ),
        (
            vec![NullKeyFlag::NullKey],
            vec![NullKeyFlag::NullKey],
            Ordering::Equal,
        ),
    ];
    for (val0, val1, expected) in values {
        let sql_vals = val0
            .clone()
            .into_iter()
            .map(|d| {
                RefCell::new(SqlValuePb {
                    data: match d {
                        NullKeyFlag::NullKey => None,
                        NullKeyFlag::NotNullKey => Some(Data::Int64Value(0)), // Placeholder value
                    },
                    is_constant: false,
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        let idx_sql_vals = val1
            .clone()
            .into_iter()
            .map(|d| {
                RefCell::new(SqlValuePb {
                    data: match d {
                        NullKeyFlag::NullKey => None,
                        NullKeyFlag::NotNullKey => Some(Data::Int64Value(0)), // Placeholder value
                    },
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();

        // Create a new index_page Leaf message
        let serialized_page = indexkey::create_page(true);

        let serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, 0, false).unwrap();

        // Get the node reader and the first entry reader
        let reader = get_index_page_reader!(serialized_page);
        let reader = reader.get_root::<index_page::Reader>().unwrap();
        let entries = indexkey::get_index_key_entries(&reader).unwrap();
        let entry = entries.get(0);
        let components = entry.get_cmpts().unwrap();

        // Compare the keys
        let res = compare_keys(&sql_vals, &components);
        assert_eq!(res, expected);
    }
}

#[test]
fn test_compare_mismatched_key() {
    let values = vec![
        // sql_value, index_value, expected_result
        (
            // Compare string with int
            vec![Data::StringValue("111".to_string())],
            vec![Data::Int64Value(111)],
            Ordering::Greater,
        ),
        (
            // Compare int with string
            vec![Data::Int64Value(111)],
            vec![Data::StringValue("111".to_string())],
            Ordering::Less,
        ),
        (
            // Compare with more values, first matches
            vec![
                Data::StringValue("111".to_string()),
                Data::StringValue("222".to_string()),
            ],
            vec![Data::StringValue("111".to_string())],
            Ordering::Greater,
        ),
    ];
    for (val0, val1, expected) in values {
        let sql_vals = val0
            .clone()
            .into_iter()
            .map(|d| {
                RefCell::new(SqlValuePb {
                    data: Some(d),
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        let idx_sql_vals = val1
            .clone()
            .into_iter()
            .map(|d| {
                RefCell::new(SqlValuePb {
                    data: Some(d),
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();

        // Create a new index_page Leaf message
        let serialized_page = indexkey::create_page(true);
        let serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, 0, false).unwrap();

        // Get the node reader and the first entry reader
        let reader = get_index_page_reader!(serialized_page);
        let reader = reader.get_root::<index_page::Reader>().unwrap();
        let entries = indexkey::get_index_key_entries(&reader).unwrap();
        let entry = entries.get(0);
        let components = entry.get_cmpts().unwrap();

        // Compare the keys
        let res = compare_keys(&sql_vals, &components);
        assert_eq!(res, expected);
    }
}

#[test]
fn test_index_binary_search() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    // Add index entries for the test cases
    let idx_values = vec![111, 222, 333];
    for (i, idxval) in idx_values.iter().enumerate() {
        let idx_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(*idxval)),
            ..Default::default()
        };
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];
        serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, i, false).unwrap();
    }

    // Read back the page
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();
    let sql_values = vec![
        // sql_value, expected_result position
        (111, SearchRes::Found(0)),
        (222, SearchRes::Found(1)),
        (333, SearchRes::Found(2)),
        (0, SearchRes::NotFound(0)),
        (115, SearchRes::NotFound(1)),
        (225, SearchRes::NotFound(2)),
        (400, SearchRes::NotFound(3)),
    ];
    for (sqlval, expected) in sql_values {
        let sql_val = SqlValuePb {
            data: Some(Data::Int64Value(sqlval)),
            ..Default::default()
        };
        let sql_cmp_vals = vec![RefCell::new(sql_val)];
        // Binary search for the key in the index node
        let res = indexsrch::index_binary_search(&entries, &sql_cmp_vals);
        assert_eq!(res, expected);
    }
}

// TBD - likely useless, we use index_iter only in this test, controls uses index_scan
#[test]
pub fn test_index_iter() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    // Add index entries for the test cases
    let idx_values = vec![111, 222, 333];
    for (i, idxval) in idx_values.iter().enumerate() {
        let idx_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(*idxval)),
            ..Default::default()
        };
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];
        serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, i, false).unwrap();
    }

    // Read back the entries
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();

    // -- Iterate at pos 0, should get all entries --
    let mut expected_idx = 0;
    for key_reader in entries {
        let component = key_reader.get_cmpts().unwrap().get(0);
        match component.which().unwrap() {
            key_component::Which::Int64Cpt(int_val) => {
                assert_eq!(int_val, (expected_idx + 1) * 111);
            }
            _ => panic!("Expected an Int64Cpt in the index key"),
        }
        expected_idx += 1;
    }
    assert_eq!(expected_idx, 3); // We should have iterated through all 3 entries

    // -- Iterate at pos 1, should get 222 and 333 --
    let mut expected_idx = 1;
    let mut skip_first = true;
    for key_reader in entries {
        if skip_first {
            skip_first = false;
            continue;
        }
        let component = key_reader.get_cmpts().unwrap().get(0);
        match component.which().unwrap() {
            key_component::Which::Int64Cpt(int_val) => {
                assert_eq!(int_val, (expected_idx + 1) * 111);
            }
            _ => panic!("Expected an Int64Cpt in the index key"),
        }
        expected_idx += 1;
    }
    assert_eq!(expected_idx, 3); // We should have iterated through all 3 entries
}

#[test]
pub fn test_index_scan() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    // Add index entries for the test cases
    let idx_values = vec![111, 222, 333];
    let mut oid_key = 11;
    for (i, idxval) in idx_values.iter().enumerate() {
        let idx_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(*idxval)),
            ..Default::default()
        };
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];
        serialized_page =
            indexkey::insert_key_from_values(&serialized_page, oid_key, &idx_sql_vals, i, false)
                .unwrap();
        oid_key += 1;
    }

    // Test cases:
    // start sqlvalue, start comparison, end sqlvalue, end comparison, expected oids, expected scan state, expected some
    let test_cases = vec![
        // 0 - Start is after end, returns None
        (
            333,
            CompOperatorPb::Ge,
            222,
            CompOperatorPb::Le,
            vec![],
            ScanOutput::Done,
            false,
        ),
        // 1 - Start is beyond the last entry, returns none
        (
            555,
            CompOperatorPb::Ge,
            555,
            CompOperatorPb::Le,
            vec![],
            ScanOutput::Done,
            false,
        ),
        // 2 - LE last elt, state done as we matched the end elt
        (
            333,
            CompOperatorPb::Ge,
            333,
            CompOperatorPb::Le,
            vec![13],
            ScanOutput::Done,
            true,
        ),
        // 3 - LT last elt, returns none
        (
            333,
            CompOperatorPb::Ge,
            333,
            CompOperatorPb::Lt,
            vec![],
            ScanOutput::Done,
            false,
        ),
        // 4 - Before first elt, return all, more output as we don't have a matching end
        (
            0,
            CompOperatorPb::Ge,
            555,
            CompOperatorPb::Lt,
            vec![11, 12, 13],
            ScanOutput::More,
            true,
        ),
        // 5 - GE first elt, return all, more output as we don't have a matching end
        (
            111,
            CompOperatorPb::Ge,
            555,
            CompOperatorPb::Lt,
            vec![11, 12, 13],
            ScanOutput::More,
            true,
        ),
        // 6 - GT first elt, more output as we don't have a matching end
        (
            111,
            CompOperatorPb::Gt,
            555,
            CompOperatorPb::Lt,
            vec![12, 13],
            ScanOutput::More,
            true,
        ),
        // 7 - GE second elt, more output as we don't have a matching end
        (
            222,
            CompOperatorPb::Ge,
            555,
            CompOperatorPb::Lt,
            vec![12, 13],
            ScanOutput::More,
            true,
        ),
        // 8 - Start equi comparison
        (
            111,
            CompOperatorPb::Eq,
            111,
            CompOperatorPb::Eq, // end comparison is ignored
            vec![11],
            ScanOutput::Done,
            true,
        ),
        // 9 - Start equi comparison no match at beginning
        (
            100,
            CompOperatorPb::Eq,
            100,
            CompOperatorPb::Eq, // end comparison is ignored
            vec![],
            ScanOutput::Done,
            false,
        ),
        // 10 - Start equi comparison no match at end
        (
            400,
            CompOperatorPb::Eq,
            400,
            CompOperatorPb::Eq, // end comparison is ignored
            vec![],
            ScanOutput::Done,
            false,
        ),
        // 11 - Start key is empty
        (
            0,
            CompOperatorPb::Ge,
            222,
            CompOperatorPb::Le,
            vec![11, 12],
            ScanOutput::Done,
            true,
        ),
        // 12 - End key is empty
        (
            222,
            CompOperatorPb::Ge,
            0,
            CompOperatorPb::Le,
            vec![12, 13],
            ScanOutput::More, // more output as we don't have a matching end
            true,
        ),
    ];
    let mut case_num = 0;
    for (
        start_val,
        start_comp,
        end_val,
        end_comp,
        expected_oids,
        expected_scan_state,
        expected_some,
    ) in test_cases
    {
        let start_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(start_val as i64)),
            ..Default::default()
        };
        let end_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(end_val as i64)),
            ..Default::default()
        };
        let start_cmp_vals = if start_val > 0 {
            vec![RefCell::new(start_sql_val)]
        } else {
            vec![]
        };
        let end_cmp_vals = if end_val > 0 {
            vec![RefCell::new(end_sql_val)]
        } else {
            vec![]
        };
        let start_comps = vec![start_comp];
        let end_comps = vec![end_comp];
        let search_opt = indexsrch::index_scan(
            &serialized_page,
            &start_cmp_vals,
            &start_comps,
            &end_cmp_vals,
            &end_comps,
            true,
        )
        .unwrap();
        match search_opt {
            Some((oids, scan_state)) => {
                assert!(
                    expected_some,
                    "Expected None result but got Some in case {}",
                    case_num
                );
                assert_eq!(oids, expected_oids, "Expected oids in case {}", case_num);
                assert_eq!(
                    scan_state, expected_scan_state,
                    "Expected state in case {}",
                    case_num
                );
            }
            None => {
                assert!(
                    !expected_some,
                    "Expected Some result but got None in case {}",
                    case_num
                );
            }
        }

        case_num += 1;
    }
}

// Same test with composite key
#[test]
fn test_index_scan_compound_key() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    // Add index entries for the test cases:
    //  ("aaa", 111) -> oid 11
    //  ("aaa", 222) -> oid 12
    //  ("aaa", 333) -> oid 13
    let idx_values = vec![111, 222, 333];
    let mut oid_key = 11;
    let idx_head_val = SqlValuePb {
        data: Some(Data::StringValue("aaa".to_string())),
        is_constant: false,
    };
    let idx_head_val_ref = RefCell::new(idx_head_val.clone());
    for (i, idxval) in idx_values.iter().enumerate() {
        let idx_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(*idxval)),
            ..Default::default()
        };
        let idx_sql_vals = vec![idx_head_val_ref.clone(), RefCell::new(idx_sql_val)];
        serialized_page =
            indexkey::insert_key_from_values(&serialized_page, oid_key, &idx_sql_vals, i, false)
                .unwrap();
        oid_key += 1;
    }

    // Test cases:
    // has lower bound, start comparison values, start comparisons,
    // has upper bound, end comparison values, end comparisons,
    // expected oids, expected scan state, expected some
    //
    // First segment is equi comparison, always matches
    let test_cases = vec![
        // 0 - 2nd segment. Start is after end, returns None
        (
            true,
            333,
            CompOperatorPb::Ge,
            true,
            222,
            CompOperatorPb::Le,
            vec![],
            ScanOutput::Done,
            false,
        ),
        // 1 - 2nd segment. Start before end, returns matching oids
        (
            true,
            111,
            CompOperatorPb::Ge,
            true,
            222,
            CompOperatorPb::Le,
            vec![11, 12],
            ScanOutput::Done,
            true,
        ),
        // 2 - 2nd segment. Start before end, strict, returns matching oids
        (
            true,
            111,
            CompOperatorPb::Gt,
            true,
            333,
            CompOperatorPb::Lt,
            vec![12],
            ScanOutput::Done,
            true,
        ),
        // 3 - 2nd segment. Open start, returns matching oids
        (
            false,
            0,
            CompOperatorPb::Gt,
            true,
            333,
            CompOperatorPb::Lt,
            vec![11, 12],
            ScanOutput::Done,
            true,
        ),
        // 4 - 2nd segment. Open ended, returns matching oids
        (
            true,
            111,
            CompOperatorPb::Gt,
            false,
            0,
            CompOperatorPb::Lt,
            vec![12, 13],
            ScanOutput::More, // Open ended, need to check the next bt page
            true,
        ),
        // 5 - 2nd segment. Open ended, returns matching oids
        (
            true,
            111,
            CompOperatorPb::Gt,
            false,
            0,
            CompOperatorPb::Lt,
            vec![12, 13],
            ScanOutput::More, // Open ended, need to check the next bt page
            true,
        ),
        // 6 - 2nd segment equi comparison
        (
            true,
            222,
            CompOperatorPb::Eq,
            false,
            0,
            CompOperatorPb::Lt,
            vec![12],
            ScanOutput::Done, // Open ended, need to check the next bt page
            true,
        ),
    ];
    let mut case_num = 0;
    for (
        has_lb,
        start_val,
        start_comp,
        has_ub,
        end_val,
        end_comp,
        expected_oids,
        expected_scan_state,
        expected_some,
    ) in test_cases
    {
        let start_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(start_val as i64)),
            ..Default::default()
        };
        let end_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(end_val as i64)),
            ..Default::default()
        };
        let start_cmp_vals = if has_lb {
            vec![idx_head_val_ref.clone(), RefCell::new(start_sql_val)]
        } else {
            vec![]
        };
        let end_cmp_vals = if has_ub {
            vec![idx_head_val_ref.clone(), RefCell::new(end_sql_val)]
        } else {
            vec![]
        };
        let start_comps = vec![CompOperatorPb::Eq, start_comp];
        let end_comps = vec![CompOperatorPb::Eq, end_comp];
        let search_opt = indexsrch::index_scan(
            &serialized_page,
            &start_cmp_vals,
            &start_comps,
            &end_cmp_vals,
            &end_comps,
            true
        )
        .unwrap();
        match search_opt {
            Some((oids, scan_state)) => {
                assert!(
                    expected_some,
                    "Expected None result but got Some in case {}",
                    case_num
                );
                assert_eq!(oids, expected_oids, "Expected oids in case {}", case_num);
                assert_eq!(
                    scan_state, expected_scan_state,
                    "Expected state in case {}",
                    case_num
                );
            }
            None => {
                assert!(
                    !expected_some,
                    "Expected Some result but got None in case {}",
                    case_num
                );
            }
        }

        case_num += 1;
    }
}
