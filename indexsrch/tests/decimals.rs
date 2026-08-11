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
use indexsrch::{compare_keys, index_binary_search, SearchRes};
use rust_decimal::Decimal;
use sqlinsts::{sql_value_pb::Data, DecimalValuePb, SqlValuePb};
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
fn test_compare_decimal_key() {
    let dec_values = vec![
        // sql_value, index_value, expected_result
        (Decimal::new(1, 0), Decimal::new(2, 0), Ordering::Less),
        (Decimal::new(2, 0), Decimal::new(1, 0), Ordering::Greater),
        (Decimal::new(3, 0), Decimal::new(3, 0), Ordering::Equal),
        (Decimal::new(10, 0), Decimal::new(4, 0), Ordering::Greater),
        (Decimal::new(5, 0), Decimal::new(10, 0), Ordering::Less),
        (Decimal::new(0, 1), Decimal::new(10, 0), Ordering::Less),
    ];
    for (val0, val1, expected) in dec_values {
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
        let init_buffer = indexkey::create_page(true);
        let serialized_page =
            indexkey::insert_key_from_values(&init_buffer, 1, &idx_sql_vals, 0, false).unwrap();

        // Get the page reader and the first entry reader
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
fn test_index_binary_search() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(true);

    // Add index entries for the test cases
    let idx_values = vec![
        Decimal::new(23, 1),  // 2.3
        Decimal::new(333, 2), // 3.33
        Decimal::new(100, 1), // 10.0
    ];
    for (i, idxval) in idx_values.iter().enumerate() {
        let idx_sql_val = SqlValuePb {
            data: Some(Data::DecimalValue(DecimalValuePb {
                number: idxval.mantissa() as i64,
                scale: idxval.scale() as u32,
            })),
            ..Default::default()
        };
        let idx_sql_vals = vec![RefCell::new(idx_sql_val)];
        serialized_page =
            indexkey::insert_key_from_values(&serialized_page, 1, &idx_sql_vals, i, false).unwrap();
    }

    // Read back the node
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();
    let dec_values = vec![
        // value and scale, expected_result position
        // 2.3
        (Decimal::new(23, 1), SearchRes::Found(0)),
        // 10.0
        (Decimal::new(100, 1), SearchRes::Found(2)),
        // 3.33
        (Decimal::new(333, 2), SearchRes::Found(1)),
        // 0.0
        (Decimal::new(0, 0), SearchRes::NotFound(0)),
        // 1.15
        (Decimal::new(115, 2), SearchRes::NotFound(0)),
        // 2.25
        (Decimal::new(255, 2), SearchRes::NotFound(1)),
        // 4.00
        (Decimal::new(101, 1), SearchRes::NotFound(3)),
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
        let res = index_binary_search(&entries, &sql_cmp_vals);
        assert_eq!(res, expected, "Failed search for value {:?}", decval);
    }
}
