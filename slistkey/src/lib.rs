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
use sqlexet::MtOidT;

#[macro_export]
macro_rules! get_index_page_reader {
    ($serialized_page:ident) => {
        // Deserialize the existing index_page
        if let Ok(reader) = ::capnp::serialize::read_message_from_flat_slice_no_alloc(
            &mut $serialized_page.as_ref(),
            ::capnp::message::ReaderOptions::new(),
        ) {
            reader
        } else {
            return Err("Failed to read message".to_string());
        }
    };
}

pub fn get_slist_entries<'a>(
    page_reader: &'a index_page::Reader,
) -> Result<capnp::primitive_list::Reader<'a, u32>, String> {
    match page_reader.which().map_err(|e| e.to_string())? {
        index_page::Which::SlistEntries(ids) => {
            let ids_reader = ids.map_err(|e| e.to_string())?;
            return Ok(ids_reader);
        }
        index_page::Which::UniqKeyEntries(_)
        | index_page::Which::NodeKeyEntries(_)
        | index_page::Which::DupKeyEntries(_) => {
            return Err("Non slist key entries".to_string());
        }
    }
}

pub fn scan_slist_entries(
    page_reader: &index_page::Reader,
    offset: usize,
    nbentries: usize,
) -> Result<Vec<MtOidT>, String> {
    let slist_entries = get_slist_entries(page_reader)?;
    let mut result = Vec::new();
    let end_offset = if offset + nbentries > slist_entries.len() as usize {
        slist_entries.len() as usize
    } else {
        offset + nbentries
    };
    for i in offset..end_offset {
        result.push(slist_entries.get(i as u32));
    }
    Ok(result)
}

pub fn insert_slist_entry(
    serialized_page: &[u8],
    oid_key: MtOidT,
    insert_pos: usize,
) -> Result<Vec<u8>, String> {
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_slist_entries(&reader)?;

    // Create a new index_page message
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();
    let page_builder = new_page_message.get_root::<index_page::Builder>().unwrap();

    let mut new_entries = page_builder.init_slist_entries(page_reader.len() as u32 + 1);
    for i in 0..insert_pos {
        let src_id = page_reader.get(i as u32);
        new_entries.set(i as u32, src_id);
    }
    new_entries.set(insert_pos as u32, oid_key);
    for i in insert_pos..page_reader.len() as usize {
        let src_id = page_reader.get(i as u32);
        new_entries.set((i + 1) as u32, src_id);
    }
    // Serialize the new page to bytes
    let mut buffer = Vec::new();
    ::capnp::serialize::write_message(&mut buffer, &new_page_message).map_err(|e| e.to_string())?;
    Ok(buffer)
}

pub fn drop_slist_entry(serialized_page: &[u8], drop_pos: usize) -> Result<Vec<u8>, String> {
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_slist_entries(&reader)?;

    // Create a new index_page message
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();
    let page_builder = new_page_message.get_root::<index_page::Builder>().unwrap();

    let mut new_entries = page_builder.init_slist_entries(page_reader.len() as u32 - 1);
    for i in 0..drop_pos {
        let src_id = page_reader.get(i as u32);
        new_entries.set(i as u32, src_id);
    }
    for i in (drop_pos + 1)..page_reader.len() as usize {
        let src_id = page_reader.get(i as u32);
        new_entries.set((i - 1) as u32, src_id);
    }
    // Serialize the new page to bytes
    let mut buffer = Vec::new();
    ::capnp::serialize::write_message(&mut buffer, &new_page_message).map_err(|e| e.to_string())?;
    Ok(buffer)
}

pub fn pretty_print_slist_page(serialized_page: &[u8]) -> Result<(), String> {
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_slist_entries(&reader).map_err(|e| e.to_string())?;
    println!("ids: {:?}", page_reader);
    Ok(())
}
