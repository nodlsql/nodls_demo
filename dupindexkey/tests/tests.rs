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
use sqlinsts::{sql_value_pb::Data, SqlValuePb};
use std::{cell::RefCell, vec};
use indexkey;

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
pub fn test_create_page_with_new_key_entry() {
    // Create SqlValuePb values for the new key
    let mut values: Vec<RefCell<SqlValuePb>> = Vec::new();
    let str_value = SqlValuePb {
        data: Some(Data::StringValue("new_key".to_string())),
        ..Default::default()
    };
    values.push(RefCell::new(str_value));
    let int_value = SqlValuePb {
        data: Some(Data::Int64Value(42)),
        ..Default::default()
    };
    values.push(RefCell::new(int_value));

    // Create a new index_page message
    let serialized_page = indexkey::create_page(false);

    // Call the function to create a page with the new key entry
    let serialized_page =
        dupindexkey::insert_key_from_values(&serialized_page, 111, &values, 0, false).unwrap();

    // Deserialize and verify the new key entry was added correctly
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
    assert_eq!(entries.len(), 1);

    let new_entry = entries.get(0);
    let components = new_entry.get_cmpts().unwrap();
    assert_eq!(components.len(), 2);
    let comp0 = components.get(0);
    match comp0.which().unwrap() {
        key_component::Which::StrCpt(s) => {
            assert_eq!(s.unwrap(), "new_key");
        }
        _ => panic!("Expected StrCpt"),
    }
    let comp1 = components.get(1);
    match comp1.which().unwrap() {
        key_component::Which::Int64Cpt(i) => {
            assert_eq!(i, 42);
        }
        _ => panic!("Expected Int64Cpt"),
    }
}

#[test]
pub fn test_insert_key_from_values() {
    //
    // 1 - initial buffer with one entry
    //

    // Create a new index_page Leaf message
    let serialized_page = indexkey::create_page(false);

    // Append the initial values and get result buffer
    let mut initial_values: Vec<RefCell<SqlValuePb>> = Vec::new();
    let val1 = SqlValuePb {
        data: Some(Data::StringValue("initial_key".to_string())),
        ..Default::default()
    };
    initial_values.push(RefCell::new(val1));
    let res = dupindexkey::insert_key_from_values(&serialized_page, 222, &initial_values, 0, false);
    assert!(res.is_ok());
    let serialized_page = res.unwrap();

    //
    // 2 - updated buffer with two entries
    //

    // Create new values to insert
    let mut new_values: Vec<RefCell<SqlValuePb>> = Vec::new();
    let new_val1 = SqlValuePb {
        data: Some(Data::StringValue("new_key".to_string())),
        ..Default::default()
    };
    new_values.push(RefCell::new(new_val1));
    let new_val2 = SqlValuePb {
        data: Some(Data::Int64Value(99)),
        ..Default::default()
    };
    new_values.push(RefCell::new(new_val2));
    let serialized_page =
        dupindexkey::insert_key_from_values(&serialized_page, 333, &new_values, 1, false).unwrap();

    // Deserialize and verify both entries exist
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
    assert_eq!(entries.len(), 2);

    // First entry has a single string component "initial_key"
    let first_entry = entries.get(0);
    let first_cmpts = first_entry.get_cmpts().unwrap();
    let dup_id_value = first_entry.get_dup_id_value().unwrap();
    let id_values = indexkey::id_values_to_vec(dup_id_value);
    assert_eq!(id_values, vec![222]);
    assert_eq!(first_cmpts.len(), 1);
    let first_comp0 = first_cmpts.get(0);
    match first_comp0.which().unwrap() {
        key_component::Which::StrCpt(s) => {
            assert_eq!(s.unwrap(), "initial_key");
        }
        _ => panic!("Expected StrCpt"),
    }

    // Second entry has two components: a string "new_key" and an int64 99
    let second_entry = entries.get(1);
    let second_cmpts = second_entry.get_cmpts().unwrap();
    let dup_id_value = second_entry.get_dup_id_value().unwrap();
    let id_values = indexkey::id_values_to_vec(dup_id_value);
    assert_eq!(id_values, vec![333]);
    assert_eq!(second_cmpts.len(), 2);
    let second_comp0 = second_cmpts.get(0);
    match second_comp0.which().unwrap() {
        key_component::Which::StrCpt(s) => {
            assert_eq!(s.unwrap(), "new_key");
        }
        _ => panic!("Expected StrCpt"),
    }
    let second_comp1 = second_cmpts.get(1);
    match second_comp1.which().unwrap() {
        key_component::Which::Int64Cpt(i) => {
            assert_eq!(i, 99);
        }
        _ => panic!("Expected Int64Cpt"),
    }
}

