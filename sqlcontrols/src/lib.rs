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

use core::str;
use jbparse;
use sqlexet::{
    IdSpc, MtOidT, MtSchemaType, SqlExeTrait, MIN_USER_DATASET_ID, MTS_ENDOFSTREAM, STS_SUCCESS,
};

use sqlinsts::{
    sql_inst_pb::Inst, sql_value_pb, CompOperatorPb, DdlOpPb, IComparePb, IDatapathPb, IDatasetPb,
    IDeletePb, IIndexPb, IProjPb, IRelPb, IRelUpdPb, IUpdatePb, IYankPb, IndexOpPb, IndexTypePb,
    RelOpPb, SqlPlanPb, SqlStmtPb, SqlValuePb,
};

use utils::SqlExecError;

use std::cell::RefCell;
use std::os::raw::c_uint;
use std::vec;

use tracing::debug;

pub mod utils;

macro_rules! next_select_stmt_exec {
    // Match two arguments: an identifier ($i:ident) and an expression ($e:expr)
    ($c: ident, $p: ident, $o: expr, $v: ident, $l: ident, $r: ident) => {
        // The macro expands to this code block
        match select_stmt_exec($c, $p, $o, $v, $l, $r) {
            Ok(res) => {
                debug!("Instruction execution offset: {} result: {:?}", $o, res);
            }
            Err(e) => {
                println!("Error at execution offset: {} result: {:?}", $o, e);
                return Err(SqlExecError::ExecutionError(format!(
                    "Failed to execute instruction at offset {}: {}",
                    $o, e
                )));
            }
        }
    };
}

macro_rules! get_id_value {
    ($v: ident, $idx: expr) => {
        if let Some(sql_value_pb::Data::OidValue(id)) = $v[$idx].borrow().data {
            id
        } else {
            return Err(SqlExecError::ExecutionError(format!(
                "Expected OID value at index {}, got {:?}",
                $idx,
                $v[$idx].borrow().data
            )));
        }
    };
}

macro_rules! get_ds_desc {
    ($c: ident, $ds_id: expr) => {
        if let Some(ds_desc) = sqloptimize::utils::get_dataset_desc($c, $ds_id) {
            ds_desc
        } else {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to get dataset descriptor for dataset ID 0x{:x}",
                $ds_id
            )));
        }
    };
}

pub fn stmt_exec(ctxt: &mut impl SqlExeTrait, input: &str) -> Result<Vec<String>, SqlExecError> {
    // Parse
    let mut ast = sqlparser::parse_stmt(input)?;
    // Translate to SQL plan
    ctxt.activate_task();
    ctxt.clear_counts();
    let plan = sqlplan::translate(ctxt, &mut ast)?;
    debug!("SQL Plan: {:?}", plan);
    // Execute
    debug!("Values: {:?}", plan.values);
    let mut values: Vec<RefCell<SqlValuePb>> = vec![];
    for val in &plan.values {
        values.push(RefCell::new(val.clone()));
    }

    let stmt = plan.sqlstmt();
    // Count how many proj insts we have
    let proj_count = plan
        .insts
        .iter()
        .filter(|inst_option| {
            if let Some(Inst::Proj(_)) = &inst_option.inst {
                return true;
            }
            false
        })
        .count();
    let proj_cols = &mut vec!["".to_string(); proj_count];
    debug!(
        "Number of projection instructions: {} proj_cols: {:?}",
        proj_count, proj_cols
    );
    let mut proj_rows: Vec<String> = vec![];
    let res = match stmt {
        SqlStmtPb::CreateDataset => alter_dataset_stmt_exec(ctxt, &plan),
        SqlStmtPb::AlterDataset => alter_dataset_stmt_exec(ctxt, &plan),
        SqlStmtPb::Select
        | SqlStmtPb::DeleteFrom
        | SqlStmtPb::UpdateRel
        | SqlStmtPb::Update
        | SqlStmtPb::Yank
        | SqlStmtPb::InsertInto => {
            select_stmt_exec(ctxt, &plan, 0, &values, proj_cols, &mut proj_rows)
        }
    };
    ctxt.deactivate_task();
    if let Err(e) = res {
        return Err(e);
    }
    Ok(proj_rows)
}

