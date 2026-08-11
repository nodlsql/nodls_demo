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

use indexcapn::indexkey_capnp::index_page;
use rust_decimal::Decimal;
use sqlinsts::{sql_value_pb::Data, CompOperatorPb, DecimalValuePb, SqlValuePb};
use std::cell::RefCell;
use std::vec;

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
fn test_insert_decimal_key() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    let dec_values = vec![
        // sql_value, expected order after insertion
        (22, Decimal::new(333, 2), 1), // 3.33
        (11, Decimal::new(23, 1), 0),  // 2.3
        (33, Decimal::new(9, 0), 2),   // int value 9
        (44, Decimal::new(100, 1), 3), // 10.0
    ];

    for (id, decval, _expected) in &dec_values {
        let number = decval.mantissa() as i64;
        let scale = decval.scale() as u32;
        let data = if scale == 0 {
            Data::Int64Value(number)
        } else {
            Data::DecimalValue(DecimalValuePb { number, scale })
        };
        let idx_sql_val = SqlValuePb {
            data: Some(data),
            ..Default::default()
        };
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];

        let insert_res = indexbt::insert_key(
            *id, // id value to insert
            &idx_sql_vals,
            &mut serialized_page,
            true,
        );
        assert!(
            insert_res.is_ok(),
            "Failed to insert key {:?}: {:?}",
            decval,
            insert_res.err()
        );
        serialized_page = insert_res.clone().unwrap();
    }

    // Scan the index page and verify the keys are in the expected order
    let mut idx = 0;

    // 1 - Full scan
    let (ids, scan_sts) = indexsrch::index_scan(&serialized_page, &vec![], &vec![], &vec![], &vec![], true)
        .unwrap()
        .unwrap();
    println!("Scanned ids: {:?}, sts: {:?}", ids, scan_sts);
    assert_eq!(
        scan_sts,
        indexsrch::ScanOutput::More,
        "Unexpected scan status after scanning all entries"
    );
    for id in ids {
        let expected_offset = dec_values[idx].2;
        println!(
            "Scanned id: {}, expected: {}",
            id, dec_values[expected_offset].0
        );
        assert_eq!(
            id,
            dec_values[expected_offset].0,
            "Unexpected id value in index scan result for key {:?}",
            dec_values[idx].1.to_string()
        );
        idx += 1;
    }

    // 2 - Scan from a specific key
    let start_dec = Decimal::new(81, 1);
    let start_key = vec![RefCell::new(SqlValuePb {
        data: Some(Data::DecimalValue(DecimalValuePb {
            number: start_dec.mantissa() as i64,
            scale: start_dec.scale() as u32,
        })),
        ..Default::default()
    })];
    let start_cmp = vec![CompOperatorPb::Gt];
    let (ids, scan_sts) = indexsrch::index_scan(&serialized_page, &start_key, &start_cmp, &vec![], &vec![], true)
        .unwrap()
        .unwrap();
    println!("Scanned ids: {:?}, sts: {:?}", ids, scan_sts);
    assert_eq!(
        scan_sts,
        indexsrch::ScanOutput::More,
        "Unexpected scan status after scanning all entries"
    );
    assert_eq!(
        ids,
        vec![33, 44],
        "Unexpected ids returned when scanning from key > {:?}",
        start_dec.to_string()
    );
}

#[test]
fn test_index_binary_search() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    // Add index entries for the test cases
    let idx_values = vec![
        Decimal::new(23, 1),  // 2.3
        Decimal::new(333, 2), // 3.33
        Decimal::new(100, 1), // 10.0
    ];
    for (i , idxval) in idx_values.iter().enumerate() {
        let idx_sql_val = SqlValuePb {
            data: Some(Data::DecimalValue(DecimalValuePb {
                number: idxval.mantissa() as i64,
                scale: idxval.scale() as u32,
            })),
            ..Default::default()
        };
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];
        serialized_page = indexkey::insert_key_from_values(&serialized_page, i as u32 + 20, &idx_sql_vals, i, false).unwrap();
    }

    indexkey::pretty_print_index_page(&serialized_page).unwrap();

    // Read back the node
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();
    let dec_values = vec![
        // value and scale, expected_result position
        // 2.3
        (Decimal::new(23, 1), indexsrch::SearchRes::Found(0)),
        // 10.0
        (Decimal::new(100, 1), indexsrch::SearchRes::Found(2)),
        // 3.33
        (Decimal::new(333, 2), indexsrch::SearchRes::Found(1)),
        // 0.0
        (Decimal::new(0, 0), indexsrch::SearchRes::NotFound(0)),
        // 1.15
        (Decimal::new(115, 2), indexsrch::SearchRes::NotFound(0)),
        // 2.25
        (Decimal::new(255, 2), indexsrch::SearchRes::NotFound(1)),
        // 4.00
        (Decimal::new(101, 1), indexsrch::SearchRes::NotFound(3)),
    ];
    for (decval, expected) in dec_values {
        let sql_val = SqlValuePb {
            data: Some(Data::DecimalValue(DecimalValuePb {
                number: decval.mantissa() as i64,
                scale: decval.scale() as u32,
            })),
            ..Default::default()
        };
        let sql_cmp_vals = vec![RefCell::new(sql_val)];
        // Binary search for the key in the index node
        let res = indexsrch::index_binary_search(&entries, &sql_cmp_vals);
        assert_eq!(res, expected, "Failed search for value {:?}", decval);
    }
}
