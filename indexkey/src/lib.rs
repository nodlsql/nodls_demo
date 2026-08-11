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

use indexcapn::indexkey_capnp::{
    dup_id_union, dup_index_key, index_page, key_component, uniq_index_key,
};
use sqlexet::MtOidT;

use sqlinsts::{sql_value_pb::Data, DecimalValuePb, SqlValuePb};
use std::cell::RefCell;

#[macro_export]
macro_rules! get_index_page_reader {
    ($serialized_page:ident) => {
        // Deserialize the existing index_page
        if let Ok(reader) = ::capnp::serialize::read_message(
            &mut std::io::Cursor::new($serialized_page),
            ::capnp::message::ReaderOptions::new(),
        ) {
            reader
        } else {
            return Err("Failed to read message".to_string());
        }
    };
}

pub fn create_page(unique: bool) -> Vec<u8> {
    // Create a new index_page Leaf message
    let mut serialized_page = Vec::new();
    let mut new_page_message = ::capnp::message::Builder::new_default();
    new_page_message.init_root::<index_page::Builder>();
    // init leaf page with empty entries list
    let page_builder = new_page_message.get_root::<index_page::Builder>().unwrap();
    if unique {
        page_builder.init_uniq_key_entries(0);
    } else {
        page_builder.init_dup_key_entries(0);
    }
    ::capnp::serialize::write_message(&mut serialized_page, &new_page_message)
        .expect("create root failed");
    serialized_page
}

pub trait IndexKeyBuilder {
    fn set_id_values(&mut self, id_values: &[MtOidT]);
    fn init_components<'b>(
        &'b mut self,
        nb_cmpts: u32,
    ) -> capnp::struct_list::Builder<'b, key_component::Owned>;
}

pub trait IndexKeyReader {
    fn get_components<'b>(
        &'b self,
    ) -> Result<capnp::struct_list::Reader<'b, key_component::Owned>, capnp::Error>;

    fn get_id_values(&self) -> Vec<MtOidT>;
}

impl<'a> IndexKeyBuilder for uniq_index_key::Builder<'a> {
    fn set_id_values(&mut self, id_values: &[MtOidT]) {
        self.set_id_value(id_values[0]);
    }

    fn init_components<'b>(
        &'b mut self,
        nb_cmpts: u32,
    ) -> capnp::struct_list::Builder<'b, key_component::Owned> {
        self.reborrow().init_cmpts(nb_cmpts)
    }
}

impl<'a> IndexKeyBuilder for dup_index_key::Builder<'a> {
    fn set_id_values(&mut self, id_values: &[MtOidT]) {
        let mut dup_id_value = self.reborrow().init_dup_id_value();
        if id_values.len() == 1 {
            dup_id_value.set_uniq_id_value(id_values[0]);
        } else {
            let mut list_builder = dup_id_value
                .reborrow()
                .init_list_id_value(id_values.len() as u32);
            for (i, id) in id_values.iter().enumerate() {
                list_builder.set(i as u32, *id);
            }
        }
    }

    fn init_components<'b>(
        &'b mut self,
        nb_cmpts: u32,
    ) -> capnp::struct_list::Builder<'b, key_component::Owned> {
        self.reborrow().init_cmpts(nb_cmpts)
    }
}

impl<'a> IndexKeyReader for uniq_index_key::Reader<'a> {
    fn get_components<'b>(
        &'b self,
    ) -> Result<capnp::struct_list::Reader<'b, key_component::Owned>, capnp::Error> {
        self.get_cmpts()
    }

    fn get_id_values(&self) -> Vec<MtOidT> {
        vec![self.get_id_value()]
    }
}

impl<'a> IndexKeyReader for dup_index_key::Reader<'a> {
    fn get_components<'b>(
        &'b self,
    ) -> Result<capnp::struct_list::Reader<'b, key_component::Owned>, capnp::Error> {
        self.get_cmpts()
    }

    fn get_id_values(&self) -> Vec<MtOidT> {
        let dup_id_value = self.get_dup_id_value().unwrap();
        id_values_to_vec(dup_id_value)
    }
}

