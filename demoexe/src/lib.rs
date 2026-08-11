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

use demodata::tstdata;

use sqlexet::{
    IdSpc, MtLTimeT, MtOidT, MtSchemaType, MtSizeT, MtStsT, SqlExeTrait, MTS_CLASSNOTFOUND,
    MTS_ENDOFSTREAM, MTS_OBJNOTFOUND, STS_SUCCESS,
};
use std::ffi::c_void;
use tracing::debug;

static META_DATASET_ID: MtOidT = 4193; // raz pseudo-dataset
static MIN_ENTITY_ID: MtOidT = 10000;

pub struct DemoContextT {
    handle: *mut c_void,
    ltime: MtLTimeT,
    tranid: MtOidT,
    counter: Option<sqlexet::UpdCounter>,
    demo_data: tstdata::SchemaPb,
}

impl SqlExeTrait for DemoContextT {
    fn new() -> Self {
        DemoContextT {
            handle: std::ptr::null_mut(),
            ltime: 0,
            tranid: 0,
            counter: None,
            demo_data: tstdata::SchemaPb {
                // list of DatasetPb
                datasets: vec![],
                indexes: vec![],
                rs_items: vec![],
                invrs_items: vec![],
                next_dataset_id: META_DATASET_ID + 1,
                next_item_id: MIN_ENTITY_ID,
            },
        }
    }

    fn increment_count(&mut self, count: sqlexet::UpdCounter) {
        // Create a counter if not already there
        if self.counter.is_none() {
            self.counter = Some(count);
            return;
        }
        let current_cnt_type = self.counter.unwrap();
        if current_cnt_type != count {
            println!(
                "Counter type mismatch: current {:?}, incrementing with {:?}",
                current_cnt_type, count
            );
            return;
        }
        self.counter.as_mut().unwrap().inc(count.get());
    }

    fn clear_counts(&mut self) {
        self.counter = None;
    }

    fn print_count(&self) -> String {
        if let Some(counter) = &self.counter {
            counter.print()
        } else {
            "".to_string()
        }
    }

    fn set_handle(&mut self, handle: *mut c_void) {
        self.handle = handle;
    }

    fn get_handle(&self) -> *mut c_void {
        self.handle
    }

    fn set_ltime(&mut self, ltime: MtLTimeT) {
        self.ltime = ltime;
    }

    fn get_ltime(&self) -> MtLTimeT {
        self.ltime
    }

    fn set_tranid(&mut self, tranid: MtOidT) {
        self.tranid = tranid;
    }

    fn get_tranid(&self) -> MtOidT {
        self.tranid
    }

    fn objid_make(&mut self, schema_type: MtSchemaType) -> MtOidT {
        if schema_type == MtSchemaType::KeyDataset {
            return next_dataset_id(self);
        }
        next_item_id(self)
    }

    fn connect_database(&mut self, _database: &str) -> MtStsT {
        STS_SUCCESS
    }

    fn disconnect_database(&mut self) {
        self.ltime = 0;
        self.tranid = 0;
    }

    fn activate_task(&self) {}

    fn deactivate_task(&mut self) {
        self.ltime = 0;
        self.tranid = 0;
    }

    fn start_local_qtran(&mut self) -> MtLTimeT {
        self.ltime = 0;
        self.ltime
    }

    fn start_tran(&mut self) -> MtOidT {
        if self.ltime != 0 {
            self.deactivate_task();
            self.activate_task();
            self.ltime = 0;
        }
        self.tranid = 0;
        self.tranid
    }

    fn tran_commit(&mut self) -> MtStsT {
        STS_SUCCESS
    }

    fn tran_abort(&self) -> MtStsT {
        STS_SUCCESS
    }

    fn create_item(
        &mut self,
        _tranid: MtOidT,
        dataset_id: MtOidT,
        obj_id: &mut MtOidT,
        data: &Vec<u8>,
        _data_size: MtSizeT,
    ) -> MtStsT {
        *obj_id = next_item_id(self);
        let item = tstdata::ItemPb {
            id: *obj_id as u32,
            jb_content: data.to_vec(),
        };
        #[allow(static_mut_refs)]
        if let Some(dataset) = self
            .demo_data
            .datasets
            .iter_mut()
            .find(|s| s.id == dataset_id as u32)
        {
            dataset.items.push(item);
            return STS_SUCCESS;
        }
        MTS_CLASSNOTFOUND
    }