fn alter_dataset_stmt_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
) -> Result<String, SqlExecError> {
    debug!("Alter dataset descriptor plan: {:?}", plan);
    ctxt.start_tran();
    // either from inst of just created for create dataset statement, or from previous alter statement
    let mut ds_id = 0;
    let mut create_ds = false;
    let mut ds_desc = jbparse::DatasetDesc {
        name: "".to_string(),
        _id: 0,
        rels: vec![],
        indexes: vec![],
    };
    for inst_option in &plan.insts {
        match inst_option.inst.as_ref() {
            // SqlInstPb { inst: Some(Index(IIndexPb { idx_type: Pkey, name: "", path: ["a.b", "c"], key_val_idx: -1, index_key: -1 })) }
            Some(Inst::DdlUpdate(d)) => {
                match DdlOpPb::try_from(d.op).unwrap() {
                    DdlOpPb::CreateDs => {
                        ds_desc.name = d.ds_name.clone();
                        ds_desc._id = ctxt.objid_make(sqlexet::MtSchemaType::KeyDataset);
                        ds_id = ds_desc._id;
                        create_ds = true;
                    }
                    DdlOpPb::DropDs => {
                        let sts = ctxt.drop_dataset(ctxt.get_tranid(), d.ds_id);
                        return if sts != STS_SUCCESS {
                            Err(SqlExecError::ExecutionError(
                                format! { "Failed to drop dataset '{}', sts=0x{:x}", d.ds_name, sts },
                            ))
                        } else {
                            ctxt.increment_count(sqlexet::UpdCounter::DropDataset(1));
                            Ok("".to_string())
                        };
                    }
                    DdlOpPb::DescribeDs => {
                        if ds_id == 0 {
                            ds_desc = get_ds_desc!(ctxt, d.ds_id);
                        }
                        let dsdesc_str = utils::pretty_print_dsdesc(&ds_desc);
                        println!("{}", dsdesc_str);
                        return Ok("".to_string());
                    }
                    DdlOpPb::CreateRel => {
                        if ds_id == 0 {
                            ds_desc = get_ds_desc!(ctxt, d.ds_id);
                            ds_id = d.ds_id;
                        }
                        // TBD - should verify rel target exists and has pkey index
                        // Verify if rel already exists
                        for r in &ds_desc.rels {
                            if r.name == d.name {
                                return Err(SqlExecError::ExecutionError(format!(
                                    "Failed to get dataset descriptor for dataset ID 0x{:x}",
                                    ds_id
                                )));
                            }
                        }
                        // TBD - should verify rel target exists and has pkey index
                        // Verify if rel already exists
                        for r in &ds_desc.rels {
                            if r.name == d.name {
                                return Err(SqlExecError::ExecutionError(format!(
                                    "Relationship '{}' already exists in dataset '{}'",
                                    d.name, ds_desc.name
                                )));
                            }
                        }
                        debug!("Creating rel: {:?}", d);
                        ds_desc.rels.push(jbparse::RelDesc {
                            name: d.name.clone(),
                            _id: ctxt.objid_make(MtSchemaType::KeyDataset),
                            tgt_dataset: d.tgt_ds_name.clone(),
                        });
                    }
                    DdlOpPb::DropRel => {
                        if ds_id == 0 {
                            ds_desc = get_ds_desc!(ctxt, d.ds_id);
                            ds_id = d.ds_id;
                        }
                        debug!("Dropping rel: {:?}", d);
                        let mut found = false;
                        for i in 0..ds_desc.rels.len() {
                            if ds_desc.rels[i].name == d.name {
                                ds_desc.rels.remove(i);
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return Err(SqlExecError::ExecutionError(format!(
                                "Failed to find rel '{}' in dataset '{}'",
                                d.name, ds_desc.name
                            )));
                        }
                    }
                    DdlOpPb::CreateIdx => {
                        if ds_id == 0 {
                            ds_desc = get_ds_desc!(ctxt, d.ds_id);
                            ds_id = d.ds_id;
                        }
                        debug!("Creating index '{}', path: {:?}", d.name, d.seg_strs);
                        // Verify if index already exists
                        for idx in &ds_desc.indexes {
                            if idx.name == d.name {
                                return Err(SqlExecError::ExecutionError(format!(
                                    "Index '{}' already exists in dataset '{}'",
                                    d.name, ds_desc.name
                                )));
                            }
                        }
                        let mut unique = true;
                        let idx_type = match IndexTypePb::try_from(d.idx_type).unwrap() {
                            IndexTypePb::Pkey => "pkey",
                            IndexTypePb::Unique => "unique",
                            _ => {
                                unique = false;
                                "default"
                            }
                        };

                        // Create capn root node
                        let mut root_key: c_uint = 0;
                        let sts = indexbt::bt_create_index(
                            ctxt,
                            ctxt.get_tranid(),
                            &mut root_key,
                            unique,
                        );
                        if sts == STS_SUCCESS {
                            debug!("Created index '{}', key={}", d.name, root_key);
                            ds_desc.indexes.push(jbparse::IndexDesc {
                                _id: root_key,
                                name: d.name.clone(),
                                idx_type: idx_type.to_string(),
                                segs: d.seg_strs.clone(),
                            });
                        } else {
                            debug!("Failed to create index '{}'", d.name);
                        }
                    }
                    DdlOpPb::DropIdx => {
                        if ds_id == 0 {
                            ds_desc = get_ds_desc!(ctxt, d.ds_id);
                            ds_id = d.ds_id;
                        }
                        debug!("Dropping index '{}'", d.name);
                        let mut found = false;
                        for i in 0..ds_desc.indexes.len() {
                            if ds_desc.indexes[i].name == d.name {
                                // Drop capn index root node
                                let sts = indexbt::bt_drop_index(
                                    ctxt,
                                    ctxt.get_tranid(),
                                    ds_desc.indexes[i]._id,
                                );
                                if sts == STS_SUCCESS {
                                    debug!(
                                        "Dropped index '{}', key={}",
                                        d.name, ds_desc.indexes[i]._id
                                    );
                                    ds_desc.indexes.remove(i);
                                    found = true;
                                } else {
                                    return Err(SqlExecError::ExecutionError(format!(
                                        "Failed to delete index dataset ID 0x{:x}",
                                        d.ds_id
                                    )));
                                }
                                break;
                            }
                        }
                        if !found {
                            return Err(SqlExecError::ExecutionError(format!(
                                "Failed to find index '{}' in dataset '{}'",
                                d.name, ds_desc.name
                            )));
                        }
                    }
                    _ => {}
                } // Match DdlOpPb
            }
            _ => {}
        }
    }
    // {"name":"xcv","indexes":[{"name":"xcv","segs":["a.b","c"]}]}
    let json_value = jsonb::to_owned_jsonb(&ds_desc).unwrap();
    let data_binary = json_value.to_vec();
    let data_size: c_uint = data_binary.len() as c_uint;
    let sts = ctxt.write_dataset(
        ctxt.get_tranid(),
        ds_desc._id,
        &ds_desc.name,
        &data_binary,
        data_size,
    );
    if sts != STS_SUCCESS {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to write dataset '{}', sts=0x{:x}",
            ds_desc.name, sts
        )));
    }
    let sts = ctxt.tran_commit();
    if sts != STS_SUCCESS {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to commit transaction '{}', sts=0x{:x}",
            ds_desc.name, sts
        )));
    }
    if create_ds {
        ctxt.increment_count(sqlexet::UpdCounter::CreateDataset(1));
    } else {
        ctxt.increment_count(sqlexet::UpdCounter::AlterDataset(1));
    }
    Ok("".to_string())
}

