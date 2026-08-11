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
use sqlexet::{MtOidT, MtSizeT, MtStsT, SqlExeTrait, STS_SUCCESS};

use sqlinsts::{CompOperatorPb, SqlValuePb};
use std::cell::RefCell;
use tracing::debug;

pub fn bt_create_index(
    ctxt: &mut impl SqlExeTrait,
    tranid: MtOidT,
    root_key: &mut MtOidT,
    uniq: bool,
) -> MtStsT {
    let serialized_page = indexkey::create_page(uniq);
    let result = ctxt.create_index(tranid, &serialized_page, root_key);
    result
}

pub fn bt_drop_index(ctxt: &mut impl SqlExeTrait, tranid: MtOidT, root_id: MtOidT) -> MtStsT {
    let result = ctxt.drop_index(tranid, root_id);
    result
}

pub fn bt_insert_key(
    ctxt: &mut impl SqlExeTrait,
    index_page_id: MtOidT,
    id_key: MtOidT, // id value to insert
    insert_key_vals: &Vec<RefCell<SqlValuePb>>,
    unique: bool,
) -> Result<(), String> {
    // Get root node for the index schema id
    let mut serialized_page: [u8; 32000] = [0; 32000];
    let mut data_size = serialized_page.len() as MtSizeT;
    let sts = ctxt.get_index_page(
        ctxt.get_tranid(),
        ctxt.get_ltime(),
        index_page_id,
        &mut serialized_page,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        return Err(format!("Failed to get index page: 0x{:X}", sts));
    }
    let data_slice = &serialized_page[..data_size as usize];
    let res = insert_key(
        id_key, // id value to insert
        insert_key_vals,
        &data_slice,
        unique,
    );
    match res {
        Ok(new_data) => {
            // Write the updated node back to the index page
            let sts = ctxt.write_index_page(
                ctxt.get_tranid(),
                index_page_id,
                &new_data,
                new_data.len() as MtSizeT,
            );
            if sts != STS_SUCCESS {
                return Err(format!("Failed to write index page: 0x{:X}", sts));
            }
        }
        Err(e) => {
            return Err(format!("Failed to insert key: {}", e));
        }
    }
    Ok(())
}

pub fn insert_key(
    id_key: MtOidT, // id value to insert
    insert_key_vals: &Vec<RefCell<SqlValuePb>>,
    serialized_page: &[u8],
    unique: bool,
) -> Result<Vec<u8>, String> {
    // Get insert position in the page
    let reader = indexkey::get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let search_res = if unique {
        let page_reader = indexkey::get_index_key_entries(&reader)?;
        indexsrch::index_binary_search(&page_reader, &insert_key_vals)
    } else {
        let page_reader = dupindexkey::get_index_key_entries(&reader)?;
        indexsrch::index_binary_search(&page_reader, &insert_key_vals)
    };
    let mut key_found = false;
    let insert_pos = match search_res {
        indexsrch::SearchRes::Found(pos) => {
            if unique {
                return Err(format!("Duplicate key found at position {}", pos));
            }
            key_found = true;
            pos // insert after
        }
        indexsrch::SearchRes::NotFound(pos) => pos, // Insert at this position
    };
    // Insert key at the insert position and get the updated node data
    let res = if unique {
        indexkey::insert_key_from_values(
            &serialized_page,
            id_key,
            &insert_key_vals,
            insert_pos as usize,
            key_found,
        )
    } else {
        dupindexkey::insert_key_from_values(
            &serialized_page,
            id_key,
            &insert_key_vals,
            insert_pos as usize,
            key_found,
        )
    };
    match res {
        Ok(new_data) => {
            return Ok(new_data);
        }
        Err(e) => {
            return Err(format!("Failed to insert key: {}", e));
        }
    }
}

