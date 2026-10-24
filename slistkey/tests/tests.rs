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
fn test_insert_slist_entry() {
    // Insert
    let serialized_page = indexkey::create_page(IndexKeyType::Slist);
    let mut result = slistkey::insert_slist_entry(&serialized_page, 333, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 111, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 222, 1).unwrap();
    result = slistkey::insert_slist_entry(&result, 444, 3).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    // Get
    let reader = get_index_page_reader!(result);
    let reader = reader
        .get_root::<index_page::Reader>()
        .expect("reader should have root");
    let entries_reader = slistkey::get_slist_entries(&reader).expect("should get slist entries");
    assert_eq!(entries_reader.len(), 4);
    assert_eq!(entries_reader.get(0), 111);
    assert_eq!(entries_reader.get(1), 222);
    assert_eq!(entries_reader.get(2), 333);
    assert_eq!(entries_reader.get(3), 444);
}

#[test]
fn test_drop_slist_entry() {
    // Insert
    let serialized_page = indexkey::create_page(IndexKeyType::Slist);
    let mut result = slistkey::insert_slist_entry(&serialized_page, 333, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 111, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 222, 1).unwrap();
    result = slistkey::insert_slist_entry(&result, 444, 3).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    // Drop
    result = slistkey::drop_slist_entry(&result, 1).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    let reader = get_index_page_reader!(result);
    let reader = reader
        .get_root::<index_page::Reader>()
        .expect("reader should have root");
    let entries_reader = slistkey::get_slist_entries(&reader).expect("should get slist entries");
    assert_eq!(entries_reader.len(), 3);
    assert_eq!(entries_reader.get(0), 111);
    assert_eq!(entries_reader.get(1), 333);
    assert_eq!(entries_reader.get(2), 444);

    result = slistkey::drop_slist_entry(&result, 0).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    let reader = get_index_page_reader!(result);
    let reader = reader
        .get_root::<index_page::Reader>()
        .expect("reader should have root");
    let entries_reader = slistkey::get_slist_entries(&reader).expect("should get slist entries");
    assert_eq!(entries_reader.len(), 2);
    assert_eq!(entries_reader.get(0), 333);
    assert_eq!(entries_reader.get(1), 444);

    result = slistkey::drop_slist_entry(&result, 1).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    let reader = get_index_page_reader!(result);
    let reader = reader
        .get_root::<index_page::Reader>()
        .expect("reader should have root");
    let entries_reader = slistkey::get_slist_entries(&reader).expect("should get slist entries");
    assert_eq!(entries_reader.len(), 1);
    assert_eq!(entries_reader.get(0), 333);
}

#[test]
fn test_scan_slist_entry() {
    // Insert
    let serialized_page = indexkey::create_page(IndexKeyType::Slist);
    let mut result = slistkey::insert_slist_entry(&serialized_page, 333, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 111, 0).unwrap();
    result = slistkey::insert_slist_entry(&result, 222, 1).unwrap();
    result = slistkey::insert_slist_entry(&result, 444, 3).unwrap();
    slistkey::pretty_print_slist_page(&result).unwrap();

    // Get
    let reader = get_index_page_reader!(result);
    let reader = reader
        .get_root::<index_page::Reader>()
        .expect("reader should have root");

    // Full scan
    let scanned_entries =
        slistkey::scan_slist_entries(&reader, 0, 4).expect("should scan slist entries");
    assert_eq!(scanned_entries, vec![111, 222, 333, 444]);
    // Partial scan from offset 2, large number of entries requested
    let scanned_entries =
        slistkey::scan_slist_entries(&reader, 2, 99).expect("should scan slist entries");
    assert_eq!(scanned_entries, vec![333, 444]);
    // Last element
    let scanned_entries =
        slistkey::scan_slist_entries(&reader, 3, 1).expect("should scan slist entries");
    assert_eq!(scanned_entries, vec![444]);
    // Too far offset
    let scanned_entries =
        slistkey::scan_slist_entries(&reader, 4, 1).expect("should scan slist entries");
    assert_eq!(scanned_entries, vec![]);
}