fn read_key_components<R: IndexKeyReader>(key_reader: &R) -> Vec<SqlValuePb> {
    let mut values = Vec::new();

    if let Ok(components) = key_reader.get_components() {
        for j in 0..components.len() {
            let component = components.get(j as u32);
            match component.which().unwrap() {
                key_component::Which::StrCpt(s) => {
                    values.push(SqlValuePb {
                        is_constant: false,
                        data: Some(Data::StringValue(s.unwrap().to_string().unwrap())),
                    });
                }
                key_component::Which::DecimalCpt(d) => {
                    values.push(SqlValuePb {
                        is_constant: false,
                        data: Some(Data::DecimalValue(DecimalValuePb {
                            number: d.clone().unwrap().get_number(),
                            scale: d.clone().unwrap().get_scale() as u32,
                        })),
                    });
                }
                key_component::Which::Int64Cpt(i) => {
                    values.push(SqlValuePb {
                        is_constant: false,
                        data: Some(Data::Int64Value(i)),
                    });
                }
                key_component::Which::BoolCpt(b) => {
                    values.push(SqlValuePb {
                        is_constant: false,
                        data: Some(Data::BoolValue(b)),
                    });
                }
                key_component::Which::NullCpt(_) => values.push(SqlValuePb {
                    is_constant: false,
                    data: Some(Data::NullValue(true)),
                }),
                key_component::Which::NoneCpt(_) => values.push(SqlValuePb {
                    is_constant: false,
                    data: None,
                }),
            }
        }
    }

    values
}

pub fn build_index_key<T: IndexKeyBuilder>(
    id_values: &[MtOidT],
    values: &Vec<RefCell<SqlValuePb>>,
    key_builder: &mut T,
) -> () {
    key_builder.set_id_values(id_values);

    // Initialize the entries list for the index key
    let mut cmpts = key_builder.init_components(values.len() as u32);

    // Populate each KeyComponent
    for (idx, value_cell) in values.iter().enumerate() {
        let value = value_cell.borrow();
        let mut component = cmpts.reborrow().get(idx as u32);

        // Set the appropriate union field based on the SqlValuePb data type
        match &value.data {
            Some(Data::StringValue(s)) => {
                component.set_str_cpt(s.as_str());
            }
            Some(Data::DecimalValue(d)) => {
                let mut decimal_value = component.reborrow().init_decimal_cpt();
                decimal_value.set_number(d.number);
                decimal_value.set_scale(d.scale as i32);
            }
            Some(Data::Int64Value(i)) => {
                component.set_int64_cpt(*i);
            }
            Some(Data::BoolValue(b)) => {
                component.set_bool_cpt(*b);
            }
            Some(Data::NullValue(_)) => {
                component.set_null_cpt(());
            }
            None => {
                component.set_none_cpt(());
            }
            _ => {}
        }
    }
}

pub fn read_key<T: IndexKeyReader>(key_reader: &T) -> (Vec<MtOidT>, Vec<SqlValuePb>) {
    let oid_values = key_reader.get_id_values();
    let values = read_key_components(key_reader);
    (oid_values, values)
}

pub fn copy_key<T: IndexKeyReader, U: IndexKeyBuilder>(
    source_key_reader: &T,
    dest_key_builder: &mut U,
) -> () {
    dest_key_builder.set_id_values(&source_key_reader.get_id_values());
    let source_cpts = source_key_reader.get_components().unwrap();
    let mut dest_cpts = dest_key_builder.init_components(source_cpts.len() as u32);

    for j in 0..source_cpts.len() {
        let src_cpts = source_cpts.get(j as u32);
        let mut dst_cpts = dest_cpts.reborrow().get(j as u32);

        // Copy the union field
        match src_cpts.which().unwrap() {
            key_component::Which::StrCpt(s) => {
                dst_cpts.set_str_cpt(s.unwrap());
            }
            key_component::Which::Int64Cpt(i) => {
                dst_cpts.set_int64_cpt(i);
            }
            key_component::Which::BoolCpt(b) => {
                dst_cpts.set_bool_cpt(b);
            }
            key_component::Which::NullCpt(_) => {
                dst_cpts.set_null_cpt(());
            }
            key_component::Which::NoneCpt(_) => {
                dst_cpts.set_none_cpt(());
            }
            key_component::Which::DecimalCpt(d) => {
                let mut decimal_value = dst_cpts.reborrow().init_decimal_cpt();
                decimal_value.set_number(d.clone().unwrap().get_number());
                decimal_value.set_scale(d.clone().unwrap().get_scale() as i32);
            }
        }
    }
}