// This is a simple case, all values present in plan have to be inserted
// into the indexes. IIndex insts don't need to keep track of key_val_idx or others.
fn insert_into_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    iinsert: &sqlinsts::IInsertPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    let tranid = ctxt.start_tran();
    ctxt.set_tranid(tranid);
    // insert values
    for i in iinsert.val_idxs.iter().map(|idx| *idx as usize) {
        debug!("Inserting value {}: {:?}", i, values[i].borrow());
        let mut obj_key: MtOidT = 0;
        let valborrow = values[i].borrow();
        let value = valborrow.data.as_ref().unwrap();
        let val_str = match value {
            sql_value_pb::Data::StringValue(s) => s.as_str(),
            _ => "",
        };
        let res = jbparse::jsonstr_to_jsonb(val_str);
        if let Err(e) = res {
            return Err(SqlExecError::ExecutionError(format!("{}", e)));
        }
        let data_binary = res.unwrap();
        let data_size: c_uint = data_binary.len() as c_uint;
        let sts = ctxt.create_item(
            ctxt.get_tranid(),
            iinsert.ds_id,
            &mut obj_key,
            &data_binary,
            data_size,
        );
        if sts != STS_SUCCESS {
            // If any error create_item does abort the transaction
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to insert value {} into dataset '{:?}', sts=0x{:x}",
                i, iinsert.ds_id, sts
            )));
        }
        // Set the oid in key_val_idx
        values[0].borrow_mut().data = Some(sql_value_pb::Data::OidValue(obj_key));
        // Update the indexes if any
        next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
    }
    let commit_sts = ctxt.tran_commit();
    if commit_sts != STS_SUCCESS {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to commit transaction {} for insert into dataset {}, sts=0x{:x}",
            ctxt.get_tranid(),
            iinsert.ds_id,
            commit_sts
        )));
    }
    ctxt.increment_count(sqlexet::UpdCounter::Insert(iinsert.val_idxs.len() as i32));
    return Ok("".to_string());
}

fn irel_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    irel: &IRelPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Relationship traversal: {:?}", irel);

    // Get the current entity from dataset scan or index scan
    let oid = get_id_value!(values, irel.key_val_idx as usize);

    // Fetch the rel part for the current item id
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size: c_uint = data_part.len() as c_uint;
    let sts = ctxt.get_relpart(0u32, oid, irel.inverse, &mut data_part, &mut data_size);
    if sts != STS_SUCCESS {
        debug!("No rel part for item ID {} sts: 0x{:x}", oid, sts);
        return Ok("".to_string());
    }
    let rels_data_slice: &[u8] = &data_part[..data_size as usize];
    let res = relpart::get_rel_successors(rels_data_slice, irel.rel_id);
    if let Ok(succs) = res {
        debug!(
            "irel_exec - Item ID {} relid: {} succs: {:?}",
            oid, irel.rel_id, succs
        );
        for succ in succs {
            debug!("Relationship traversal - successor item ID: {}", succ);
            // Set the successor OID value to the target key val idx
            values[irel.tgt_key_val_idx as usize].borrow_mut().data =
                Some(sql_value_pb::Data::OidValue(succ));
            // Process next instruction if any
            next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
        }
    }
    Ok("".to_string())
}

