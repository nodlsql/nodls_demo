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

use sqlexet::SqlExeTrait;
use sqlinsts::{sql_inst_pb, EvalPhasePb, IDatapathPb, IRelPb, SqlInstPb, SqlPlanPb, SqlValuePb};
use std::collections::HashMap;
use std::vec;
use tracing::debug;

pub mod analyze;
pub mod iutils;
pub mod utils;
use utils::{get_invrel_for_offset, pretty_print_plan, DatasetAnalyzer, SqlTranslateError};

// Match the dataset name in from list to get the key_val_idx
macro_rules! ds_key_val_idx {
    ($m: ident, $d: expr) => {
        if let Some(idx) = $m.get($d) {
            *idx
        } else if $m.len() == 1 {
            // If only one ds in from list, pick it up
            *$m.values().next().unwrap()
        } else {
            return Err(SqlTranslateError::AmbiguousDatapath($d.clone()));
        }
    };
}

pub fn optimize_plan(
    ctxt: &mut impl SqlExeTrait,
    sqlplan: &mut SqlPlanPb,
) -> Result<(), SqlTranslateError> {
    let analyzers = analyze::get_dataset_analyzers(ctxt, sqlplan)?;
    // Optimization logic here
    let mut optimized_insts = Vec::new();
    let added_values = vec![];
    debug!("Analyzers: {:#?}", analyzers);
    debug!("Initial SQL Plan: {}", pretty_print_plan(sqlplan));

    // Hashmap of dataset name to key_val_idx
    let mut key_val_idx_map: HashMap<String, i32> = HashMap::new();

    // Simple parent path for dataset, insert, update, delete
    if analyzers.len() == 1 {
        key_val_idx_map.insert("".to_string(), analyzers[0].idataset.key_val_idx);
    }
    for sqlplan_inst in &mut sqlplan.insts {
        let inst = sqlplan_inst.clone();
        match inst.inst.as_ref() {
            Some(sql_inst_pb::Inst::Dataset(d)) => {
                // Get the idataset key_val_idx to set in idatapath
                let ds_name = d.name.clone();
                // Simple parent path for dataset
                key_val_idx_map.insert(ds_name.clone(), d.key_val_idx);
                // Push idataset or iindex if applicable, or both for drop dataset.
                // There is nothing to do for rels, inverse rels should clear with GC.
                // TBD - everything could be cleaned up by GC, just the dataset desc needs to clear.
                iutils::push_idataset(&analyzers, &d, d.key_val_idx, &mut optimized_insts);
            }
            Some(sql_inst_pb::Inst::Dpath(d)) => {
                // Analyzers are empty for statements like 'select 1'
                if analyzers.is_empty() {
                    optimized_insts.push(SqlInstPb {
                        inst: Some(sql_inst_pb::Inst::Dpath(d.clone())),
                    });
                    continue;
                }
                if d.ds_name == "dataset" {
                    let eval_phase = if d.phase == EvalPhasePb::Projection as i32 {
                        EvalPhasePb::StrippedProjection as i32
                    } else {
                        d.phase as i32
                    };
                    let path_str = jbparse::path_to_jbpath(&d.pathsegs.join("."), &d.jsonpath);
                    jbparse::check_json_path(&path_str).map_err(|e| SqlTranslateError::InvalidJsonPath(e))?;
                    optimized_insts.push(SqlInstPb {
                        inst: Some(sql_inst_pb::Inst::Dpath(IDatapathPb {
                            phase: eval_phase,
                            key_val_idx: 0,
                            path_str: path_str,
                            ..d.clone()
                        })),
                    });
                    continue;
                }

                // Get the initial key_val_idx from the parent dataset, in case of multiple datasets in from list
                let mut key_val_idx = ds_key_val_idx!(key_val_idx_map, &d.parent_path[0]);

                // From translate:
                // - parent content has single element dsname
                // - pathsegs contains relsegs followed by jsonpath segs
                // After optimization:
                // - parent content has dsname and rel segs if any
                // - jpath contains only jsonpath segs.

                // This is the actual parent, or target dsname from last rel inst.
                // Not to confuse with the top parent in the parent path.
                // . top parent in path is dsname in: [dsname, rel1, rel2]
                // . actual parent is tgt name in: [dsname, rel1 (tgt name), rel2(tgt name)]
                let mut current_parent = d.parent_path[0].clone();
                let mut top_parent_path = vec![current_parent.clone()];
                let pathsegs = d.pathsegs.clone();
                let mut new_pathsegs = d.pathsegs.clone();
                let mut joffset = 0;

                // Loop on path segments
                for path_seg in &pathsegs {
                    debug!(
                        "IDatapath top parent path: '{:?}' current parent: '{:?}' current seg: '{:?}'",
                        top_parent_path, current_parent, path_seg,
                    );
                    let invrel_parent = get_invrel_for_offset(&d.invrels, joffset);

                    // If path is for rel.xxx predicate eval or projection, insert an IRel inst before IDatapath,
                    // and map the IDatapath inst to the rel generated target key_val_idx.

                    // 1 - Check if there is already a rel inst for this rel at this level in the optimized insts
                    let rel_inst_match = get_matched_rel_inst_for_relpath(
                        &top_parent_path,
                        &invrel_parent,
                        path_seg,
                        &optimized_insts,
                    );
                    if let Some(rinst) = rel_inst_match {
                        debug!(
                        "Found existing IRel inst for rel parent '{:?}' pathsegs '{:?}', reusing it", 
                           d.parent_path, d.pathsegs);
                        // Update parent path and jpath for next iteration
                        top_parent_path.push(path_seg.clone());
                        current_parent = rinst.tgt_ds_name.clone();
                        new_pathsegs = new_pathsegs[1..].to_vec();
                        key_val_idx = rinst.tgt_key_val_idx;
                        joffset += 1;
                    }
                    // 2 - If not try to get one from analyzers
                    else if let Some(rinst) = build_rel_inst(
                        &current_parent,
                        &path_seg,
                        &top_parent_path,
                        &invrel_parent,
                        key_val_idx,
                        // Candidate next key_val_idx if we built a rel inst
                        sqlplan.max_value_idx,
                        &analyzers,
                    ) {
                        debug!(
                            "Build IRel inst for current parent '{}' top parent path '{:?}' new_jpath '{:?}' tgt_key_val_idx '{}'",
                             current_parent, top_parent_path, new_pathsegs, rinst.tgt_key_val_idx
                        );
                        // Adopt the existing rel target idx for the next inst key val idx
                        sqlplan.values.push(SqlValuePb {
                            is_constant: false,
                            data: None,
                        });
                        top_parent_path.push(path_seg.clone());
                        new_pathsegs = new_pathsegs[1..].to_vec();
                        key_val_idx = sqlplan.max_value_idx;
                        sqlplan.max_value_idx += 1;
                        // Got one rel, insert it, update the parent ds and top parent path for next iteration
                        current_parent = rinst.tgt_ds_name.clone();
                        // Add rel inst before datapath inst
                        optimized_insts.push(SqlInstPb {
                            inst: Some(sql_inst_pb::Inst::Rel(IRelPb { ..rinst })),
                        });
                        joffset += 1;
                    } else {
                        break;
                    }
                }
                // 4 - No more rel found for this path segment, fix the path and generate the datapath inst
                let path_str = jbparse::path_to_jbpath(&new_pathsegs.join("."), &d.jsonpath);
                jbparse::check_json_path(&path_str).map_err(|e| SqlTranslateError::InvalidJsonPath(e))?;
                iutils::push_idatapath(
                    &analyzers,
                    &d,
                    &new_pathsegs,
                    &path_str,
                    &current_parent,
                    &top_parent_path,
                    key_val_idx,
                    &mut optimized_insts,
                );
            }
            Some(sql_inst_pb::Inst::Update(u)) => {
                // Only one dataset expected
                let key_val_idx = ds_key_val_idx!(key_val_idx_map, &"".to_string());
                iutils::push_iupdate(&analyzers, &u, key_val_idx, &mut optimized_insts);
            }
            Some(sql_inst_pb::Inst::Yank(y)) => {
                // Only one dataset expected
                let key_val_idx = ds_key_val_idx!(key_val_idx_map, &"".to_string());
                iutils::push_iyank(&analyzers, &y, key_val_idx, &mut optimized_insts);
            }
            Some(sql_inst_pb::Inst::RelUpdate(r)) => {
                // Only one dataset expected
                let key_val_idx = ds_key_val_idx!(key_val_idx_map, &"".to_string());
                iutils::push_irelupd(&analyzers[0], &r, key_val_idx, &mut optimized_insts);
            }
            Some(sql_inst_pb::Inst::Insert(i)) => {
                // Only one dataset expected
                let key_val_idx = ds_key_val_idx!(key_val_idx_map, &"".to_string());
                iutils::push_iinsert(&analyzers[0], &i, key_val_idx, &mut optimized_insts);
            }
            Some(sql_inst_pb::Inst::Delete(d)) => {
                // Only one dataset expected
                let key_val_idx = ds_key_val_idx!(key_val_idx_map, &"".to_string());
                iutils::push_idelete(&analyzers[0], &d, key_val_idx, &mut optimized_insts);
            }
            Some(sql_inst_pb::Inst::DdlUpdate(d)) => {
                // Only one dataset expected
                iutils::push_ddlupdate(&analyzers, &d, &mut optimized_insts);
            }
            _ => {
                optimized_insts.push(inst.clone());
            }
        }
    }
    sqlplan.insts = optimized_insts;
    sqlplan.values.extend(added_values);
    Ok(())
}