// TBD - currently if first call has GE lower bound, we want the next call for 'more' to use
// TBD - GT with last datapath value fetched as a start key. This won't work if key value has
// TBD - large number of successors that need to be split into multiple result sets.
// TBD - Also for large number of successors we will need to use a cursor and leverage ltime
// TBD - scan on the successor bag to have reliable pagination.
// So strategy is:
// 1) iindex instruction passes a flag 'more'
// 2) For the first call, we use the start constant key and GE/GT operator
// 3) For the next call with 'more' flag, we will ignore the constant and use the datapath instead
// 4) Probably need to pass a scan context back and forth to keep track.
pub fn bt_index_scan(
    ctxt: &mut impl SqlExeTrait,
    index_page_id: MtOidT,
    start_key: &Vec<RefCell<SqlValuePb>>,
    start_comp: &Vec<CompOperatorPb>,
    end_key: &Vec<RefCell<SqlValuePb>>,
    end_comp: &Vec<CompOperatorPb>,
    uniq: bool,
) -> Result<Option<(Vec<MtOidT>, indexsrch::ScanOutput)>, String> {
    // Fetch the root page
    let mut serialized_page: [u8; 32000] = [0; 32000];
    let mut data_size = serialized_page.len() as MtSizeT;
    let sts = ctxt.get_index_page(
        ctxt.get_tranid(),
        ctxt.get_ltime(),
        index_page_id,
        &mut serialized_page,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        return Err(format!("Failed to get index page: 0x{:X}", sts));
    }
    return indexsrch::index_scan(
        &serialized_page,
        start_key,
        start_comp,
        end_key,
        end_comp,
        uniq,
    );
}

pub fn bt_delete_key(
    ctxt: &mut impl SqlExeTrait,
    index_page_id: MtOidT,
    id_key: MtOidT, // id value to delete
    del_key_vals: &Vec<RefCell<SqlValuePb>>,
    unique: bool,
) -> Result<(), String> {
    // Fetch the root page
    let mut serialized_page: [u8; 32000] = [0; 32000];
    let mut data_size = serialized_page.len() as MtSizeT;
    let sts = ctxt.get_index_page(
        ctxt.get_tranid(),
        ctxt.get_ltime(),
        index_page_id,
        &mut serialized_page,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        return Err(format!("Failed to get index page: 0x{:X}", sts));
    }
    let res = delete_key(id_key, del_key_vals, &serialized_page, unique);
    match res {
        Ok(new_data) => {
            // Write the updated node back to the index page
            let sts = ctxt.write_index_page(
                ctxt.get_tranid(),
                index_page_id,
                &new_data,
                new_data.len() as MtSizeT,
            );
            if sts != STS_SUCCESS {
                return Err(format!("Failed to write index page: 0x{:X}", sts));
            }
        }
        Err(e) => {
            return Err(format!("Failed to delete key: {}", e));
        }
    }
    Ok(())
}

pub fn delete_key(
    id_key: MtOidT,
    del_key_vals: &Vec<RefCell<SqlValuePb>>,
    serialized_page: &[u8],
    unique: bool,
) -> Result<Vec<u8>, String> {
    let reader = indexkey::get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let search_res = if unique {
        let page_reader = indexkey::get_index_key_entries(&reader)?;
        indexsrch::index_binary_search(&page_reader, del_key_vals)
    } else {
        let page_reader = dupindexkey::get_index_key_entries(&reader)?;
        indexsrch::index_binary_search(&page_reader, del_key_vals)
    };

    // Locate the key to delete using binary search
    debug!("Index delete search result: {:?}", search_res);
    let delete_pos = match search_res {
        indexsrch::SearchRes::Found(pos) => pos,
        indexsrch::SearchRes::NotFound(_) => {
            return Err(format!("Key not found for deletion"));
        }
    };
    // Delete the key at the delete position and get the updated node data
    let res = if unique {
        indexkey::delete_key_at_position(&serialized_page, delete_pos as usize)
    } else {
        dupindexkey::delete_key_at_position(&serialized_page, id_key, delete_pos as usize)
    };
    match res {
        Ok(new_data) => {
            return Ok(new_data);
        }
        Err(e) => {
            return Err(format!("Failed to delete key: {}", e));
        }
    }
}