fn irel_update_exec(
    ctxt: &mut impl SqlExeTrait,
    _plan: &SqlPlanPb,
    _offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    irelupd: &IRelUpdPb,
) -> Result<String, SqlExecError> {
    // Get value refs
    let curr_id = get_id_value!(values, irelupd.key_val_idx as usize);
    let upd_type = RelOpPb::try_from(irelupd.upd_type).unwrap();
    for r in &irelupd.ranges {
        debug!("Scan range: {:?}", r);
        let res = index_scan(ctxt, irelupd.tgt_index_root_id, values, &r.ranges, true);
        if let Ok(res_opt) = res {
            match res_opt {
                Some((ids, _)) => {
                    debug!(
                        "Index scan returned OIDs for relationship update: {:?}",
                        ids
                    );
                    for item_id in &ids {
                        let sts = utils::update_rels(
                            ctxt,
                            upd_type,
                            true, // inverse rel
                            irelupd.rel_id,
                            *item_id,
                            &vec![curr_id],
                        );
                        if sts != STS_SUCCESS {
                            println!(
                                "Failed to update inverse rel - item id {} sts: {}",
                                item_id, sts
                            );
                        }
                    }
                    // Update the self id direct relationship with the target ids we got from the PK index
                    let sts = utils::update_rels(
                        ctxt,
                        upd_type,
                        false, // direct rel
                        irelupd.rel_id,
                        curr_id,
                        &ids,
                    );
                    if sts != STS_SUCCESS {
                        println!("Failed to update rel - curr_id {} sts: {}", curr_id, sts);
                    }
                }
                None => {
                    // No matching entry, set null value
                    debug!(
                        "irel_update_exec - no matching entry key_val_idx: {}",
                        irelupd.key_val_idx
                    );
                }
            }
        }
    }
    Ok("".to_string())
}

fn select_stmt_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    let inst_option = plan.insts.get(offset);
    debug!("Execute inst at offset {}: {:?}", offset, inst_option);
    let res = match inst_option {
        Some(inst) => match inst.inst.as_ref() {
            Some(Inst::Dataset(s)) => {
                idataset_exec(ctxt, plan, offset, values, &s, proj_cols, proj_rows)
            }
            Some(Inst::Index(i)) => {
                iindex_exec(ctxt, plan, offset, values, &i, proj_cols, proj_rows)
            }
            Some(Inst::Dpath(p)) => {
                idatapath_exec(ctxt, plan, offset, values, &p, proj_cols, proj_rows)
            }
            Some(Inst::Expr(e)) => iexpr_exec(ctxt, plan, offset, values, &e, proj_cols, proj_rows),
            Some(Inst::Comp(c)) => {
                icompare_exec(ctxt, plan, offset, values, &c, proj_cols, proj_rows)
            }
            Some(Inst::Proj(p)) => iproj_exec(ctxt, plan, offset, values, &p, proj_cols, proj_rows),
            Some(Inst::Rel(r)) => irel_exec(ctxt, plan, offset, values, &r, proj_cols, proj_rows),
            Some(Inst::RelUpdate(u)) => irel_update_exec(ctxt, plan, offset, values, &u),
            Some(Inst::Insert(i)) => {
                insert_into_exec(ctxt, plan, offset, values, &i, proj_cols, proj_rows)
            }
            Some(Inst::Delete(d)) => {
                idelete_exec(ctxt, plan, offset, values, &d, proj_cols, proj_rows)
            }
            Some(Inst::Update(u)) => {
                iupdate_exec(ctxt, plan, offset, values, &u, proj_cols, proj_rows)
            }
            Some(Inst::Yank(y)) => iyank_exec(ctxt, plan, offset, values, &y, proj_cols, proj_rows),
            _ => Ok("".to_string()),
        },
        None => Ok("".to_string()),
    };
    res
}

fn iupdate_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    iupdate: &IUpdatePb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    // Get value refs
    let item_id = get_id_value!(values, iupdate.key_val_idx as usize);
    // Get smaller scope for mutable borrow of res_val
    {
        let mut data_part: [u8; 32000] = [0; 32000];
        let mut data_size: c_uint = data_part.len() as c_uint;
        let mut set_id: c_uint = 0;
        let sts = ctxt.get_datapart(
            ctxt.get_ltime(),
            item_id,
            &mut set_id,
            &mut data_part,
            &mut data_size,
        );
        debug!("get_datapart sts: 0x{:x} size: {}", sts, data_size);
        if sts != STS_SUCCESS {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to get datapart for item ID 0x{:x} sts: 0x{:x}",
                item_id, sts
            )));
        }
        let data_slice: &[u8] = &data_part[..data_size as usize];
        let owned_jsonb = jsonb::OwnedJsonb::new(data_slice.to_vec());
        let raw_jsonb = owned_jsonb.as_raw();
        let mut updated_str = raw_jsonb.to_string();
        // For each update segment, apply the update to the datapart content
        for (i, path) in iupdate.pathsegs.iter().enumerate() {
            let val_idx = iupdate.val_idxs[i] as usize;
            // Split the dot separated path into a vector of segments
            let path_segs: Vec<String> = path.split('.').map(|s| s.to_string()).collect();

            // Drop previous path value if any
            let res = jbparse::drop_from_jsonstr(&updated_str, &path_segs);
            if let Err(e) = res {
                return Err(SqlExecError::ExecutionError(format!(
                    "Failed to drop update path {:?} from datapart for item ID 0x{:x}: {}",
                    path, item_id, e
                )));
            };
            updated_str = res.unwrap();

            // Insert value at path
            let value = &values[val_idx].borrow();
            let res = jbparse::insert_into_jsonstr(&updated_str, &path_segs, value);
            if let Err(e) = res {
                return Err(SqlExecError::ExecutionError(format!(
                    "Failed to insert update path {:?} to datapart for item ID 0x{:x}: {}",
                    path, item_id, e
                )));
            };
            updated_str = res.unwrap();
        }
        // Update the datapart with the updated content
        let updated_data_binary = jbparse::jsonstr_to_jsonb(&updated_str).unwrap();
        let sts = ctxt.update_item(
            ctxt.get_tranid(),
            item_id,
            &updated_data_binary,
            updated_data_binary.len() as c_uint,
        );
        if sts != STS_SUCCESS {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to update datapart for item ID 0x{:x}",
                item_id,
            )));
        }
        // TBD - need to block schema updates
    }

    // Index keys are removed by next IIndex instructions
    next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);

    ctxt.increment_count(sqlexet::UpdCounter::Update(1));
    Ok("".to_string())
}

