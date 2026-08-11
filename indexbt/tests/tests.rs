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
use indexsrch::SearchRes;
use sqlinsts::{sql_value_pb::Data, SqlValuePb};
use std::cell::RefCell;

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
pub fn test_insert_key() {
    // Create a new index_page message
    let mut serialized_page = indexkey::create_page(true);
    let test_data = [
        // Composite key, id value, expected insert success result
        (("val1", 123), 111, true),
        (("val1", 123), 112, false), // duplicate key, should fail
        (("val2", 456), 113, true),
    ];

    for ((val1, val2), id, success_result) in test_data.iter() {
        let ins_key = vec![
            RefCell::new(SqlValuePb {
                is_constant: true,
                data: Some(Data::StringValue(val1.to_string())),
            }),
            RefCell::new(SqlValuePb {
                is_constant: true,
                data: Some(Data::Int64Value(*val2 as i64)),
            }),
        ];
        // Call the function to create a page with the new key entry
        let insert_res = indexbt::insert_key(
            *id as u32, // id value to insert
            &ins_key,
            &mut serialized_page,
            true,
        );
        println!(
            "Insert key result for key ({}, {}): {:?}",
            val1,
            val2,
            if let Err(e) = &insert_res {
                format!("Error: {:?}", e)
            } else {
                "Success".to_string()
            }
        );
        assert_eq!(
            insert_res.is_ok(),
            *success_result,
            "bt_insert_key result mismatch: {:?}",
            insert_res.err()
        );

        // Deserialize and verify the new key entry was added correctly
        if *success_result {
            let serialized_res = insert_res.unwrap();
            let page_reader = get_index_page_reader!(serialized_res);
            let reader = page_reader.get_root::<index_page::Reader>().unwrap();
            let entries = indexkey::get_index_key_entries(&reader).unwrap();
            let srch_res = indexsrch::index_binary_search(
                &entries,
                &vec![
                    RefCell::new(SqlValuePb {
                        is_constant: true,
                        data: Some(Data::StringValue(val1.to_string())),
                    }),
                    RefCell::new(SqlValuePb {
                        is_constant: true,
                        data: Some(Data::Int64Value(*val2 as i64)),
                    }),
                ],
            );
            println!(
                "Search result after insertion: key: ({}, {}) res: {:?}",
                val1, val2, srch_res
            );
            assert!(
                matches!(srch_res, SearchRes::Found(_)),
                "Inserted key not found in the page"
            );
            let mut serialized_segs = Vec::new();
            let szn_res = ::capnp::serialize::write_message_segments(
                &mut serialized_segs,
                &page_reader.into_segments(),
            );
            assert!(
                szn_res.is_ok(),
                "Failed to serialize updated page: {:?}",
                szn_res.err()
            );
            serialized_page = serialized_segs;
        }
    }
}

#[test]
fn test_delete_key() {
    // Create a new index_page message
    let mut serialized_page = indexkey::create_page(true);
    let test_data = [
        // Composite key, id value, expected fetch success result after deletion
        (("val1", 123), 111, true),
        (("val2", 124), 114, false), // will be deleted
        (("val3", 125), 115, true),
    ];

    // Insert keys
    for ((val1, val2), id, _) in test_data.iter() {
        let ins_key = vec![
            RefCell::new(SqlValuePb {
                is_constant: true,
                data: Some(Data::StringValue(val1.to_string())),
            }),
            RefCell::new(SqlValuePb {
                is_constant: true,
                data: Some(Data::Int64Value(*val2 as i64)),
            }),
        ];
        // Call the function to create a page with the new key entry
        let res = indexbt::insert_key(
            *id as u32, // id value to insert
            &ins_key,
            &mut serialized_page,
            true,
        );
        assert!(res.is_ok(), "Failed to insert key: {:?}", res.err());
        let updated_page = res.unwrap();
        // Write the updated page back to the buffer for the next iteration
        serialized_page = updated_page.clone();
    }
    println!("Index page after insertions:");
    indexkey::pretty_print_index_page(&serialized_page).unwrap();

    // Create start key sql value for deletion
    let del_key = vec![
        RefCell::new(SqlValuePb {
            is_constant: true,
            data: Some(Data::StringValue("val2".to_string())),
        }),
        RefCell::new(SqlValuePb {
            is_constant: true,
            data: Some(Data::Int64Value(124)),
        }),
    ];

    // Call the function to delete the key entry
    let delete_res = indexbt::delete_key(114, &del_key, &mut serialized_page, true);
    assert!(
        delete_res.is_ok(),
        "Failed to delete key: {:?}",
        delete_res.err()
    );
    // Deserialize and verify the key entry was deleted correctly
    let serialized_page = delete_res.unwrap();
    println!("Index page after deletion:");
    indexkey::pretty_print_index_page(&serialized_page).unwrap();

    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();

    for ((val1, val2), _id, expected) in test_data.iter() {
        let composite_key = vec![
            RefCell::new(SqlValuePb {
                is_constant: true,
                data: Some(Data::StringValue(val1.to_string())),
            }),
            RefCell::new(SqlValuePb {
                is_constant: true,
                data: Some(Data::Int64Value(*val2 as i64)),
            }),
        ];
        // Verify keys are there or not
        let srch_res = indexsrch::index_binary_search(&entries, &composite_key);
        assert!(
            matches!(srch_res, SearchRes::Found(_)) == *expected,
            "Other key should still be found in the page"
        );
    }
}
