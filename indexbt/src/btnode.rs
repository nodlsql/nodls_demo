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
use sqlinsts::sqlinsts::SqlValuePb;
use tracing::debug;

pub fn child_page_id(
    node_reader: &index_page::Reader,
    key_values: &Vec<SqlValuePb>,
) -> Result<MtOidT, String> {
    let page_reader;
    let res = if node_reader.has_node_key_entries() {
        page_reader = indexkey::get_index_key_entries(&node_reader)?;
        indexsrch::index_binary_search(&page_reader, key_values)
    } else {
        return Err("Unknown entry type".to_string());
    };
    let fetch_pos = match res {
        indexsrch::SearchRes::Found(pos) => pos,
        indexsrch::SearchRes::NotFound(pos) => pos, // position at the previous key value if not found
    };
    // Fetch the MtOid at the found position
    let key_entry = page_reader.get(fetch_pos as u32);
    let mt_oid = key_entry.get_id_value();
    Ok(mt_oid)
}

macro_rules! copy_entries_and_serialize {
    ($src:expr, $dst:expr, $first:expr, $last:expr, $newmsg:expr, $resbuf:expr) => {
        for i in $first..$last {
            let src_key_entry = $src.get(i as u32);
            let mut dst_key_entry = $dst.reborrow().get((i - $first) as u32);
            indexkey::copy_key(&src_key_entry, &mut dst_key_entry);
        }
        ::capnp::serialize::write_message($resbuf, $newmsg).map_err(|e| e.to_string())?;
    };
}

fn copy_entries(
    src_reader: &index_page::Reader,
    dst_buf: &mut Vec<u8>,
    first: usize,
    count: usize,
) -> Result<(), String> {
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();
    let left_builder = new_page_message.get_root::<index_page::Builder>().unwrap();
    // Slist - note we can't factor as new_entries have incompatible types
    if src_reader.has_slist_entries() {
        let mut new_entries = left_builder.init_slist_entries(count as u32);
        let page_reader = slistkey::get_slist_entries(&src_reader)?;
        for i in first..first + count {
            let src_key = page_reader.get(i as u32);
            new_entries.set((i - first) as u32, src_key);
        }
        ::capnp::serialize::write_message(dst_buf, &new_page_message).map_err(|e| e.to_string())?;
    }
    // All others
    else {
        let page_reader = indexkey::get_index_key_entries(&src_reader)?;
        if src_reader.has_dup_key_entries() {
            let mut new_entries = left_builder.init_dup_key_entries(count as u32);
            copy_entries_and_serialize!(
                page_reader,
                new_entries,
                first,
                first + count,
                &new_page_message,
                dst_buf
            );
        } else if src_reader.has_uniq_key_entries() || src_reader.has_node_key_entries() {
            let mut new_entries = left_builder.init_uniq_key_entries(count as u32);
            copy_entries_and_serialize!(
                page_reader,
                new_entries,
                first,
                first + count,
                &new_page_message,
                dst_buf
            );
        }
    }
    Ok(())
}

pub fn split_page(
    child_reader: &index_page::Reader,
) -> Result<(u32, Vec<SqlValuePb>, Vec<u8>, Vec<u8>), String> {
    let nb_entries = if child_reader.has_slist_entries() {
        slistkey::get_slist_entries(&child_reader)?.len()
    } else {
        indexkey::get_index_key_entries(&child_reader)?.len()
    };
    let mid = nb_entries / 2;
    debug!("Splitting page at index {}", mid);

    // Create a new left page message
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();

    // Copy the first half of the entries to the new left page.
    let mut left_serialized_page = Vec::new();
    copy_entries(&child_reader, &mut left_serialized_page, 0, mid as usize)?;

    // Create a new right page message
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();

    let mut right_serialized_page = Vec::new();
    // Copy the second half of the entries to the new right page
    copy_entries(
        &child_reader,
        &mut right_serialized_page,
        mid as usize,
        nb_entries as usize - mid as usize,
    )?;

    // Return the key component values of the first entry in the right page to be inserted into the parent node
    debug!("Splitting page at index {} len {}", mid, nb_entries);
    if child_reader.has_slist_entries() {
        let right_first_key = slistkey::get_slist_entries(&child_reader)?.get(mid);
        return Ok((
            right_first_key,
            Vec::new(),
            left_serialized_page,
            right_serialized_page,
        ));
    } else {
        let right_first_key_entry = indexkey::get_index_key_entries(&child_reader)?.get(mid);
        let (_, key_vals) = indexkey::read_key(&right_first_key_entry);
        return Ok((0, key_vals, left_serialized_page, right_serialized_page));
    };
}
