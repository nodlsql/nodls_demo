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

use indexcapn::indexkey_capnp::{dup_index_key, index_page, key_component};
use indexkey;
use sqlexet::MtOidT;

use sqlinsts::SqlValuePb;
use std::cell::RefCell;

use tracing::debug;

// Needed only for dup index key as for delete we need to copy separately the key components
// and the reduced id list
pub fn copy_key_components(
    source_cpts: capnp::struct_list::Reader<key_component::Owned>,
    mut dest_cpts: capnp::struct_list::Builder<key_component::Owned>,
) {
    for i in 0..source_cpts.len() {
        let src_cpt = source_cpts.get(i as u32);
        let mut dst_cpt = dest_cpts.reborrow().get(i as u32);
        match src_cpt.which().unwrap() {
            key_component::Which::StrCpt(s) => {
                dst_cpt.set_str_cpt(s.unwrap());
            }
            key_component::Which::Int64Cpt(i) => {
                dst_cpt.set_int64_cpt(i);
            }
            key_component::Which::BoolCpt(b) => {
                dst_cpt.set_bool_cpt(b);
            }
            key_component::Which::NullCpt(_) => {
                dst_cpt.set_null_cpt(());
            }
            key_component::Which::NoneCpt(_) => {
                dst_cpt.set_none_cpt(());
            }
            key_component::Which::DecimalCpt(d) => {
                let mut decimal_value = dst_cpt.reborrow().init_decimal_cpt();
                decimal_value.set_number(d.clone().unwrap().get_number());
                decimal_value.set_scale(d.clone().unwrap().get_scale() as i32);
            }
        }
    }
}

pub fn get_index_key_entries<'a>(
    page_reader: &'a index_page::Reader,
) -> Result<capnp::struct_list::Reader<'a, dup_index_key::Owned>, String> {
    match page_reader.which().unwrap() {
        index_page::Which::UniqKeyEntries(_) => {
            return Err("Unique key entries".to_string());
        }
        index_page::Which::DupKeyEntries(entries) => {
            let reader = entries.unwrap();
            return Ok(reader);
        }
    }
}

pub fn insert_key_from_values(
    serialized_page: &[u8],
    id_key: MtOidT,
    values: &Vec<RefCell<SqlValuePb>>,
    insert_pos: usize,
    key_found: bool,
) -> Result<Vec<u8>, String> {
    let reader = indexkey::get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_index_key_entries(&reader)?;

    // Create a new index_page message
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();
    // init leaf page with empty entries list
    let page_builder = new_page_message.get_root::<index_page::Builder>().unwrap();

    // Get the existing key_entries and calculate the new size
    let existing_count = page_reader.len();
    let new_count = if key_found {
        existing_count
    } else {
        existing_count + 1
    };

    // Initialize the key_entries list with the new size
    let mut new_entries = page_builder.init_dup_key_entries(new_count as u32);

    // Copy entries before the insertion point
    for i in 0..insert_pos {
        let src_key_entry = page_reader.get(i as u32);
        let mut dst_key_entry = new_entries.reborrow().get(i as u32);
        indexkey::copy_key(&src_key_entry, &mut dst_key_entry);
    }

    // Insert the new key at the insertion point
    if key_found {
        let existing_key_entry = page_reader.get(insert_pos as u32);
        // Get the current ID list for the existing key
        let dup_id_value = existing_key_entry.get_dup_id_value().unwrap();
        let mut id_keys = indexkey::id_values_to_vec(dup_id_value);
        // Add the new ID to the list if it's not already present
        if !id_keys.contains(&id_key) {
            id_keys.push(id_key);
        }
        debug!(
            "insert key from values - updating entry at position {}",
            insert_pos
        );
        let mut new_key_entry = new_entries.reborrow().get(insert_pos as u32);
        indexkey::build_index_key(&id_keys, values, &mut new_key_entry);
    } else {
        debug!(
            "insert key from values - appending new entry at position {}",
            insert_pos
        );
        let mut new_key_entry = new_entries.reborrow().get(insert_pos as u32);
        indexkey::build_index_key(&vec![id_key], values, &mut new_key_entry);
    }

    // Copy entries after the insertion point
    let source_pos = if key_found {
        insert_pos + 1
    } else {
        insert_pos
    };
    let mut dest_pos = if key_found {
        insert_pos
    } else {
        insert_pos + 1
    };
    for spos in source_pos..existing_count as usize {
        debug!(
            "insert key from values - copying entry at position {} after insertion point",
            spos
        );
        let src_key_entry = page_reader.get(spos as u32);
        let mut dst_key_entry = new_entries.reborrow().get((dest_pos) as u32);
        indexkey::copy_key(&src_key_entry, &mut dst_key_entry);
        dest_pos += 1;
        if dest_pos >= new_count as usize {
            break;
        }
    }

    // Serialize the new page to bytes
    let mut buffer = Vec::new();
    ::capnp::serialize::write_message(&mut buffer, &new_page_message).map_err(|e| e.to_string())?;
    Ok(buffer)
}