#[test]
fn test_insert_keys_from_values() {
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(false);

    let insert_values = vec![
        // id_value, values to insert
        (123 as u32, "string10", 0), // initial entry [123]
        (456 as u32, "string20", 1), // insert at the end [123, >456<]
        (133 as u32, "string15", 1), // insert in the middle [123, >133<, 456]
        (100 as u32, "string05", 0), // insert at the beginning [>100<, 123, 133, 456]
    ];
    for (id_value, str_val, insert_pos) in insert_values {
        let mut values: Vec<RefCell<SqlValuePb>> = Vec::new();
        let str_sql_val = SqlValuePb {
            data: Some(Data::StringValue(str_val.to_string())),
            ..Default::default()
        };
        values.push(RefCell::new(str_sql_val));
        let int_sql_val = SqlValuePb {
            data: Some(Data::Int64Value(id_value as i64)),
            ..Default::default()
        };
        values.push(RefCell::new(int_sql_val));
        serialized_page = dupindexkey::insert_key_from_values(
            &serialized_page,
            id_value,
            &values,
            insert_pos,
            false,
        )
        .unwrap();
    }
    println!("test insert keys from values - result page");
    let _ = dupindexkey::pretty_print_index_page(&serialized_page);

    // Read back the node and verify the updated key values
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
    assert_eq!(entries.len(), 4);

    // Verify the order of entries based on id_value
    let expected_order = vec![100 as u32, 123 as u32, 133 as u32, 456 as u32];
    for (i, expected_id) in expected_order.iter().enumerate() {
        let entry = entries.get(i as u32);
        let dup_id_value = entry.get_dup_id_value().unwrap();
        let id_values = indexkey::id_values_to_vec(dup_id_value);
        assert_eq!(id_values, vec![*expected_id]);
    }
}

#[test]
fn test_read_key() {
    let id_value = 123;
    let mut values: Vec<RefCell<SqlValuePb>> = Vec::new();
    let str_val = SqlValuePb {
        data: Some(Data::StringValue("string0".to_string())),
        ..Default::default()
    };
    values.push(RefCell::new(str_val));
    let int_val = SqlValuePb {
        data: Some(Data::Int64Value(0)),
        ..Default::default()
    };
    values.push(RefCell::new(int_val));

    // Create a new index_page Leaf message
    let serialized_page = indexkey::create_page(false);

    let serialized_page =
        dupindexkey::insert_key_from_values(&serialized_page, id_value, &values, 0, false).unwrap();

    // Read back the node and extract the key
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
    let (res_ids, res_values) = indexkey::read_key(&entries.get(0));
    assert_eq!(res_ids, vec![id_value]);
    assert_eq!(res_values.len(), 2);
    match res_values[0].data.as_ref().unwrap() {
        Data::StringValue(s) => {
            assert_eq!(s, "string0");
        }
        _ => panic!("Expected StringValue"),
    }
    match res_values[1].data.as_ref().unwrap() {
        Data::Int64Value(i) => {
            assert_eq!(*i, 0);
        }
        _ => panic!("Expected Int64Value"),
    }
}

#[test]
fn test_delete_key() {
    let test_cases = vec![
        // delete position, delete id, expected remaining ids after deletion
        (0, 1000, vec![1001, 1002, 1003, 1004]), // delete first entry (ids 1000)
        (2, 1002, vec![1000, 1001, 1003, 1004]), // delete middle entry (ids 1002)
        (4, 1004, vec![1000, 1001, 1002, 1003]), // delete last entry (ids 1004)
    ];
    for (delete_pos, delete_id, expected_ids) in test_cases {
        // Create a new index_page Leaf message
        let mut serialized_page = indexkey::create_page(false);

        // Append some entries to the node
        for i in 0..5 {
            let mut values: Vec<RefCell<SqlValuePb>> = Vec::new();
            let str_sql_val = SqlValuePb {
                data: Some(Data::StringValue(format!("string{}", i))),
                ..Default::default()
            };
            values.push(RefCell::new(str_sql_val));
            let int_sql_val = SqlValuePb {
                data: Some(Data::Int64Value(i)),
                ..Default::default()
            };
            values.push(RefCell::new(int_sql_val));
            let id = i + 1000;
            serialized_page = dupindexkey::insert_key_from_values(
                &serialized_page,
                id as u32,
                &values,
                i as usize,
                false,
            )
            .unwrap();
        }

        // Delete the key at position 2 (ids 1002) and get the updated node buffer
        let serialized_page =
            dupindexkey::delete_key_at_position(&serialized_page, delete_id, delete_pos).unwrap();
        // Read back the node and verify the key was deleted
        let reader = get_index_page_reader!(serialized_page);
        let reader = reader.get_root::<index_page::Reader>().unwrap();
        let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
        assert_eq!(entries.len(), 4);
        // Verify the remaining entries have ids 1000, 1001, 1003, 1004
        for (i, expected_id) in expected_ids.iter().enumerate() {
            let entry = entries.get(i as u32);
            let dup_id_value = entry.get_dup_id_value().unwrap();
            let id_values = indexkey::id_values_to_vec(dup_id_value);
            assert_eq!(id_values, vec![*expected_id]);
        }
    }
}