// If we are here there is no rel inst for the rel path.
// 1 - No rel inst:        'select rs1.a, rs2.b' while evaluating rs2.b
// 2 - No rel inst:        'select rs1.rs2.a' while evaluating rs2.a. Here rs1 inst is already generated
// 3 - Already a rel inst: 'select rs1.a, rs1.b' while evaluating rs1.b
pub fn build_rel_inst(
    ds_name: &String, // actual parent from last rel target ds
    rel_name: &String,
    parent_path: &Vec<String>,
    inverse_parent: &Option<String>,
    key_val_idx: i32,
    tgt_key_val_idx: i32,
    analyzers: &Vec<DatasetAnalyzer>,
) -> Option<IRelPb> {
    debug!(
        "Build rel for ds '{}' rel '{}' parent path '{:?}' inverse '{:?}'",
        ds_name, rel_name, parent_path, inverse_parent,
    );
    let rel_parent = if let Some(inv_parent) = inverse_parent {
        inv_parent
    } else {
        ds_name
    };
    // Start with top parent ds and follow rel target ds names to get the actual analyzer
    for ds_analyzer in analyzers {
        if &ds_analyzer.idataset.name != rel_parent {
            continue;
        }
        for rel_analyzer in &ds_analyzer.rel_analyzers {
            if rel_analyzer.rel_name == *rel_name {
                // If inverse rel the tgt ds name should be the rel parent.
                // inverse(myds.rs) target points to itself, 'myds'
                let tgt_ds = if inverse_parent.is_some() {
                    rel_parent.clone()
                } else {
                    rel_analyzer.tgt_ds_name.clone()
                };
                return Some(IRelPb {
                    name: rel_name.clone(),
                    ds_name: rel_parent.clone(),
                    parent_path: parent_path.clone(),
                    rel_id: rel_analyzer.rel_id,
                    inverse: inverse_parent.is_some(),
                    tgt_ds_name: tgt_ds,
                    tgt_ds_id: rel_analyzer.tgt_ds_id,
                    key_val_idx: key_val_idx,
                    tgt_key_val_idx: tgt_key_val_idx,
                });
            }
        }
    }
    debug!(
        "No rel found for ds '{}' rel '{}' parent path '{:?}' inverse '{:?}'",
        ds_name, rel_name, parent_path, inverse_parent,
    );
    None
}

