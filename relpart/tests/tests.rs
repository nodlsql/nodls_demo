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
    let oid_key = 456;
    let mut modified_bytes = relpart_bytes.clone();

    // 1 - insert a rel into an empty relpart
    relpart::insert_rel_succs(&mut modified_bytes, rel_id, &vec![oid_key])
        .expect("Failed to insert rel successor");

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
    assert_eq!(succs.get(0), oid_key);

    // 2 - insert another successor key into the same rel
    let oid_key2 = 789;
    relpart::insert_rel_succs(&mut modified_bytes, rel_id, &vec![oid_key2])
        .expect("Failed to insert second rel successor");

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
    assert_eq!(succs.get(0), oid_key);
    assert_eq!(succs.get(1), oid_key2);

    // 3 - insert a rel successor for a new rel
    let rel_id2 = 456;
    relpart::insert_rel_succs(&mut modified_bytes, rel_id2, &vec![oid_key])
        .expect("Failed to insert rel successor for new rel");
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
    assert_eq!(succs1.get(0), oid_key);
    assert_eq!(succs1.get(1), oid_key2);
    // Verify rSuccs for rel_id2
    let succs2 = match r2.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs2.len(), 1);
    assert_eq!(succs2.get(0), oid_key);

    // 4 - insert two successor keys into the same rel, with one duplicate
    let oid_key3 = 999;
    relpart::insert_rel_succs(&mut modified_bytes, rel_id, &vec![oid_key2, oid_key3])
        .expect("Failed to insert multiple rel successors with duplicate");
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
    // Verify that oid_key3 was added but oid_key2 was not duplicated
    let rels = rel_part_reader.get_rels().expect("Failed to get rels");
    assert_eq!(rels.len(), 2);
    let r1 = rels.get(0);
    assert_eq!(r1.get_rid(), rel_id);
    let succs1 = match r1.which().expect("Failed to determine rel type") {
        rel::Which::RSuccs(s) => s.expect("Failed to get rSuccs"),
        _ => panic!("Expected rSuccs type"),
    };
    assert_eq!(succs1.len(), 3);
    assert_eq!(succs1.get(0), oid_key);
    assert_eq!(succs1.get(1), oid_key2);
    assert_eq!(succs1.get(2), oid_key3);
}

#[test]
pub fn test_remove_rel_succs() {
    let test_data = vec![
        // Initial data, remove params, expected result (rm count, [succs])
        // Remove one successor
        [(123, vec![456, 789]), (123, vec![456]), (1, vec![789])],
        // Remove all successors
        [(123, vec![456, 789]), (123, vec![456, 789]), (2, vec![])],
        // Remove  non-existent successor (should have no effect)
        [(123, vec![456, 789]), (123, vec![555]), (0, vec![456, 789])],
        // Remove  non-existent rel id (should have no effect)
        [(123, vec![456, 789]), (125, vec![456]), (0, vec![456, 789])],
    ];
    let mut init_buf = relpart::create_relpart();

    let mut test_case_num = 0;
    for test_case in test_data {
        // Unpack test case
        let init_rel_id = test_case[0].0;
        let init_oid_keys = test_case[0].1.clone();
        let rel_id = test_case[1].0;
        let oid_keys = &test_case[1].1;
        let expected_rm_count = test_case[2].0;
        let expected_oid_keys = &test_case[2].1;

        // Insert a rel successor to set up the test
        relpart::insert_rel_succs(&mut init_buf, init_rel_id, &init_oid_keys)
            .expect("Failed to insert rel successor");
        // Remove the successor for the rel_id
        let rm_count = relpart::remove_rel_succs(&mut init_buf, rel_id, &oid_keys)
            .expect("Failed to remove rel successors");

        // Display the resulting relpart
        println!(
            "test_remove_rel_succs - RelPart for test case {} : {:?}",
            test_case_num, relpart::display_relpart(init_buf.as_slice())
        );

        // Verify the modification
        assert_eq!(
            rm_count, expected_rm_count,
            "Expected rm_count does not match"
        );
        let succs = relpart::get_rel_successors(&init_buf, 123)
            .expect("Failed to get rel successors after removal");
        assert_eq!(
            succs.len(),
            expected_oid_keys.len(),
            "Expected no successors after removal"
        );
        assert_eq!(
            &succs, expected_oid_keys,
            "Expected successors do not match after removal"
        );
        test_case_num += 1;
    }
}
