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
use indexkey::IndexKeyType;
use sqlinsts::sqlinsts::{sql_value_pb::Data, SqlValuePb};

macro_rules! get_index_page_reader {
    ($serialized_page:ident) => {
        // Deserialize the existing index_page
        if let Ok(reader) = ::capnp::serialize::read_message_from_flat_slice_no_alloc(
            &mut $serialized_page.as_ref(),
            ::capnp::message::ReaderOptions::new(),
        ) {
            reader
        } else {
            panic!("Failed to read message");
        }
    };
}

#[test]
pub fn test_split_uniqkey_page() {
    // Create a new index_page message
    let mut serialized_page = indexkey::create_page(IndexKeyType::Unique);
    let test_data = [
        // Composite key, id value
        (("val1", 123), 111),
        (("val1", 124), 112), // duplicate key, should fail
        (("val2", 456), 113),
    ];

    let mut insert_res;
    for ((val1, val2), id) in test_data.iter() {
        let ins_key = vec![
            SqlValuePb {
                is_constant: true,
                data: Some(Data::StringValue(val1.to_string())),
            },
            SqlValuePb {
                is_constant: true,
                data: Some(Data::Int64Value(*val2 as i64)),
            },
        ];
        // Call the function to create a page with the new key entry
        insert_res = indexbt::insert_key(
            *id as u32, // id value to insert
            &ins_key,
            &mut serialized_page,
        );
        assert!(
            insert_res.is_ok(),
            "bt_insert_key result mismatch: {:?}",
            insert_res.err()
        );
        serialized_page = insert_res.unwrap();
    }
    // split page
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let page_entries = indexkey::get_index_key_entries(&reader).unwrap();
    println!("Page reader len {}", page_entries.len());
    let (_, key_vals, left_serialized_page, right_serialized_page) =
        indexbt::btnode::split_page(&reader).unwrap();
    assert!(
        !key_vals.is_empty(),
        "Failed to split page: key values are empty"
    );

    // Deserialize the left child and verify the key entries there
    let serialized_res = left_serialized_page.to_vec();
    let page_reader = get_index_page_reader!(serialized_res);
    let reader = page_reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();
    assert_eq!(entries.len(), 1, "Expected 1 entry in the page after split");
    for entry in entries.iter() {
        println!("Entry in left child: {:?}", entry);
    }

    // Deserialize the right child and verify the key entries there
    let serialized_res = right_serialized_page.to_vec();
    let page_reader = get_index_page_reader!(serialized_res);
    let reader = page_reader.get_root::<index_page::Reader>().unwrap();
    let entries = indexkey::get_index_key_entries(&reader).unwrap();
    assert_eq!(
        entries.len(),
        2,
        "Expected 2 entries in the right page after split"
    );
    for entry in entries.iter() {
        println!("Entry in right child: {:?}", entry);
    }

    // Check the key segments value returned from the split, should be the 1st key in
    // right child
    println!("Key values returned from split: {:?}", key_vals);
    assert_eq!(
        key_vals.len(), // 2 segments
        2,
        "Key values returned from split should not be empty"
    );
    // Check 1st segment value
    let sqlval = key_vals[0].clone();
    assert_eq!(sqlval.data, Some(Data::StringValue("val1".to_string())));
    // Check 2nd segment value
    let sqlval = key_vals[1].clone();
    assert_eq!(sqlval.data, Some(Data::Int64Value(124)));
}

#[test]
fn test_split_slist_page() {
    // Create a new index_page message for a singly-linked list page
    let mut serialized_page = indexkey::create_page(IndexKeyType::Slist);
    let test_data = [111, 112, 113];

    // Insert keys
    for (i, id) in test_data.iter().enumerate() {
        let insert_res = slistkey::insert_slist_entry(&serialized_page, *id as u32, i);
        assert!(
            insert_res.is_ok(),
            "Failed to insert key: {:?}",
            insert_res.err()
        );
        serialized_page = insert_res.unwrap();
    }
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader.get_root::<index_page::Reader>().unwrap();
    let slist_entries = slistkey::get_slist_entries(&reader).unwrap();
    assert_eq!(
        slist_entries.len() as usize,
        test_data.len(),
        "Expected all slist entries to be inserted"
    );
    let (right_first_id, _key_vals, left_serialized_page, right_serialized_page) =
        indexbt::btnode::split_page(&reader).unwrap();
    assert_eq!(
        right_first_id, test_data[1],
        "Failed to split page: right first id is not the first right data entry"
    );
    // Deserialize the left child and verify the key entries there
    let serialized_res = left_serialized_page.to_vec();
    let page_reader = get_index_page_reader!(serialized_res);
    let reader = page_reader.get_root::<index_page::Reader>().unwrap();
    let slist_entries = slistkey::get_slist_entries(&reader).unwrap();
    assert!(
        !slist_entries.is_empty(),
        "Left child after split should have slist entries"
    );
    for entry in slist_entries.iter() {
        println!("Entry in left child: {:?}", entry);
    }
    // Deserialize the right child and verify the key entries there
    let serialized_res = right_serialized_page.to_vec();
    let page_reader = get_index_page_reader!(serialized_res);
    let reader = page_reader.get_root::<index_page::Reader>().unwrap();
    let slist_entries = slistkey::get_slist_entries(&reader).unwrap();
    assert!(
        !slist_entries.is_empty(),
        "Right child after split should have slist entries"
    );
    for entry in slist_entries.iter() {
        println!("Entry in right child: {:?}", entry);
    }
}