pub fn get_matched_rel_inst_for_relpath(
    parent_path: &Vec<String>,
    inverse_tgt: &Option<String>,
    rel_name: &String,
    insts: &Vec<SqlInstPb>,
) -> Option<IRelPb> {
    for inst in insts {
        if let Some(sql_inst_pb::Inst::Rel(rel_inst)) = &inst.inst {
            debug!(
                "Check rel inst match: parent path '{:?}' rel inst parent path '{:?}' rel name '{:?}' inverse_tgt '{:?}'",
                parent_path, rel_inst.parent_path, rel_inst.name, &inverse_tgt
            );
            // Coming in with parent path 'ds.rel1', and rel name 'rel2' for direct rel 'ds.rel1.rel2'.
            // Looking for rel with parent path 'ds.rel1' and rel name 'rel2'.
            if rel_inst.name == *rel_name && &rel_inst.parent_path == parent_path {
                debug!(
                "Rel inst matching: parent path '{:?}' rel inst parent path '{:?}' rel name '{:?}' inverse_tgt '{:?}'",
                parent_path, rel_inst.parent_path, rel_inst.name, &inverse_tgt);
                if let Some(inverse_tgt_ds) = inverse_tgt.clone() {
                    debug!(
                        "Check inverse match: tgt ds name '{}' rel inst tgt ds name '{}'",
                        inverse_tgt_ds, rel_inst.tgt_ds_name
                    );
                    if !rel_inst.inverse || rel_inst.tgt_ds_name != inverse_tgt_ds {
                        continue;
                    }
                }
                return Some(rel_inst.clone());
            }
        }
    }
    None
}

// TBD - move it back to sqlplan since we can't use it here because of
// mut borrow of sqlplan. Best would be to make it a macro and use it in both places.
pub fn add_value(
    sqlplan: &mut sqlinsts::SqlPlanPb,
    is_constant: bool,
    data: Option<sqlinsts::sql_value_pb::Data>,
) -> i32 {
    let val_idx = sqlplan.max_value_idx;
    sqlplan.values.push(SqlValuePb {
        is_constant: is_constant,
        data: data,
    });
    sqlplan.max_value_idx += 1;
    val_idx
}