fn iyank_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    iyank: &IYankPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    // Get value refs
    let item_id = get_id_value!(values, iyank.key_val_idx as usize);
    // Get smaller scope for mutable borrow of res_val
    {
        let mut data_part: [u8; 32000] = [0; 32000];
        let mut data_size: c_uint = data_part.len() as c_uint;
        let mut set_id: c_uint = 0;
        let sts = ctxt.get_datapart(
            ctxt.get_ltime(),
            item_id,
            &mut set_id,
            &mut data_part,
            &mut data_size,
        );
        debug!("get_datapart sts: 0x{:x} size: {}", sts, data_size);
        if sts != STS_SUCCESS {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to get datapart for item ID 0x{:x} sts: 0x{:x}",
                item_id, sts
            )));
        }
        let data_slice: &[u8] = &data_part[..data_size as usize];
        let owned_jsonb = jsonb::OwnedJsonb::new(data_slice.to_vec());
        let raw_jsonb = owned_jsonb.as_raw();
        let mut updated_str = raw_jsonb.to_string();
        // For each update segment, apply the update to the datapart content
        for path in &iyank.pathsegs {
            // Split the dot separated path into a vector of segments
            let path_segs: Vec<String> = path.split('.').map(|s| s.to_string()).collect();

            // Yank path content if any
            let res = jbparse::drop_from_jsonstr(&updated_str, &path_segs);
            if let Err(e) = res {
                return Err(SqlExecError::ExecutionError(format!(
                    "Failed to delete update path {:?} from datapart for item ID 0x{:x}: {}",
                    path, item_id, e
                )));
            };
            updated_str = res.unwrap();
        }
        // Update the datapart with the updated content
        let updated_data_binary = jbparse::jsonstr_to_jsonb(&updated_str).unwrap();
        let sts = ctxt.update_item(
            ctxt.get_tranid(),
            item_id,
            &updated_data_binary,
            updated_data_binary.len() as c_uint,
        );
        if sts != STS_SUCCESS {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to update datapart for item ID 0x{:x}",
                item_id,
            )));
        }
        // TBD - need to block schema updates
    }

    // Index keys are removed by next IIndex instructions
    next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);

    ctxt.increment_count(sqlexet::UpdCounter::Update(1));
    Ok("".to_string())
}

fn idelete_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    idelete: &IDeletePb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Delete instruction execution: {:?}", idelete);
    let item_id = get_id_value!(values, idelete.key_val_idx as usize);
    debug!("Deleting item ID {}", item_id);

    // Delete inv rels from target dataset items
    let res = utils::delete_inv_rel_predecessors(ctxt, item_id);
    if let Err(e) = res {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to delete inverse rel predecessors for item ID {}: {}",
            item_id, e
        )));
    }

    // Index keys are removed by next IIndex instructions
    next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);

    // Delete the item datapart
    let sts = ctxt.delete_item(ctxt.get_tranid(), IdSpc::DataPart, item_id);
    if sts == STS_SUCCESS {
        ctxt.increment_count(sqlexet::UpdCounter::Delete(1));
    } else {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to delete item ID {}, sts=0x{:x}",
            item_id, sts
        )));
    }
    // TBD - remove relpart
    Ok("".to_string())
}

fn iexpr_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    iexpr: &sqlinsts::IExprPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Expression evaluation: {:?}", iexpr);

    let left_val = values[iexpr.lval_idx as usize].borrow();
    let right_val = values[iexpr.rval_idx as usize].borrow();
    let res_val = utils::evaluate_expr(
        &left_val,
        &right_val,
        sqlinsts::OperPb::try_from(iexpr.op).unwrap(),
    );
    debug!("Expression result value: {:?}", res_val);
    // Set the result value to the target val idx
    values[iexpr.resval_idx as usize].borrow_mut().data = Some(res_val);
    // Process next instruction if any
    next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
    Ok("".to_string())
}

fn icompare_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    icomp: &IComparePb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Compare: {:?}", icomp);

    let mut match_count = 0;
    let comp_op = CompOperatorPb::try_from(icomp.comp).unwrap();
    // Loop over IN(x,y) values if any, otherwise once by default
    for val_offset in 0..icomp.right_val_cnt {
        let matching_comp;
        // Reduce val borrow scope
        {
            let left_val = values[icomp.left_val_idx as usize].borrow();
            let right_val = values[(icomp.right_val_idx + val_offset) as usize].borrow();
            debug!("Left value: {:?}", left_val);
            debug!("Right value: {:?}", right_val);
            matching_comp = utils::compare_values(
                &left_val,
                &right_val,
                comp_op,
            );
        }
        if matching_comp {
            match_count += 1;
            // For NOT IN(x, y) we need all values to be matching the NE test
            if comp_op == CompOperatorPb::NotIn && match_count < icomp.right_val_cnt {
                debug!(
                    "Comparison result: false (NOT IN, match count {} < right val count {})",
                    match_count, icomp.right_val_cnt
                );
                continue;
            }
            debug!("Comparison result: true");
            // Process next instruction if any
            next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
            break;
        } else {
            debug!("Comparison result: false");
        }
    }
    Ok("".to_string())
}

