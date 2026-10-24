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
use indexsrch::SearchRes;
use sqlinsts::sqlinsts::{sql_value_pb::Data, SqlValuePb};
use std::cmp::Ordering;

#[macro_export]
macro_rules! get_index_page_reader {
    ($serialized_page:ident) => {
        // Deserialize the existing index_page
        if let Ok(reader) = ::capnp::serialize::read_message_from_flat_slice(
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
fn test_search_key_pos() {
    // Insert
    let serialized_page = indexkey::create_page(IndexKeyType::Slist);
    let mut result = slistkey::insert_slist_entry(&serialized_page, 333, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 111, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 222, 1).unwrap();
    result = slistkey::insert_slist_entry(&result, 444, 3).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    let sql_val = SqlValuePb {
        data: Some(Data::OidValue(111)),
        ..Default::default()
    };
    let ordering = indexsrch::compare_id_key(&sql_val, 111);
    assert_eq!(ordering, Ordering::Equal);
    let ordering = indexsrch::compare_id_key(&sql_val, 110);
    assert_eq!(ordering, Ordering::Greater);
    let ordering = indexsrch::compare_id_key(&sql_val, 112);
    assert_eq!(ordering, Ordering::Less);

    let reader = get_index_page_reader!(result);
    let reader = reader
        .get_root::<index_page::Reader>()
        .expect("reader should have root");
    let entries_reader = slistkey::get_slist_entries(&reader).expect("should get slist entries");
    let sql_values = vec![
        // sql_value, expected_result position
        (110, SearchRes::NotFound(0)),
        (111, SearchRes::Found(0)),
        (222, SearchRes::Found(1)),
        (225, SearchRes::NotFound(2)),
        (333, SearchRes::Found(2)),
        (444, SearchRes::Found(3)),
        (445, SearchRes::NotFound(4)),
    ];
    for (sqlval, expected) in sql_values {
        let res = indexsrch::index_binary_search(
            &entries_reader,
            &vec![SqlValuePb {
                data: Some(Data::OidValue(sqlval)),
                ..Default::default()
            }],
        );
        assert_eq!(res, expected);
    }
}