pub fn get_index_key_entries<'a>(
    page_reader: &'a index_page::Reader,
) -> Result<capnp::struct_list::Reader<'a, uniq_index_key::Owned>, String> {
    match page_reader.which().map_err(|e| e.to_string())? {
        index_page::Which::UniqKeyEntries(entries) => {
            let reader = entries.unwrap();
            return Ok(reader);
        }
        index_page::Which::DupKeyEntries(_) => {
            return Err("Duplicate key entries".to_string());
        }
    }
}

pub fn insert_key_from_values(
    serialized_page: &[u8],
    oid_key: MtOidT,
    values: &Vec<RefCell<SqlValuePb>>,
    insert_pos: usize,
    key_found: bool,
) -> Result<Vec<u8>, String> {
    let reader = get_index_page_reader!(serialized_page);
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
    let mut new_entries = page_builder.init_uniq_key_entries(new_count as u32);

    // Copy entries before the insertion point
    for i in 0..insert_pos {
        let src_key_entry = page_reader.get(i as u32);
        let mut dst_key_entry = new_entries.reborrow().get(i as u32);
        copy_key(&src_key_entry, &mut dst_key_entry);
    }

    // Insert the new key at the insertion point
    let mut new_key_entry = new_entries.reborrow().get(insert_pos as u32);
    build_index_key(&vec![oid_key], values, &mut new_key_entry);

    // Copy entries after the insertion point
    for i in insert_pos..existing_count as usize {
        let src_key_entry = page_reader.get(i as u32);
        let mut dst_key_entry = new_entries.reborrow().get((i + 1) as u32);
        copy_key(&src_key_entry, &mut dst_key_entry);
    }

    // Serialize the new page to bytes
    let mut buffer = Vec::new();
    ::capnp::serialize::write_message(&mut buffer, &new_page_message).map_err(|e| e.to_string())?;
    Ok(buffer)
}

pub fn delete_key_at_position(
    serialized_page: &[u8],
    delete_pos: usize,
) -> Result<Vec<u8>, String> {
    let reader = get_index_page_reader!(serialized_page);
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
    let new_count = existing_count - 1;
    // Initialize the key_entries list with the new size
    let mut new_entries = page_builder.init_uniq_key_entries(new_count as u32);

    // Copy entries before the deletion point
    for i in 0..delete_pos {
        let src_key_entry = page_reader.get(i as u32);
        let mut dst_key_entry = new_entries.reborrow().get(i as u32);
        copy_key(&src_key_entry, &mut dst_key_entry);
    }

    // Copy entries after the deletion point
    for i in (delete_pos + 1)..existing_count as usize {
        let src_key_entry = page_reader.get(i as u32);
        let mut dst_key_entry = new_entries.reborrow().get((i - 1) as u32);
        copy_key(&src_key_entry, &mut dst_key_entry);
    }
    // Serialize the new page to bytes
    let mut buffer = Vec::new();
    ::capnp::serialize::write_message(&mut buffer, &new_page_message).map_err(|e| e.to_string())?;
    Ok(buffer)
}

pub fn id_values_to_vec(dup_id_value: dup_id_union::Reader) -> Vec<MtOidT> {
    match dup_id_value.which().unwrap() {
        dup_id_union::Which::UniqIdValue(id) => vec![id],
        dup_id_union::Which::ListIdValue(ids) => {
            let ids_reader = ids.unwrap();
            let mut ids = Vec::new();
            for i in 0..ids_reader.len() {
                ids.push(ids_reader.get(i));
            }
            ids
        }
        _ => panic!("Unexpected id_value type"),
    }
}

pub fn pretty_print_index_page(serialized_page: &[u8]) -> Result<(), String> {
    let reader = get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    let page_reader = get_index_key_entries(&reader)?;
    for i in 0..page_reader.len() {
        let key_entry = page_reader.get(i as u32);
        let (id_values, values) = read_key(&key_entry);
        println!("Key {}: ids: {:?}, values: {:?}", i, id_values, values);
    }
    Ok(())
}
