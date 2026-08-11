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

// Sql executor trait

use std::ffi::c_void;

// To keep in sync with MT_SCHEMA_TYPE enum in mt_sql_utils.h
#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum MtSchemaType {
    KeyDataset = 0,
    KeyObject,
    KeyAT,
    KeyRS,
    KeyType,
    KeyIndex,
    KeyEPDict,
    KeyMethod,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum IdSpc {
    Internal = 0,
    DataPart,
    RelPart,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum UpdCounter {
    Insert(i32),
    Delete(i32),
    Update(i32),
    AddSucc(i32),
    RmSucc(i32),
    CreateDataset(i32),
    AlterDataset(i32),
    DropDataset(i32),
}

impl UpdCounter {
    pub fn inc(&mut self, count: i32) {
        match self {
            UpdCounter::Insert(ref mut cnt)
            | UpdCounter::Delete(ref mut cnt)
            | UpdCounter::Update(ref mut cnt)
            | UpdCounter::AddSucc(ref mut cnt)
            | UpdCounter::RmSucc(ref mut cnt) 
            | UpdCounter::CreateDataset(ref mut cnt)
            | UpdCounter::AlterDataset(ref mut cnt)
            | UpdCounter::DropDataset(ref mut cnt) => {
                *cnt += count;
            }
        }
    }

    pub fn get(&self) -> i32 {
        match self {
            UpdCounter::Insert(cnt)
            | UpdCounter::Delete(cnt)
            | UpdCounter::Update(cnt)
            | UpdCounter::AddSucc(cnt)
            | UpdCounter::RmSucc(cnt)
            | UpdCounter::CreateDataset(cnt)
            | UpdCounter::AlterDataset(cnt)
            | UpdCounter::DropDataset(cnt) => *cnt,
        }
    }

    pub fn print(&self) -> String {
        match self {
            UpdCounter::Insert(cnt) => format!("{} items inserted", cnt),
            UpdCounter::Delete(cnt) => format!("{} items deleted", cnt),
            UpdCounter::Update(cnt) => format!("{} items updated", cnt),
            UpdCounter::AddSucc(cnt) => format!("{} successors added", cnt),
            UpdCounter::RmSucc(cnt) => format!("{} successors removed", cnt),
            UpdCounter::CreateDataset(_) => format!("Dataset created"),
            UpdCounter::AlterDataset(_) => format!("Dataset altered"),
            UpdCounter::DropDataset(_) => format!("Dataset dropped"),
        }
    }
}

pub const STS_SUCCESS: i32 = 1;
pub const MTS_INVDUPLICATE: i32 = 0x84681e2;
pub const MTS_ENDOFSTREAM: i32 = 0x08468078;
pub const MTS_CLASSNOTFOUND: i32 = 0x84680fa;
pub const MTS_OBJNOTFOUND: i32 = 0x8468442;

// TBD - needs more accurate surrogate check. Currently this includes dataset desc at 0x1061
pub const MIN_USER_DATASET_ID: u32 = 0x1060;

pub type MtSizeT = u32;
pub type MtOidT = u32;
pub type MtLTimeT = u32;
pub type MtStsT = i32;

pub trait SqlExeTrait {
    fn new() -> Self;
    fn set_handle(&mut self, handle: *mut c_void);
    fn get_handle(&self) -> *mut c_void;
    fn set_ltime(&mut self, ltime: MtLTimeT);
    fn get_ltime(&self) -> MtLTimeT;
    fn set_tranid(&mut self, tranid: MtOidT);
    fn get_tranid(&self) -> MtOidT;

    fn increment_count(&mut self, cnt_type: UpdCounter);
    fn clear_counts(&mut self);
    // Pretty print count message if any or return empty string
    fn print_count(&self) -> String;

    fn objid_make(&mut self, schema_type: MtSchemaType) -> MtOidT;

    fn connect_database(&mut self, database: &str) -> MtStsT;
    fn disconnect_database(&mut self);
    fn activate_task(&self);
    fn deactivate_task(&mut self);
    fn start_local_qtran(&mut self) -> MtLTimeT;
    fn start_tran(&mut self) -> MtOidT;
    fn tran_commit(&mut self) -> MtStsT;
    fn tran_abort(&self) -> MtStsT;
    fn create_item(
        &mut self,
        tranid: MtOidT,
        dataset_id: MtOidT,
        item_id: &mut MtOidT,
        data: &Vec<u8>,
        data_size: MtSizeT,
    ) -> MtStsT;
    fn update_item(
        &mut self,
        tranid: MtOidT,
        item_id: MtOidT,
        data: &Vec<u8>,
        data_size: MtSizeT,
    ) -> MtStsT;
    fn delete_item(&mut self, tranid: MtOidT, ns: IdSpc, item_id: MtOidT) -> MtStsT;
    // Create or update dataset
    fn write_dataset(
        &mut self,
        tranid: MtOidT,
        dataset_id: MtOidT,
        dataset_name: &str,
        data: &Vec<u8>,
        data_size: MtSizeT,
    ) -> MtStsT;
    fn drop_dataset(
        &mut self,
        tranid: MtOidT,
        dataset_id: MtOidT,
    ) -> MtStsT;
    fn create_index(&mut self, tranid: MtOidT, root_data: &Vec<u8>, root_id: &mut MtOidT)
        -> MtStsT;
    fn drop_index(&mut self, tranid: MtOidT, root_id: MtOidT)
        -> MtStsT;
    fn write_index_page(
        &mut self,
        tranid: MtOidT,
        page_id: MtOidT,
        page_data: &Vec<u8>,
        page_data_size: MtSizeT,
    ) -> MtStsT;
    fn get_index_page(
        &self,
        tranid: MtOidT,
        ltime: MtLTimeT,
        index_page_id: MtOidT,
        page_data: &mut [u8; 32000],
        page_data_size: *mut MtSizeT,
    ) -> MtStsT;
    fn get_schema_item(
        &self,
        tranid: MtOidT,
        ltime: MtLTimeT,
        schema_name: &str,
        obj_type: MtSchemaType,
        schema_item_id: &mut MtOidT,
    ) -> MtStsT;
    fn dataset_enum_start(
        &self,
        ltime: MtLTimeT,
        dataset_id: MtOidT,
        dataset_stream: &mut MtOidT,
        ids: *mut MtOidT,
        num_ids: &mut MtSizeT,
    ) -> MtStsT;
    fn dataset_enum_many(
        &self,
        dataset_stream: MtOidT,
        ids: *mut MtOidT,
        num_ids: &mut MtSizeT,
    ) -> MtStsT;
    fn dataset_enum_end(&self, dataset_stream: MtOidT) -> MtStsT;
    fn get_datapart(
        &self,
        ltime: MtLTimeT,
        item_id: MtOidT,
        dataset_id: &mut MtOidT,
        data_part: &mut [u8; 32000],
        data_size: &mut MtSizeT,
    ) -> MtStsT;
    fn get_relpart(
        &self,
        ltime: MtLTimeT,
        item_id: MtOidT,
        inverse: bool,
        data_part: &mut [u8; 32000],
        data_size: &mut MtSizeT,
    ) -> MtStsT;
    fn write_relpart(
        &mut self,
        tranid: MtOidT,
        item_id: MtOidT,
        inverse: bool,
        data_part: &Vec<u8>,
        data_size: MtSizeT,
    ) -> MtStsT;
}
