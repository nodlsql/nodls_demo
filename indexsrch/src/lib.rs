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

use indexcapn::indexkey_capnp::{dup_index_key, index_page, key_component, uniq_index_key};
use rust_decimal::Decimal;
use sqlinsts::{sql_value_pb::Data, CompOperatorPb, SqlValuePb};
use std::cell::RefCell;
use std::cmp::Ordering;

use tracing::debug;

#[derive(Debug, PartialEq)]
pub enum ScanOutput {
    More,
    Done,
}

pub fn index_scan(
    serialized_page: &[u8],
    start_key: &Vec<RefCell<SqlValuePb>>,
    start_comp: &Vec<CompOperatorPb>,
    end_key: &Vec<RefCell<SqlValuePb>>,
    end_comp: &Vec<CompOperatorPb>,
    uniq: bool,
) -> Result<Option<(Vec<u32>, ScanOutput)>, String> {
    // TBD - handle errors
    let reader = indexkey::get_index_page_reader!(serialized_page);
    let reader = reader
        .get_root::<index_page::Reader>()
        .map_err(|e| e.to_string())?;
    if uniq {
        let page_reader = indexkey::get_index_key_entries(&reader).map_err(|e| e.to_string())?;
        index_scan_with_reader(&page_reader, start_key, start_comp, end_key, end_comp)
    } else {
        let page_reader = dupindexkey::get_index_key_entries(&reader).map_err(|e| e.to_string())?;
        index_scan_with_reader(&page_reader, start_key, start_comp, end_key, end_comp)
    }
}

fn index_scan_with_reader<T: IndexKeyListReader>(
    page_reader: &T,
    start_key: &Vec<RefCell<SqlValuePb>>,
    start_comp: &Vec<CompOperatorPb>,
    end_key: &Vec<RefCell<SqlValuePb>>,
    end_comp: &Vec<CompOperatorPb>,
) -> Result<Option<(Vec<u32>, ScanOutput)>, String> {
    let nb_entries = page_reader.len() as i32;
    debug!("Index scan - nb entries in page: {}", nb_entries);
    if nb_entries == 0 {
        return Ok(None); // No entries in the index page
    }
    // TBD - doesn't handle first segment missing but 2nd or more segments there.
    // TBD - in that case full index scan is better than class scan still.
    let mut start_pos = 0i32;
    if !start_key.is_empty() {
        let search_res = index_binary_search(page_reader, start_key);
        // If strict, start one after if found
        let mut strict_cmp = 0i32;
        for (_i, comp) in start_comp.iter().enumerate() {
            if *comp == CompOperatorPb::Gt {
                strict_cmp = 1i32;
                break;
            }
        }
        start_pos = match search_res {
            SearchRes::Found(pos) => pos as i32 + strict_cmp,
            SearchRes::NotFound(pos) => pos as i32,
        };
        // Special case for equi comparison, we can return immediately if match or no match
        let mut equi_comp = true;
        for i in 0..start_comp.len() {
            if start_comp[i] != CompOperatorPb::Eq && start_comp[i] != CompOperatorPb::In {
                equi_comp = false;
                break;
            }
        }
        // Special case for equi comparison and number of components match start key length
        if equi_comp && start_pos >= 0 && start_pos < nb_entries {
            let nbsegs = page_reader.get_cmpts_at(start_pos as u32).unwrap().len();
            if nbsegs == start_key.len() as u32 {
                if let SearchRes::NotFound(_) = search_res {
                    return Ok(None); // No results if equality comparison and key not found
                } else {
                    return Ok(Some((
                        page_reader.get_id_values_at(start_pos as u32),
                        ScanOutput::Done,
                    )));
                }
            }
        }
    }
    // Assume open ended
    let mut end_pos = nb_entries;
    let mut end_comp_le = true;
    for c in end_comp.iter() {
        if *c != CompOperatorPb::Le {
            end_comp_le = false;
            break;
        }
    }
    if !end_key.is_empty() {
        // If strict, stop one before if found
        let mut strict_cmp = 0i32;
        for (_i, comp) in end_comp.iter().enumerate() {
            if *comp == CompOperatorPb::Lt {
                strict_cmp = 1i32;
                break;
            }
        }
        let search_res = index_binary_search(page_reader, end_key);
        end_pos = match search_res {
            SearchRes::Found(pos) => pos as i32 - strict_cmp,
            SearchRes::NotFound(pos) => pos as i32,
        };
    }
    let mut result_oids = vec![];
    // Check if any valid range exists
    if start_pos > end_pos || end_pos < 0 || start_pos >= nb_entries {
        return Ok(None); // No results if start is greater than end
    }
    // Adjust end position if open ended search
    if end_pos >= nb_entries {
        end_pos -= 1;
    }
    for i in start_pos..=end_pos {
        let oids = page_reader.get_id_values_at(i as u32);
        debug!(
            "Index entry at position {}: oids={:?} start/end pos: {}/{}",
            i, oids, start_pos, end_pos
        );
        result_oids.extend(oids);
    }
    // We are done if there is an end key and we haved reached it.
    // For instance with (1,2,3) keys, lenght 3:
    // - search LE 3 -> end pos 2 => done if end pos <= len -1
    // - search LE 4 -> end pos 3 => done if end pos <= len
    // - search LT 3 -> end pos 1 => done if end pos <= len -1
    // - search LT 4 -> end pos 2 => done if end pos <= len -1
    let scan_state = if !end_key.is_empty()
        && ((end_comp_le && end_pos <= nb_entries) || end_pos < nb_entries - 1)
    {
        ScanOutput::Done
    } else {
        ScanOutput::More
    };
    Ok(Some((result_oids, scan_state)))
}

