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

use std::vec;

use entitycapn::entity_capnp::{rel, rel_part};

#[test]
pub fn test_insert_rel() {
    let relpart_bytes = relpart::create_relpart();
    let rel_id = 123;
    let id_key = 456;
    let mut modified_bytes = relpart_bytes.clone();

    // 1 - insert a rel into an empty relpart
    relpart::insert_rel_elts(&mut modified_bytes, rel_id, &vec![id_key])
        .expect("Failed to insert rel element");

    // Verify the modification
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(modified_bytes.as_slice()),
        ::capnp::message::ReaderOptions::new(),
    )
    .expect("Failed to read modified relpart");
    let rel_part_reader = message_reader
        .get_root::<rel_part::Reader>()
        .expect("Failed to get rel_part reader");
    println!(
        "test_insert_rel - RelPart after first modification: {:?}",
        rel_part_reader
    );

    // Check the rels list
    let rels = rel_part_reader.get_rels().expect("Failed to get rels");
    assert_eq!(rels.len(), 1);
    let r = rels.get(0);
    assert_eq!(r.get_rid(), rel_id);

    // Check the rSuccs list
    let succs = match r.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs.len(), 1);
    assert_eq!(succs.get(0), id_key);

    // 2 - insert another element key into the same rel
    let id_key2 = 789;
    relpart::insert_rel_elts(&mut modified_bytes, rel_id, &vec![id_key2])
        .expect("Failed to insert second rel element");

    // Verify the modification
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(modified_bytes.as_slice()),
        ::capnp::message::ReaderOptions::new(),
    )
    .expect("Failed to read modified relpart");
    let rel_part_reader = message_reader
        .get_root::<rel_part::Reader>()
        .expect("Failed to get rel_part reader");
    println!(
        "test_insert_rel - RelPart after second modification: {:?}",
        rel_part_reader
    );

    // Check the rels list again
    let rels = rel_part_reader.get_rels().expect("Failed to get rels");
    assert_eq!(rels.len(), 1);
    let r = rels.get(0);
    assert_eq!(r.get_rid(), rel_id);
    let succs = match r.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs.len(), 2);
    assert_eq!(succs.get(0), id_key);
    assert_eq!(succs.get(1), id_key2);

    // 3 - insert a rel element for a new rel
    let rel_id2 = 456;
    relpart::insert_rel_elts(&mut modified_bytes, rel_id2, &vec![id_key])
        .expect("Failed to insert rel element for new rel");
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(modified_bytes.as_slice()),
        ::capnp::message::ReaderOptions::new(),
    )
    .expect("Failed to read modified relpart");
    let rel_part_reader = message_reader
        .get_root::<rel_part::Reader>()
        .expect("Failed to get rel_part reader");
    println!(
        "test_insert_rel - RelPart after third modification: {:?}",
        rel_part_reader
    );

    let rels = rel_part_reader.get_rels().expect("Failed to get rels");
    assert_eq!(rels.len(), 2);
    let r1 = rels.get(0);
    let r2 = rels.get(1);
    assert_eq!(r1.get_rid(), rel_id);
    assert_eq!(r2.get_rid(), rel_id2);
    // Verify rSuccs for rel_id
    let succs1 = match r1.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs1.len(), 2);
    assert_eq!(succs1.get(0), id_key);
    assert_eq!(succs1.get(1), id_key2);
    // Verify rSuccs for rel_id2
    let succs2 = match r2.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs2.len(), 1);
    assert_eq!(succs2.get(0), id_key);

    // 4 - insert two element keys into the same rel, with one duplicate
    let id_key3 = 999;
    let res = relpart::insert_rel_elts(&mut modified_bytes, rel_id, &vec![id_key2, id_key3]);
    match res {
        Ok(_) => {
            // This should not happen, as id_key2 is a duplicate
            panic!("Expected error for duplicate element, but insert succeeded");
        }
        Err(e) => assert!(
            e.to_string().contains("already exists"),
            "Expected error for duplicate element"
        ),
    }

    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(modified_bytes.as_slice()),
        ::capnp::message::ReaderOptions::new(),
    )
    .expect("Failed to read modified relpart");
    let rel_part_reader = message_reader
        .get_root::<rel_part::Reader>()
        .expect("Failed to get rel_part reader");
    println!(
        "test_insert_rel - RelPart after fourth modification: {:?}",
        rel_part_reader
    );
    // Verify that id_key3 was added but id_key2 was not duplicated
    let rels = rel_part_reader.get_rels().expect("Failed to get rels");
    assert_eq!(rels.len(), 2);
    let r1 = rels.get(0);
    assert_eq!(r1.get_rid(), rel_id);
    let succs1 = match r1.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs1.len(), 2);
    assert_eq!(succs1.get(0), id_key);
    assert_eq!(succs1.get(1), id_key2);
}

#[test]
pub fn test_remove_rel_elts() {
    let test_data = vec![
        // Initial data, remove params, expected result (rm count, [succs]), error msg if any
        // Remove one element
        ([(123, vec![456, 789]), (123, vec![456]), (1, vec![789])], ""),
        // Remove all elements
        ([(123, vec![456]), (123, vec![456, 789]), (2, vec![])], ""),
        // Remove  non-existent element (should have no effect)
        ([(123, vec![456, 789]), (123, vec![555]), (0, vec![456, 789])], "Element not found"),
        // Remove  non-existent rel id (should have no effect)
        ([(123, vec![]), (125, vec![456]), (0, vec![456, 789])], "Relationship not found"),
    ];
    let mut init_buf = relpart::create_relpart();

    let mut test_case_num = 0;
    for (test_case, error_msg) in test_data {
        // Unpack test case
        let init_rel_id = test_case[0].0;
        let init_id_keys = test_case[0].1.clone();
        let rel_id = test_case[1].0;
        let id_keys = &test_case[1].1;
        let expected_rm_count = test_case[2].0;
        let expected_id_keys = &test_case[2].1;

        // Insert a rel successor to set up the test
        relpart::insert_rel_elts(&mut init_buf, init_rel_id, &init_id_keys)
            .expect("successor already exists");
        // Remove the successor for the rel_id
        let res = relpart::remove_rel_elts(&mut init_buf, rel_id, &id_keys);
        match res {
            Ok(rm_count) => {
                if !error_msg.is_empty() {
                    panic!("Expected error message but got success");
                }
                assert_eq!(
                    rm_count, expected_rm_count,
                    "Expected rm_count does not match"
                );
                let succs = relpart::get_rel_elts(&init_buf, rel_id)
                    .expect("Failed to get rel elements after removal");
                assert_eq!(
                    succs.len(),
                    expected_id_keys.len(),
                    "Expected no elements after removal"
                );
                assert_eq!(
                    &succs, expected_id_keys,
                    "Expected elements do not match after removal"
                );
            }
            Err(e) => {
                println!("Error message for remove_rel_elts: {}", e);
                if !error_msg.is_empty() {
                    assert!(
                        e.to_string().contains(error_msg),
                        "Expected error message not found"
                    );
                } else {
                    panic!("Unexpected error: {}", e);
                }
            }
        }

        // Display the resulting relpart
        println!(
            "test_remove_rel_elts - RelPart for test case {} : {:?}",
            test_case_num,
            relpart::display_relpart(init_buf.as_slice())
        );

        test_case_num += 1;
    }
}