pub fn delete_key_at_position(
    serialized_page: &[u8],
    id_key: MtOidT, // TBD - remove key if empty after id key deletion
    delete_pos: usize,
) -> Result<Vec<u8>, String> {
    let reader = indexkey::get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_index_key_entries(&reader)?;

    // Create a new index_page message
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();
    // init leaf page with empty entries list
    let page_builder = new_page_message.get_root::<index_page::Builder>().unwrap();

    let existing_count = page_reader.len();
    if delete_pos >= existing_count as usize {
        return Err(format!(
            "Delete position {} out of bounds (existing count {})",
            delete_pos, existing_count
        ));
    }
    // Read key at position, check num ids
    let key_entry = page_reader.get(delete_pos as u32);
    let dup_id_value = key_entry.get_dup_id_value().unwrap();
    let id_keys = indexkey::id_values_to_vec(dup_id_value);
    debug!(
        "delete key at position {} - existing ids: {:?}",
        delete_pos, id_keys
    );
    let drop_full_key = id_keys.len() == 1;

    let new_count = if drop_full_key {
        existing_count - 1
    } else {
        existing_count
    };
    // Initialize the key_entries list with the new size
    let mut new_entries = page_builder.init_dup_key_entries(new_count as u32);

    // Copy entries before the deletion point
    for i in 0..delete_pos {
        let src_key_entry = page_reader.get(i as u32);
        let mut dst_key_entry = new_entries.reborrow().get(i as u32);
        debug!(
            "delete key at position - copying entry at position {} before deletion point, new count {}",
            i, new_count
        );
        indexkey::copy_key(&src_key_entry, &mut dst_key_entry);
    }
    // If not dropping the key, copy the entry at the deletion point with the id removed
    if !drop_full_key {
        let mut new_id_keys = id_keys.clone();
        new_id_keys.retain(|&id| id != id_key);
        // if only one id left, convert to uniq_id_value
        if new_id_keys.len() == 1 {
            let mut new_key_entry = new_entries.reborrow().get(delete_pos as u32);
            new_key_entry
                .reborrow()
                .init_dup_id_value()
                .set_uniq_id_value(new_id_keys[0]);
            let source_cpts = key_entry.get_cmpts().unwrap();
            let dest_cpts = new_key_entry
                .reborrow()
                .init_cmpts(source_cpts.len() as u32);
            copy_key_components(source_cpts, dest_cpts);
        } else {
            let mut new_key_entry = new_entries.reborrow().get(delete_pos as u32);
            let mut dup_id_value = new_key_entry.reborrow().init_dup_id_value();
            let mut list_builder = dup_id_value
                .reborrow()
                .init_list_id_value(new_id_keys.len() as u32);
            for (i, id) in new_id_keys.iter().enumerate() {
                list_builder.set(i as u32, *id);
            }
            let source_cpts = key_entry.get_cmpts().unwrap();
            let dest_cpts = new_key_entry
                .reborrow()
                .init_cmpts(source_cpts.len() as u32);
            copy_key_components(source_cpts, dest_cpts);
        }
    }
    let src_pos = delete_pos + 1;
    let mut dest_pos = if drop_full_key {
        delete_pos
    } else {
        delete_pos + 1
    };
    debug!(
        "delete key at position - start after deletion point. delpos {} srcpos {} destpos {}",
        delete_pos, src_pos, dest_pos
    );
    // Copy entries after the deletion point
    for read_pos in src_pos..existing_count as usize {
        debug!(
            "delete key at position - copying entry from position {} to position {} after deletion point",
            read_pos, dest_pos
        );
        let src_key_entry = page_reader.get(read_pos as u32);
        let mut dst_key_entry = new_entries.reborrow().get(dest_pos as u32);
        indexkey::copy_key(&src_key_entry, &mut dst_key_entry);
        dest_pos += 1;
        if dest_pos >= new_count as usize {
            break;
        }
    }
    // Serialize the new page to bytes
    let mut buffer = Vec::new();
    ::capnp::serialize::write_message(&mut buffer, &new_page_message).map_err(|e| e.to_string())?;
    Ok(buffer)
}

// Duplicated code as get_index_key_entries return incompatible type
pub fn pretty_print_index_page(serialized_page: &[u8]) -> Result<(), String> {
    let reader = indexkey::get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_index_key_entries(&reader)?;
    for i in 0..page_reader.len() {
        let key_entry = page_reader.get(i as u32);
        let (id_values, values) = indexkey::read_key(&key_entry);
        println!("Key {}: ids: {:?}, values: {:?}", i, id_values, values);
    }
    Ok(())
}