fn idatapath_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    idatapath: &IDatapathPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Datapath: {:?}", idatapath);

    // Get value refs
    let item_id = get_id_value!(values, idatapath.key_val_idx as usize);
    // Get smaller scope for mutable borrow of res_val
    {
        let res = utils::get_sqlval_for_path(ctxt, idatapath.phase, item_id, &idatapath.path_str);
        if let Err(e) = res {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to get value for datpath for item ID 0x{:x}: {}",
                item_id, e
            )));
        };
        let sqlval = res.unwrap();
        // No rels preview if not a SELECT xxx.yyy.*, i.e. last element of pathsegs is a wildcard
        if !(idatapath.jsonpath.len() == 1 && idatapath.jsonpath[0] == "*") {
            // Smaller scope for mutable borrow of res_val
            {
                let res_val = &mut values[idatapath.val_idx as usize].borrow_mut();
                res_val.data = sqlval.data.clone();
            }
            // Done
            next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
            return Ok("".to_string());
        }
        // Get string value of current entity datapart
        let mut res_str = match sqlval.data {
            Some(sql_value_pb::Data::StringValue(ref s)) => s.clone(),
            _ => {
                // Done
                next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
                return Ok("".to_string());
            }
        };
        // Merge rels target pk content with the data part
        let mut rels_data_part: [u8; 32000] = [0; 32000];
        let mut rels_data_size: c_uint = rels_data_part.len() as c_uint;
        let sts = ctxt.get_relpart(
            ctxt.get_ltime(),
            item_id,
            false, // direct rel
            &mut rels_data_part,
            &mut rels_data_size,
        );
        debug!(
            "get_relpart for item ID 0x{:x}, sts: 0x{:x} size: {}",
            item_id, sts, rels_data_size
        );
        if sts == STS_SUCCESS {
            let rels_data_slice: &[u8] = &rels_data_part[..rels_data_size as usize];
            let relpart = relpart::get_rels(&rels_data_slice.to_vec());
            debug!("Item ID 0x{:x} rels part: {:?}", item_id, relpart);
            // fetch the target data parts
            if let Ok(rels) = relpart {
                for (relid, succs) in rels {
                    debug!("Rel ID: 0x{:x}, successors: {:?}", relid, succs);
                    // Find the rel name in the idatapath inst
                    let rel_desc_opt = idatapath
                        .rel_descs
                        .iter()
                        .filter_map(|rel_desc| {
                            if rel_desc.id == relid {
                                return Some(rel_desc);
                            }
                            None
                        })
                        .next();
                    if rel_desc_opt.is_none() {
                        debug!("Rel id 0x{:x} not found for obj 0x{:x}", relid, item_id);
                        continue;
                    }
                    let rel_desc = rel_desc_opt.unwrap();
                    let new_res_str = utils::populate_rel_pkvals_for_succs(
                        ctxt,
                        &rel_desc.name,
                        &succs,
                        &rel_desc.pk_segs,
                        &res_str,
                    );
                    res_str = new_res_str;
                }
            }
        } else {
            debug!("Failed to get rels part for item ID 0x{:x}", item_id);
        }
        // Smaller scope for mutable borrow of res_val
        let res_val = &mut values[idatapath.val_idx as usize].borrow_mut();
        res_val.data = Some(sql_value_pb::Data::StringValue(res_str));
        debug!("Result value at index {}: {:?}", idatapath.val_idx, res_val);
    }
    // Process next instruction if any
    next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);
    return Ok("".to_string());
}

fn iproj_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    iproj: &IProjPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Projection: {:?}", iproj);
    let res_val = values[iproj.val_idx as usize].borrow();
    let str_val = match &res_val.data {
        Some(sql_value_pb::Data::StringValue(s)) => s,
        Some(sql_value_pb::Data::Int64Value(i)) => &i.to_string(),
        Some(sql_value_pb::Data::DecimalValue(d)) => {
            // Convert decimal bytes to string
            let dec = rust_decimal::Decimal::new(d.number as i64, d.scale as u32);
            &dec.to_string()
        }
        _ => "null",
    };
    debug!(
        "Projected value - val_idx {} content: {}",
        iproj.val_idx, str_val
    );
    // Clear the trailing columns as they are not yet populated for this row
    for i in iproj.col_num as usize..proj_cols.len() {
        proj_cols[i] = "null".to_string();
    }

    // Fill our column
    proj_cols[iproj.col_num as usize] = str_val.to_string();
    drop(res_val);

    // Process next instruction if any
    let row_count = proj_rows.len();
    next_select_stmt_exec!(ctxt, plan, offset + 1, values, proj_cols, proj_rows);

    debug!(
        "After proj at offset: {} row_cnt: {}.{} col_num: {}, proj_cols: {:?}",
        offset,
        iproj.col_num,
        row_count,
        proj_rows.len(),
        proj_cols
    );
    // If row count has not moved, push the current projection as a new row
    if proj_rows.len() == row_count {
        let proj_row = proj_cols.join(", ");
        proj_rows.push(proj_row);
    }
    return Ok("".to_string());
}