pub fn compare_keys(
    values: &Vec<RefCell<SqlValuePb>>,
    components: &capnp::struct_list::Reader<key_component::Owned>,
) -> Ordering {
    let mut comp = Ordering::Equal;
    for j in 0..components.len() {
        let valref_opt = values.get(j as usize);
        let valref = match valref_opt {
            Some(v) => v,
            None => {
                // Less values than key components, return less
                comp = Ordering::Less;
                break;
            }
        };
        let component = components.get(j as u32);
        debug!(
            "Comparing key component {:?} with value {:?}",
            component,
            valref.borrow()
        );
        match valref.borrow().data.as_ref() {
            // None has precedence over explicit null value '{a: null}'
            None => {
                match component.which().unwrap() {
                    key_component::Which::NoneCpt(_) => {
                        comp = Ordering::Equal;
                    }
                    _ => {
                        // None is less than any non-null value
                        comp = Ordering::Less;
                        break;
                    }
                }
            }
            Some(Data::NullValue(_)) => {
                match component.which().unwrap() {
                    key_component::Which::NoneCpt(_) => {
                        comp = Ordering::Greater;
                    }
                    key_component::Which::NullCpt(_) => {
                        comp = Ordering::Equal;
                    }
                    _ => {
                        // Null is less than any non-null value
                        comp = Ordering::Less;
                        break;
                    }
                }
            }
            Some(Data::StringValue(s)) => {
                match component.which().unwrap() {
                    key_component::Which::StrCpt(s_cpt) => {
                        let s_cpt_val = s_cpt.unwrap().to_string().unwrap();
                        comp = s.cmp(&s_cpt_val);
                        if comp != Ordering::Equal {
                            break;
                        }
                        debug!("Comparing '{}' with '{}', result: {:?}", s, s_cpt_val, comp);
                    }
                    // Assume string is greater than int
                    key_component::Which::Int64Cpt(_) => {
                        comp = Ordering::Greater;
                        break;
                    }
                    // String greater than null
                    _ => {
                        comp = Ordering::Greater;
                        break;
                    }
                }
            }
            Some(Data::DecimalValue(d)) => {
                match component.which().unwrap() {
                    key_component::Which::DecimalCpt(d_cpt) => {
                        let d_cpt_number = d_cpt.clone().unwrap().get_number();
                        let d_cpt_scale = d_cpt.clone().unwrap().get_scale();
                        let v1 = Decimal::new(d.number, (d.scale as u8).into());
                        let v2 = Decimal::new(d_cpt_number, (d_cpt_scale as u8).into());
                        comp = v1.cmp(&v2);
                        debug!(
                            "Comparing decimal value {:?} with idx decimal {:?} comp: {:?}",
                            d, d_cpt, comp
                        );
                        if comp != Ordering::Equal {
                            break;
                        }
                    }
                    key_component::Which::Int64Cpt(i) => {
                        let v1 = Decimal::new(d.number, (d.scale as u8).into());
                        let v2 = Decimal::new(i, 0);
                        comp = v1.cmp(&v2);
                        debug!(
                            "Comparing decimal value {:?} with idx int {} comp: {:?}",
                            d, i, comp
                        );
                        if comp != Ordering::Equal {
                            break;
                        }
                    }
                    key_component::Which::StrCpt(_) => {
                        // Assume decimal is less than string
                        comp = Ordering::Less;
                        break;
                    }
                    // Decimal greater than null or boolean
                    _ => {
                        comp = Ordering::Greater;
                        break;
                    }
                }
            }
            Some(Data::Int64Value(i)) => {
                match component.which().unwrap() {
                    key_component::Which::DecimalCpt(d_cpt) => {
                        let d_cpt_number = d_cpt.clone().unwrap().get_number();
                        let d_cpt_scale = d_cpt.clone().unwrap().get_scale();
                        let v1 = Decimal::new(*i, 0);
                        let v2 = Decimal::new(d_cpt_number, (d_cpt_scale as u8).into());
                        comp = v1.cmp(&v2);
                        debug!(
                            "Comparing int value {:?} with idx decimal {:?} comp: {:?}",
                            i, d_cpt, comp
                        );
                        if comp != Ordering::Equal {
                            break;
                        }
                    }
                    key_component::Which::Int64Cpt(i_cpt) => {
                        comp = i.cmp(&i_cpt);
                        if comp != Ordering::Equal {
                            break;
                        }
                        debug!("Comparing '{}' with '{}', result: {:?}", i, i_cpt, comp);
                    }
                    key_component::Which::StrCpt(_) => {
                        // Assume int is less than string
                        comp = Ordering::Less;
                        break;
                    }
                    _ => {
                        // Int greater than null
                        comp = Ordering::Greater;
                        break;
                    }
                }
            }
            Some(Data::BoolValue(b)) => {
                match component.which().unwrap() {
                    key_component::Which::BoolCpt(b_cpt) => {
                        comp = b.cmp(&b_cpt);
                        if comp != Ordering::Equal {
                            break;
                        }
                        debug!("Comparing '{}' with '{}', result: {:?}", b, b_cpt, comp);
                    }
                    key_component::Which::StrCpt(_) | key_component::Which::Int64Cpt(_) => {
                        // Assume bool is less than string or int
                        comp = Ordering::Less;
                        break;
                    }
                    _ => {
                        // Bool greater than null
                        comp = Ordering::Greater;
                        break;
                    }
                }
            }
            Some(Data::OidValue(_)) => {
                // TBD
            }
        }
    } // for j
    if comp == Ordering::Equal && values.len() > components.len() as usize {
        // More values than key components, return greater
        comp = Ordering::Greater;
    }
    debug!(
        "Comparing key with values - key components: {:?} values: {:?} result: {:?}",
        components, values, comp
    );
    comp
}