    fn update_item(
        &mut self,
        _tranid: MtOidT,
        obj_id: MtOidT,
        data: &Vec<u8>,
        _data_size: MtSizeT,
    ) -> MtStsT {
        let item = tstdata::ItemPb {
            id: obj_id as u32,
            jb_content: data.to_vec(),
        };
        for dataset in self.demo_data.datasets.iter_mut() {
            // Update existing item if exists
            if let Some(pos) = dataset.items.iter().position(|i| i.id as MtOidT == obj_id) {
                dataset.items.remove(pos);
                dataset.items.push(item);
                return STS_SUCCESS;
            }
        }
        MTS_CLASSNOTFOUND
    }

    fn delete_item(&mut self, _tranid: MtOidT, ns: IdSpc, item_id: MtOidT) -> MtStsT {
        let mut candidate_data_spaces = vec![];
        match ns {
            IdSpc::DataPart => {
                for dataset in self.demo_data.datasets.iter_mut() {
                    if let Some(pos) = dataset.items.iter().position(|i| i.id as MtOidT == item_id)
                    {
                        dataset.items.remove(pos);
                        return STS_SUCCESS;
                    }
                }
            }
            IdSpc::Internal => {
                candidate_data_spaces.push(&mut self.demo_data.indexes);
                candidate_data_spaces.push(&mut self.demo_data.invrs_items);
            }
            IdSpc::RelPart => {
                candidate_data_spaces.push(&mut self.demo_data.rs_items);
            }
        };
        for items in candidate_data_spaces {
            if let Some(pos) = items.iter().position(|i| i.id as MtOidT == item_id) {
                items.remove(pos);
                return STS_SUCCESS;
            }
        }
        MTS_OBJNOTFOUND
    }

    fn write_dataset(
        &mut self,
        _tranid: MtOidT,
        ds_id: MtOidT,
        ds_name: &str,
        data: &Vec<u8>,
        _data_size: MtSizeT,
    ) -> MtStsT {
        let set_desc = tstdata::ItemPb {
            id: ds_id as u32,
            jb_content: data.to_vec(),
        };
        // Update existing dataset with matching id, if not found return error
        #[allow(static_mut_refs)]
        if let Some(d) = self
            .demo_data
            .datasets
            .iter_mut()
            .find(|s| s.id as MtOidT == ds_id)
        {
            d.descriptor = Some(set_desc);
            d.name = ds_name.to_string();
            return STS_SUCCESS;
        }
        // Append new dataset if not found
        self.demo_data.datasets.push(tstdata::DatasetPb {
            id: ds_id as u32,
            name: ds_name.to_string(),
            descriptor: Some(set_desc),
            items: vec![],
        });
        STS_SUCCESS
    }

    fn drop_dataset(&mut self, _tranid: MtOidT, dataset_id: MtOidT) -> MtStsT {
        // Find dataset with matching id and remove it
        #[allow(static_mut_refs)]
        if let Some(pos) = self
            .demo_data
            .datasets
            .iter()
            .position(|s| s.id as MtOidT == dataset_id)
        {
            self.demo_data.datasets.remove(pos);
            return STS_SUCCESS;
        }
        MTS_CLASSNOTFOUND
    }

    fn create_index(
        &mut self,
        _tranid: MtOidT,
        root_data: &Vec<u8>,
        root_id: &mut MtOidT,
    ) -> MtStsT {
        *root_id = next_item_id(self);
        let item = tstdata::ItemPb {
            id: *root_id as u32,
            jb_content: root_data.to_vec(),
        };
        self.demo_data.indexes.push(item);
        return STS_SUCCESS;
    }

    fn drop_index(&mut self, _tranid: MtOidT, root_id: MtOidT) -> MtStsT {
        #[allow(static_mut_refs)]
        if let Some(pos) = self
            .demo_data
            .indexes
            .iter()
            .position(|i| i.id as MtOidT == root_id)
        {
            self.demo_data.indexes.remove(pos);
            return STS_SUCCESS;
        }
        MTS_OBJNOTFOUND
    }

    fn write_index_page(
        &mut self,
        _tranid: MtOidT,
        index_page_id: MtOidT,
        page_data: &Vec<u8>,
        page_data_size: MtSizeT,
    ) -> MtStsT {
        // Find index page in DATA_CTX.indexes with matching id and update its content with page_data
        #[allow(static_mut_refs)]
        if let Some(item) = self
            .demo_data
            .indexes
            .iter_mut()
            .find(|i| i.id as MtOidT == index_page_id)
        {
            item.jb_content = page_data[..page_data_size as usize].to_vec();
            debug!(
                "Wrote index page for id {}: {:?} {} bytes",
                item.id, item.jb_content, page_data_size
            );
            return STS_SUCCESS;
        }
        MTS_OBJNOTFOUND
    }

