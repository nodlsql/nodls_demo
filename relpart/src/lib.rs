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
use sqlexet::{MtOidT, MtSizeT, SqlExeTrait, STS_SUCCESS};

use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum SqlRelAccessError {
    #[error("Relationship not found{0}")]
    RelNotFound(String),
    #[error("Invalid relationship{0}")]
    InvalidRel(String),
    #[error("Relationship write failed{0}")]
    RelWriteFailed(String),
}

pub fn add_rel_successors(
    ctx: &mut impl SqlExeTrait,
    rel_id: MtOidT,
    inverse: bool,
    rel_part_id: MtOidT,
    oid_keys: &Vec<MtOidT>,
) -> Result<u32, SqlRelAccessError> {
    // Fetch the rel_part data
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size = data_part.len() as MtSizeT;
    let sts = ctx.get_relpart(
        ctx.get_ltime(),
        rel_part_id,
        inverse,
        &mut data_part,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        // Create the rel_part if it doesn't exist
        let mut new_rel_buffer = create_relpart();
        insert_rel_succs(&mut new_rel_buffer, rel_id, oid_keys).map_err(|e| {
            SqlRelAccessError::InvalidRel(format!("Failed to insert rel successor: {}", e))
        })?;
        let sts = ctx.write_relpart(
            ctx.get_tranid(),
            rel_part_id,
            inverse,
            &new_rel_buffer,
            new_rel_buffer.len() as MtSizeT,
        );
        if sts != STS_SUCCESS {
            return Err(SqlRelAccessError::RelWriteFailed(format!(
                "Failed to write new rel part: 0x{:X}",
                sts
            )));
        }
        Ok(1)
    } else {
        // Update existing rel_part
        let mut rel_buffer = data_part[..data_size as usize].to_vec();
        insert_rel_succs(&mut rel_buffer, rel_id, oid_keys).map_err(|e| {
            SqlRelAccessError::InvalidRel(format!("Failed to insert rel successor: {}", e))
        })?;
        let sts = ctx.write_relpart(
            ctx.get_tranid(),
            rel_part_id,
            inverse,
            &rel_buffer,
            rel_buffer.len() as MtSizeT,
        );
        if sts != STS_SUCCESS {
            return Err(SqlRelAccessError::RelWriteFailed(format!(
                "Failed to write updated rel part: 0x{:X}",
                sts
            )));
        }
        Ok(1)
    }
}

pub fn rm_rel_successors(
    ctx: &mut impl SqlExeTrait,
    rel_id: MtOidT,
    inverse: bool,
    rel_part_id: MtOidT,
    oid_keys: &Vec<MtOidT>,
) -> Result<u32, SqlRelAccessError> {
    // Fetch the rel_part data
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size = data_part.len() as MtSizeT;
    let sts = ctx.get_relpart(
        ctx.get_ltime(),
        rel_part_id,
        inverse,
        &mut data_part,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        println!(
            "Rel part not found for rel_part_id: {}, inverse: {}",
            rel_part_id, inverse
        );
        return Err(SqlRelAccessError::RelNotFound(format!(
            "Failed to fetch rel part: 0x{:X}",
            sts
        )));
    }
    // Update existing rel_part
    let mut rel_buffer = data_part[..data_size as usize].to_vec();
    let rm_count = remove_rel_succs(&mut rel_buffer, rel_id, oid_keys).map_err(|e| {
        SqlRelAccessError::InvalidRel(format!("Failed to remove rel successor: {}", e))
    })?;
    let sts = ctx.write_relpart(
        ctx.get_tranid(),
        rel_part_id,
        inverse,
        &rel_buffer,
        rel_buffer.len() as MtSizeT,
    );
    if sts != STS_SUCCESS {
        return Err(SqlRelAccessError::RelWriteFailed(format!(
            "Failed to write updated rel part: 0x{:X}",
            sts
        )));
    }
    Ok(rm_count)
}

pub fn create_relpart() -> Vec<u8> {
    let mut serialized_rel = Vec::new();
    let mut new_rel_message = ::capnp::message::Builder::new_default();
    new_rel_message.init_root::<rel_part::Builder>();
    ::capnp::serialize::write_message(&mut serialized_rel, &new_rel_message).unwrap();
    serialized_rel
}