fn dataset_process_ids(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    idataset_inst: &sqlinsts::SqlInstPb,
    id_vec: &[u32],
    num_ids: c_uint,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Dataset Scan: {:?}", idataset_inst);
    let key_val_idx = match &idataset_inst.inst {
        Some(Inst::Dataset(s)) => s.key_val_idx as usize,
        Some(Inst::Index(i)) => i.key_val_idx as usize,
        _ => {
            return Err(SqlExecError::ExecutionError(
                "Invalid instruction for dataset_process_ids".to_string(),
            ))
        }
    };
    // For each id, process next instruction if any
    for i in 0..num_ids as usize {
        let item_id = id_vec[i];
        debug!("process item ID: 0x{:x}", item_id);

        if item_id < MIN_USER_DATASET_ID {
            continue;
        }
        // Reduce scope for borrow_mut to unborrow
        {
            // Set item_id value at key_val_idx
            let mut val = values[key_val_idx as usize].borrow_mut();
            val.data = Some(sql_value_pb::Data::OidValue(item_id));
            debug!("Set value at index {}: {:?}", key_val_idx, val);
        }
        next_select_stmt_exec!(ctxt, plan, offset, values, proj_cols, proj_rows);
    }
    Ok("".to_string())
}

fn idataset_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    idataset: &IDatasetPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    debug!("Dataset: {:?}", idataset);

    // Get offset for next instruction
    let next_offset = offset + 1;

    let key_val_idx = idataset.key_val_idx as usize;
    debug!("Key value index: {}", key_val_idx);

    let mut set_stream: c_uint = 0 as c_uint;
    let mut id_vec: Vec<u32> = vec![0; 10];
    let mut num_ids: c_uint = id_vec.len() as c_uint;
    let mut next_ids = false;
    let mut ret = Ok("".to_string());
    let idataset_inst = sqlinsts::SqlInstPb {
        inst: Some(Inst::Dataset(idataset.clone())),
    };
    let sts = loop {
        let sts;
        if next_ids {
            sts = ctxt.dataset_enum_many(set_stream, id_vec.as_mut_ptr(), &mut num_ids);
            if sts == STS_SUCCESS || sts == MTS_ENDOFSTREAM {
                match dataset_process_ids(
                    ctxt,
                    plan,
                    next_offset,
                    values,
                    &idataset_inst,
                    &id_vec,
                    num_ids,
                    proj_cols,
                    proj_rows,
                ) {
                    Ok(s) => {
                        ret = Ok(s);
                        if sts == MTS_ENDOFSTREAM {
                            break sts;
                        }
                        continue;
                    }
                    Err(e) => {
                        ret = Err(e);
                        break sts;
                    }
                }
            }
        } else {
            next_ids = true;
            // get ids, activate task
            sts = ctxt.dataset_enum_start(
                ctxt.get_ltime(),
                idataset.dataset_id,
                &mut set_stream,
                id_vec.as_mut_ptr(),
                &mut num_ids,
            );
            if sts == STS_SUCCESS || sts == MTS_ENDOFSTREAM {
                match dataset_process_ids(
                    ctxt,
                    plan,
                    next_offset,
                    values,
                    &idataset_inst,
                    &id_vec,
                    num_ids,
                    proj_cols,
                    proj_rows,
                ) {
                    Ok(s) => {
                        ret = Ok(s);
                        if sts == MTS_ENDOFSTREAM {
                            break sts;
                        }
                        continue;
                    }
                    Err(e) => {
                        ret = Err(e);
                        break sts;
                    }
                }
            };
        }
        if sts == MTS_ENDOFSTREAM || sts != STS_SUCCESS {
            break sts;
        }
    };
    if ret.is_ok() && sts != STS_SUCCESS && sts != MTS_ENDOFSTREAM {
        ret = Err(SqlExecError::ExecutionError(format!(
            "Failed to start dataset enumeration for dataset '{}'",
            idataset.name
        )));
    }

    // Free stream if set
    let end_stream_sts = ctxt.dataset_enum_end(set_stream);
    if end_stream_sts != STS_SUCCESS && ret.is_ok() {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to end dataset enumeration for dataset '{}'",
            idataset.name
        )));
    }
    return ret;
}