    fn get_index_page(
        &self,
        _tranid: MtOidT,
        _ltime: MtLTimeT,
        index_page_id: MtOidT,
        page_data: &mut [u8; 32000],
        page_data_size: *mut MtSizeT,
    ) -> MtStsT {
        // loop in DATA_CTX.indexes and find item with id matching index_page_id, then return its content as page data
        unsafe {
            #[allow(static_mut_refs)]
            if let Some(item) = self
                .demo_data
                .indexes
                .iter()
                .find(|i| i.id as MtOidT == index_page_id)
            {
                let data = &item.jb_content;
                debug!(
                    "Read index page for id {}: {:?} {} bytes",
                    item.id,
                    item.jb_content,
                    data.len()
                );
                return copy_data_part(data, page_data, &mut *page_data_size);
            }
        }
        MTS_OBJNOTFOUND
    }

    fn get_schema_item(
        &self,
        _tranid: MtOidT,
        _ltime: MtLTimeT,
        schema_name: &str,
        _obj_type: MtSchemaType,
        schema_item: &mut MtOidT,
    ) -> MtStsT {
        if schema_name == "dataset" {
            *schema_item = META_DATASET_ID;
            return STS_SUCCESS;
        }
        // Scan DatasetPb for a dataset with the given name and return its id
        #[allow(static_mut_refs)]
        if let Some(set) = self
            .demo_data
            .datasets
            .iter()
            .find(|s| s.name == schema_name)
        {
            *schema_item = set.id as MtOidT;
            return STS_SUCCESS;
        }
        MTS_CLASSNOTFOUND
    }

    fn dataset_enum_start(
        &self,
        _ltime: MtLTimeT,
        dataset_id: MtOidT,
        set_stream: &mut MtOidT,
        ids: *mut MtOidT,
        num_ids: &mut MtSizeT,
    ) -> MtStsT {
        *num_ids = 0;
        if dataset_id == META_DATASET_ID {
            // Return all dataset ids in DATA_CTX
            *set_stream = META_DATASET_ID;
            unsafe {
                let ids_slice = std::slice::from_raw_parts_mut(ids, 100);
                for (i, set) in self.demo_data.datasets.iter().enumerate() {
                    ids_slice[i] = set.id as MtOidT;
                    *num_ids += 1;
                }
            }
            return MTS_ENDOFSTREAM;
        }
        if let Some(set) = self
            .demo_data
            .datasets
            .iter()
            .find(|s| s.id == dataset_id as u32)
        {
            *set_stream = set.id as MtOidT;
            unsafe {
                let ids_slice = std::slice::from_raw_parts_mut(ids, 100);
                for (i, item) in set.items.iter().enumerate() {
                    if i >= 100 {
                        break;
                    }
                    ids_slice[i] = item.id as MtOidT; // use item id
                    *num_ids += 1;
                }
            }
        } else {
            return MTS_CLASSNOTFOUND;
        }
        MTS_ENDOFSTREAM
    }

    fn dataset_enum_many(
        &self,
        _set_stream: MtOidT,
        _ids: *mut MtOidT,
        _num_ids: &mut MtSizeT,
    ) -> MtStsT {
        STS_SUCCESS
    }

    fn dataset_enum_end(&self, _set_stream: MtOidT) -> MtStsT {
        STS_SUCCESS
    }

    fn get_datapart(
        &self,
        _ltime: MtLTimeT,
        oid: MtOidT,
        dataset_id: &mut MtOidT,
        data_part: &mut [u8; 32000],
        data_size: &mut MtSizeT,
    ) -> MtStsT {
        read_datapart(self, oid, dataset_id, data_part, data_size)
    }

    fn get_relpart(
        &self,
        _ltime: MtLTimeT,
        oid: MtOidT,
        inverse: bool,
        data_part: &mut [u8; 32000],
        data_size: &mut MtSizeT,
    ) -> MtStsT {
        let data_space = if inverse {
            IdSpc::Internal
        } else {
            IdSpc::RelPart
        };
        read_rs_datapart(self, oid, data_space, data_part, data_size)
    }