pub fn insert_rel_succs(
    rel_buffer: &mut Vec<u8>,
    rel_id: MtOidT,
    oid_keys: &Vec<MtOidT>,
) -> ::capnp::Result<u32> {
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(rel_buffer.as_slice()),
        ::capnp::message::ReaderOptions::new(),
    )?;
    let rel_part_reader = message_reader.get_root::<rel_part::Reader>()?;
    let rels = rel_part_reader.get_rels()?;

    // Find existing rel with matching rid; check for duplicate successor
    let mut add_rel_succs = vec![];
    let mut rid_found_pos = None;
    for i in 0..rels.len() {
        let r = rels.get(i);
        if r.get_rid() == rel_id {
            rid_found_pos = Some(i as u32);
            if let Ok(rel::Which::RSuccs(Ok(succs))) = r.which() {
                for k in 0..oid_keys.len() {
                    let mut found = false;
                    for j in 0..succs.len() {
                        if succs.get(j) == oid_keys[k] {
                            // Skip if successor already exists
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        // Not found, append to add_rel_succs
                        add_rel_succs.push(oid_keys[k]);
                    }
                }
            }
            break;
        }
    }

    // Build a new RelPart message with the modification
    let mut new_message = ::capnp::message::Builder::new_default();
    let mut new_rel_part = new_message.init_root::<rel_part::Builder>();

    let new_count = if rid_found_pos.is_some() {
        rels.len()
    } else {
        rels.len() + 1
    };
    let mut new_rels = new_rel_part.reborrow().init_rels(new_count as u32);
    let mut added_count = 0;
    for i in 0..rels.len() {
        let src = rels.get(i);
        let mut dst = new_rels.reborrow().get(i as u32);
        dst.set_rid(src.get_rid());

        if Some(i as u32) == rid_found_pos {
            // Extend rSuccs with the new successor keys
            let existing = match src.which()? {
                rel::Which::RSuccs(s) => s?,
                rel::Which::RSet(_) => {
                    return Err(::capnp::Error::failed("RelSet NYI".to_string()));
                }
            };
            let new_len = existing.len() + add_rel_succs.len() as u32;
            let mut new_succs = dst.init_r_succs(new_len);
            for j in 0..existing.len() {
                new_succs.set(j, existing.get(j));
            }
            for k in 0..add_rel_succs.len() {
                new_succs.set(existing.len() + k as u32, add_rel_succs[k]);
                added_count += 1;
            }
        } else {
            // Copy existing rel as-is in write buffer
            match src.which()? {
                rel::Which::RSuccs(s) => {
                    dst.set_r_succs(s?)?;
                }
                rel::Which::RSet(rs) => {
                    dst.set_r_set(rs?)?;
                }
            }
        }
    }

    // If no matching rel was found, append a new one
    if rid_found_pos.is_none() {
        let mut new_rel = new_rels.reborrow().get(rels.len() as u32);
        new_rel.set_rid(rel_id);
        let mut succs = new_rel.init_r_succs(oid_keys.len() as u32);
        // Set the new successor keys
        for k in 0..oid_keys.len() {
            succs.set(k as u32, oid_keys[k]);
            added_count += 1;
        }
    }

    // Write updated RelPart back to rel_buffer
    let mut new_buffer = Vec::new();
    ::capnp::serialize::write_message(&mut new_buffer, &new_message)?;
    *rel_buffer = new_buffer;

    Ok(added_count)
}