#[test]
fn test_insert_duplicate_key() {
    let test_entries = vec![
        // position, sqlvalue, ids
        (0, 111, vec![1001]),
        (1, 222, vec![1002, 1003, 1004]),
        (2, 333, vec![1005]),
    ];
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(false);

    {
        for (pos, val, ids) in test_entries {
            println!(
                "test insert duplicate key - inserting key with value {} and ids {:?} at pos {}",
                val, ids, pos
            );
            let mut values: Vec<RefCell<SqlValuePb>> = Vec::new();
            let sql_val = SqlValuePb {
                data: Some(Data::Int64Value(val)),
                ..Default::default()
            };
            values.push(RefCell::new(sql_val));

            let mut key_found = false;
            for id in ids {
                println!(
                    "test insert duplicate key - insert id {} at pos {}",
                    id, pos
                );
                serialized_page = dupindexkey::insert_key_from_values(
                    &serialized_page,
                    id as u32,
                    &values,
                    pos as usize,
                    key_found,
                )
                .unwrap();
                key_found = true;
            }
        }
    }
    println!("test insert duplicate key - result page");
    let _ = dupindexkey::pretty_print_index_page(&serialized_page);

    // Verify the duplicate key entries were created correctly
    let expected = vec![
        // position, sqlvalue, ids
        (0, 111, vec![1001]),
        (1, 222, vec![1002, 1003, 1004]),
        (2, 333, vec![1005]),
    ];
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
    assert_eq!(entries.len(), 3);
    for (pos, val, ids) in expected {
        let entry = entries.get(pos as u32);
        let dup_id_value = entry.get_dup_id_value().unwrap();
        let id_values = indexkey::id_values_to_vec(dup_id_value);
        assert_eq!(id_values, *ids);
        let cmpts = entry.get_cmpts().unwrap();
        assert_eq!(cmpts.len(), 1);
        let comp0 = cmpts.get(0);
        match comp0.which().unwrap() {
            key_component::Which::Int64Cpt(i) => {
                assert_eq!(i, val);
            }
            _ => panic!("Unexpected key component type"),
        }
    }
}

#[test]
fn test_delete_duplicate_key() {
    let test_entries = vec![
        // position, sqlvalue, ids
        (0, 111, vec![1001]),
        (1, 222, vec![1002, 1003, 1004]),
        (2, 333, vec![1005]),
    ];
    // Create a new index_page Leaf message
    let mut serialized_page = indexkey::create_page(false);

    for (pos, val, ids) in test_entries {
        let mut values: Vec<RefCell<SqlValuePb>> = Vec::new();
        let sql_val = SqlValuePb {
            data: Some(Data::Int64Value(val)),
            ..Default::default()
        };
        values.push(RefCell::new(sql_val));

        let mut key_found = false;
        for id in ids {
            let res = dupindexkey::insert_key_from_values(
                &serialized_page,
                id as u32,
                &values,
                pos as usize,
                key_found,
            );
            serialized_page = res.unwrap();
            key_found = true;
        }
    }
    println!("test delete duplicate key - initial page");
    let _ = dupindexkey::pretty_print_index_page(&serialized_page);

    let test_cases = vec![
        // delete position, delete id, expected remaining ids after deletion
        (1, 1003, vec![1001, 1002, 1004, 1005]),
        (1, 1004, vec![1001, 1002, 1005]),
        (1, 1002, vec![1001, 1005]),
    ];
    let expected_ids = vec![1001, 1005];

    let mut serialized_res = vec![];
    serialized_res.extend_from_slice(&serialized_page);
    for (delete_pos, delete_id, _expected_ids) in test_cases {
        // Delete the key at the specified position and get the updated node buffer
        serialized_res =
            dupindexkey::delete_key_at_position(&serialized_res, delete_id, delete_pos).unwrap();
        println!(
            "test delete duplicate key - after deletion of id {} at pos {}",
            delete_id, delete_pos
        );
        let _ = dupindexkey::pretty_print_index_page(&serialized_res);
    }

    // Read back the node and verify the key was deleted
    let reader = get_index_page_reader!(serialized_res);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let entries = dupindexkey::get_index_key_entries(&reader).unwrap();
    // Verify the remaining entries have the expected ids
    let mut res_ids = vec![];
    for entry in entries.iter() {
        let dup_id_value = entry.get_dup_id_value().unwrap();
        let id_values = indexkey::id_values_to_vec(dup_id_value);
        res_ids.extend(id_values);
    }
    assert_eq!(res_ids, expected_ids);
}