#[derive(Debug, PartialEq, Eq)]
pub enum SearchRes {
    // index found at position
    Found(u32),
    // index not found, insertion point:
    // . shift right to insert at this position
    // . insert at end if out of bounds, i.e. target if greater than all elements
    NotFound(u32),
}

pub trait IndexKeyListReader {
    fn len(&self) -> u32;
    fn get_id_values_at(&self, index: u32) -> Vec<u32>;
    fn get_cmpts_at(
        &self,
        index: u32,
    ) -> Result<capnp::struct_list::Reader<'_, key_component::Owned>, capnp::Error>;
}

impl<'a> IndexKeyListReader for capnp::struct_list::Reader<'_, uniq_index_key::Owned> {
    fn len(&self) -> u32 {
        self.len()
    }

    fn get_id_values_at(&self, index: u32) -> Vec<u32> {
        vec![self.get(index).get_id_value()]
    }

    fn get_cmpts_at(
        &self,
        index: u32,
    ) -> Result<capnp::struct_list::Reader<'_, key_component::Owned>, capnp::Error> {
        self.get(index).get_cmpts()
    }
}

impl<'a> IndexKeyListReader for capnp::struct_list::Reader<'_, dup_index_key::Owned> {
    fn len(&self) -> u32 {
        self.len()
    }

    fn get_id_values_at(&self, index: u32) -> Vec<u32> {
        let dup_id_value = self.get(index).get_dup_id_value().unwrap();
        indexkey::id_values_to_vec(dup_id_value)
    }

    fn get_cmpts_at(
        &self,
        index: u32,
    ) -> Result<capnp::struct_list::Reader<'_, key_component::Owned>, capnp::Error> {
        self.get(index).get_cmpts()
    }
}

pub fn index_binary_search<T: IndexKeyListReader>(
    key_list_reader: &T,
    target: &Vec<RefCell<SqlValuePb>>,
) -> SearchRes {
    let mut left = 0;
    let mut right = key_list_reader.len(); // use len() from custom trait

    while left < right {
        let mid = left + (right - left) / 2;
        let components = key_list_reader.get_cmpts_at(mid as u32).unwrap();
        let cmp_result = compare_keys(target, &components);

        match cmp_result {
            Ordering::Less => right = mid,
            Ordering::Greater => left = mid + 1,
            Ordering::Equal => return SearchRes::Found(mid),
        }
    }
    SearchRes::NotFound(left)
}