pub fn remove_rel_succs(
    rel_buffer: &mut Vec<u8>,
    rel_id: MtOidT,
    oid_keys: &Vec<MtOidT>,
) -> ::capnp::Result<u32> {
    debug!(
        "remove_rel_succs before image - rel_id: {} relPart: {:?}",
        rel_id,
        display_relpart(rel_buffer)
    );
    // Remove successors for a given rel_id
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(rel_buffer.as_slice()),
        ::capnp::message::ReaderOptions::new(),
    )?;
    let rel_part_reader = message_reader.get_root::<rel_part::Reader>()?;
    let rels = rel_part_reader.get_rels()?;
    let mut new_message = ::capnp::message::Builder::new_default();
    let mut new_rel_part = new_message.init_root::<rel_part::Builder>();
    let mut new_rels = new_rel_part.reborrow().init_rels(rels.len() as u32);
    let mut del_count = 0;
    let mut empty_rids = vec![];
    for i in 0..rels.len() {
        let src = rels.get(i);
        let mut dst = new_rels.reborrow().get(i as u32);
        dst.set_rid(src.get_rid());
        if src.get_rid() == rel_id {
            // Filter out the specified successor keys
            if let Ok(rel::Which::RSuccs(Ok(succs))) = src.which() {
                let filtered_succs: Vec<MtOidT> = (0..succs.len())
                    .map(|j| succs.get(j))
                    .filter(|s| !oid_keys.contains(s))
                    .collect();
                del_count += succs.len() as u32 - filtered_succs.len() as u32;
                if filtered_succs.is_empty() {
                    empty_rids.push(src.get_rid());
                }
                let mut new_succs = dst.init_r_succs(filtered_succs.len() as u32);
                for j in 0..filtered_succs.len() {
                    new_succs.set(j as u32, filtered_succs[j]);
                }
            } else {
                // If it's not RSuccs, copy as-is (or handle RSet if needed)
                match src.which()? {
                    rel::Which::RSet(rs) => {
                        dst.set_r_set(rs?)?;
                    }
                    _ => {}
                }
            }
        } else {
            // Copy existing rel as-is in write buffer
            match src.which()? {
                rel::Which::RSuccs(s) => {
                    dst.set_r_succs(s?)?;
                }
                rel::Which::RSet(rs) => {
                    dst.set_r_set(rs?)?;
                }
            }
        }
    }
    // If any rels got empty, remove them from the new_rels list
    if !empty_rids.is_empty() {
        let mut compacted_rels = new_rel_part
            .reborrow()
            .init_rels((rels.len() - empty_rids.len() as u32) as u32);
        let mut idx = 0;
        for i in 0..rels.len() {
            let r = rels.get(i);
            if !empty_rids.contains(&r.get_rid()) {
                let mut dst = compacted_rels.reborrow().get(idx);
                dst.set_rid(r.get_rid());
                match r.which()? {
                    rel::Which::RSuccs(s) => {
                        dst.set_r_succs(s?)?;
                    }
                    rel::Which::RSet(rs) => {
                        dst.set_r_set(rs?)?;
                    }
                }
                idx += 1;
            }
        }
    }
    // Write updated RelPart back to rel_buffer
    let mut new_buffer = Vec::new();
    ::capnp::serialize::write_message(&mut new_buffer, &new_message)?;
    *rel_buffer = new_buffer;
    debug!(
        "remove_rel_succs after image - rel_id: {} relPart: {:?}",
        rel_id,
        display_relpart(rel_buffer)
    );
   Ok(del_count)
}

// Get all the rels as an array of (rid, [succs]) tuples for testing
pub fn get_rels(rel_buffer: &[u8]) -> ::capnp::Result<Vec<(MtOidT, Vec<MtOidT>)>> {
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(rel_buffer),
        ::capnp::message::ReaderOptions::new(),
    )?;
    let rel_part_reader = message_reader.get_root::<rel_part::Reader>()?;
    let rels = rel_part_reader.get_rels()?;
    let mut result = vec![];
    for i in 0..rels.len() {
        let r = rels.get(i);
        let rid = r.get_rid();
        let mut succs = vec![];
        if let Ok(rel::Which::RSuccs(Ok(s))) = r.which() {
            for j in 0..s.len() {
                succs.push(s.get(j));
            }
        }
        result.push((rid, succs));
    }
    Ok(result)
}

pub fn get_rel_successors(rel_buffer: &[u8], rel_id: MtOidT) -> ::capnp::Result<Vec<MtOidT>> {
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(rel_buffer),
        ::capnp::message::ReaderOptions::new(),
    )?;
    debug!(
        "get_rel_successors - rel_id: {} relPart: {:?}",
        rel_id,
        display_relpart(rel_buffer)
    );
    let rel_part_reader = message_reader.get_root::<rel_part::Reader>()?;
    let rels = rel_part_reader.get_rels()?;
    for i in 0..rels.len() {
        let r = rels.get(i);
        if r.get_rid() == rel_id {
            if let Ok(rel::Which::RSuccs(Ok(s))) = r.which() {
                let mut succs = vec![];
                for j in 0..s.len() {
                    succs.push(s.get(j));
                }
                return Ok(succs);
            }
        }
    }
    Ok(vec![]) // Return empty if no matching rel or no successors
}

pub fn display_relpart(rel_buffer: &[u8]) -> String {
    let message_reader = ::capnp::serialize::read_message(
        &mut std::io::Cursor::new(rel_buffer),
        ::capnp::message::ReaderOptions::new(),
    )
    .expect("failed to init message reader");
    let rel_part_reader = message_reader
        .get_root::<rel_part::Reader>()
        .expect("failed to read relpart");
    format!("{:?}", rel_part_reader)
}