fn index_scan(
    ctxt: &mut impl SqlExeTrait,
    root_id: MtOidT,
    values: &Vec<RefCell<SqlValuePb>>,
    ranges: &Vec<sqlinsts::RangePb>,
    unique: bool,
) -> Result<Option<(Vec<u32>, indexsrch::ScanOutput)>, String> {
    let mut fetched_ids: Vec<u32> = vec![];
    // In predicate number of values if applicable, otherwise 1
    let in_pred_nb_vals = if ranges.is_empty() {
        1
    } else {
        ranges[0].lower_bound_nb_vals
    };
    for in_val_offset in 0..in_pred_nb_vals {
        let mut start_keys = vec![];
        let mut start_cmps = vec![];
        let mut end_keys = vec![];
        let mut end_cmps = vec![];
        for r in ranges {
            if r.lower_bound_val_idx < 0 {
                continue;
            }
            start_keys.push(values[(r.lower_bound_val_idx + in_val_offset) as usize].clone());
            start_cmps.push(CompOperatorPb::try_from(r.lower_op).unwrap());
            if r.upper_bound_val_idx < 0 {
                continue;
            }
            end_keys.push(values[r.upper_bound_val_idx as usize].clone());
            end_cmps.push(CompOperatorPb::try_from(r.upper_op).unwrap());
        }
        debug!("Index scan start keys: {:?}", start_keys);
        debug!("Index scan start cmps: {:?}", start_cmps);
        debug!("Index scan end keys: {:?}", end_keys);
        debug!("Index scan end cmps: {:?}", end_cmps);
        let result = indexbt::bt_index_scan(
            ctxt,
            root_id,
            &start_keys,
            &start_cmps,
            &end_keys,
            &end_cmps,
            unique,
        )
        .map_err(|e| format!("Failed to execute index scan: {}", e))?;
        if result.is_none() {
            break;
        }
        let (ids, scan_output) = result.unwrap();
        fetched_ids.extend(ids);
        debug!(
            "Index scan fetched ids: {:?}, scan_output: {:?}",
            fetched_ids, scan_output
        );
        // Scan returns 'more' if reached end of index page, if comparison is eq or in, there is
        // no need to fetch more, continue to next value if any
        if let indexsrch::ScanOutput::More = scan_output {
            if !ranges.is_empty()
                && (ranges[0].lower_op == CompOperatorPb::Eq as i32
                    || ranges[0].lower_op == CompOperatorPb::In as i32)
            {
                debug!("Index scan returned more results but comparison is EQ or IN, no need to fetch more");
                continue;
            }
            debug!("Index scan returned more results than buffer size, need to fetch more");
            return Ok(Some((fetched_ids, scan_output)));
        }
    }
    if fetched_ids.is_empty() {
        debug!("Index scan returned no results");
        return Ok(None);
    }
    Ok(Some((fetched_ids, indexsrch::ScanOutput::Done)))
}

fn iindex_exec(
    ctxt: &mut impl SqlExeTrait,
    plan: &SqlPlanPb,
    offset: usize,
    values: &Vec<RefCell<SqlValuePb>>,
    iindex: &IIndexPb,
    proj_cols: &mut Vec<String>,
    proj_rows: &mut Vec<String>,
) -> Result<String, SqlExecError> {
    // Get offset for next instruction
    let next_offset = offset + 1;
    let indext_type = IndexTypePb::try_from(iindex.idx_type).unwrap();
    let unique = indext_type != IndexTypePb::Default;

    if iindex.op == IndexOpPb::InsertKey as i32 || iindex.op == IndexOpPb::DeleteKey as i32 {
        debug!("Executing index insert/upsert instruction: {:?}", iindex);
        let curr_id = get_id_value!(values, iindex.key_val_idx as usize);
        let mut sqlvals = vec![];
        for seg_str in &iindex.seg_strs {
            // Fetch value for the segment path
            let res = utils::get_sqlval_for_path(ctxt, 0, curr_id, seg_str);
            if let Err(e) = res {
                return Err(SqlExecError::ExecutionError(format!(
                    "Failed to get value for index segment path '{}' for item ID 0x{:x}: {}",
                    seg_str, curr_id, e
                )));
            };
            sqlvals.push(RefCell::new(res.unwrap()));
        }
        let res = if iindex.op == IndexOpPb::InsertKey as i32 {
            indexbt::bt_insert_key(ctxt, iindex.root_id, curr_id, &sqlvals, unique)
        } else {
            indexbt::bt_delete_key(ctxt, iindex.root_id, curr_id, &sqlvals, unique)
        };
        if let Err(e) = res {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to execute index insert/upsert for item ID 0x{:x}: {}",
                curr_id, e
            )));
        }
        next_select_stmt_exec!(ctxt, plan, next_offset, values, proj_cols, proj_rows);
        return Ok("".to_string());
    } else {
        // Index scan
        let mut ret = Ok("".to_string());

        let ranges = match &iindex.range {
            Some(r) => &r.ranges,
            None => &vec![],
        };
        let iindex_inst = sqlinsts::SqlInstPb {
            inst: Some(Inst::Index(iindex.clone())),
        };

        let scan_res = index_scan(ctxt, iindex.root_id, values, &ranges, unique);
        if let Ok(res_opt) = scan_res {
            debug!("Index scan result {:?}", res_opt);
            match res_opt {
                Some(res) => {
                    let ids = res.0;
                    debug!("Index scan returned OIDs: {:?}", ids);
                    let num_ids = ids.len() as c_uint;
                    match dataset_process_ids(
                        ctxt,
                        plan,
                        next_offset,
                        values,
                        &iindex_inst,
                        &ids,
                        num_ids,
                        proj_cols,
                        proj_rows,
                    ) {
                        Ok(s) => {
                            ret = Ok(s);
                            // TBD - handle 'more' results
                        }
                        Err(e) => {
                            ret = Err(e);
                        }
                    }
                }
                None => {
                    // No matching entry, set null value
                    debug!(
                        "No matching entry for index scan, set null value at index {}",
                        iindex.key_val_idx
                    );
                }
            }
        } else {
            return Err(SqlExecError::ExecutionError(format!(
                "Failed to execute index scan for index '{}'",
                iindex.name
            )));
        };
        ret
    }
}