    fn write_relpart(
        &mut self,
        _tranid: MtOidT,
        oid: MtOidT,
        inverse: bool,
        data_part: &Vec<u8>,
        data_size: MtSizeT,
    ) -> MtStsT {
        let data_space = if inverse {
            IdSpc::Internal
        } else {
            IdSpc::RelPart
        };
        let items = match data_space {
            IdSpc::Internal => &self.demo_data.invrs_items,
            _ => &self.demo_data.rs_items,
        };
        // Check if item exists, if so delete it first
        #[allow(static_mut_refs)]
        if let Some(pos) = items.iter().position(|i| i.id as MtOidT == oid) {
            match data_space {
                IdSpc::Internal => self.demo_data.invrs_items.remove(pos),
                _ => self.demo_data.rs_items.remove(pos),
            };
        }
        // Insert new item with oid and data_part as content
        let new_item = tstdata::ItemPb {
            id: oid as u32,
            jb_content: data_part[..data_size as usize].to_vec(),
        };
        #[allow(static_mut_refs)]
        match data_space {
            IdSpc::Internal => self.demo_data.invrs_items.push(new_item),
            _ => self.demo_data.rs_items.push(new_item),
        };
        debug!("Write rel: space {:?} rels part id {}", data_space, oid);
        for item in self.demo_data.rs_items.iter() {
            let relinfo = relpart::get_rels(&item.jb_content);
            debug!("Item id {}: relinfo {:?}", item.id, relinfo);
        }
        for item in self.demo_data.invrs_items.iter() {
            let relinfo = relpart::get_rels(&item.jb_content);
            debug!("Item id {}: inv relinfo {:?}", item.id, relinfo);
        }
        STS_SUCCESS
    }
}

fn next_item_id(ctx: &mut DemoContextT) -> MtOidT {
    ctx.demo_data.next_item_id += 1;
    ctx.demo_data.next_item_id
}

fn next_dataset_id(ctx: &mut DemoContextT) -> MtOidT {
    ctx.demo_data.next_dataset_id += 1;
    ctx.demo_data.next_dataset_id
}

fn copy_data_part(data: &[u8], data_part: &mut [u8; 32000], data_size: &mut MtSizeT) -> MtStsT {
    let len = data.len().min(32000);
    data_part[..len].copy_from_slice(&data[..len]);
    *data_size = len as MtSizeT;
    STS_SUCCESS
}

fn read_rs_datapart(
    ctx: &DemoContextT,
    oid: MtOidT,
    ns: IdSpc,
    data_part: &mut [u8; 32000],
    data_size: &mut MtSizeT,
) -> MtStsT {
    let items = match ns {
        IdSpc::Internal => &ctx.demo_data.invrs_items,
        _ => &ctx.demo_data.rs_items,
    };
    for item in items.iter() {
        if item.id as MtOidT == oid {
            let data = &item.jb_content;
            debug!(
                "Read data part for item id {}: {:?} {} bytes",
                item.id,
                item.jb_content,
                data.len()
            );
            return copy_data_part(data, data_part, data_size);
        }
    }
    MTS_OBJNOTFOUND
}

fn read_datapart(
    ctx: &DemoContextT,
    oid: MtOidT,
    dataset_id: &mut MtOidT,
    data_part: &mut [u8; 32000],
    data_size: &mut MtSizeT,
) -> MtStsT {
    // Loop through all datasets in DATA_CTX and find the one with matching dataset_id, then find item in that set with matching id and return its content as data part
    for dataset in ctx.demo_data.datasets.iter() {
        // If oid is less than MIN_ENTITY_ID, treat it as meta 'dataset' and look for matching dataset_id
        if oid < MIN_ENTITY_ID {
            if dataset.id as MtOidT == oid {
                let desc = dataset.descriptor.as_ref().unwrap();
                let data = &desc.jb_content;
                debug!(
                    "Read data part for dataset id {}: {:?} {} bytes",
                    dataset.id,
                    data,
                    data.len()
                );
                *dataset_id = META_DATASET_ID;
                return copy_data_part(data, data_part, data_size);
            }
        }
        // Otherwise look for oid in all datasets
        for item in dataset.items.iter() {
            if item.id as MtOidT == oid {
                let data = &item.jb_content;
                debug!(
                    "Read data part for item id {}: {:?} {} bytes",
                    item.id,
                    item.jb_content,
                    data.len()
                );
                *dataset_id = dataset.id as MtOidT;
                return copy_data_part(data, data_part, data_size);
            }
        }
    }
    MTS_OBJNOTFOUND
}
