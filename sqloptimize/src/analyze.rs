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

use jbparse::DatasetDesc;
use sqlexet::SqlExeTrait;
use sqlinsts::{
    sql_inst_pb::Inst, CompositeRangePb, DdlOpPb, EvalPhasePb, IComparePb, IDatapathPb, IIndexPb,
    RangePb, SqlPlanPb, SqlStmtPb, SqlValuePb,
};
use std::vec;
use tracing::debug;

use crate::utils::{
    build_ds_desc_analyzer, build_rel_details_for_reldesc, compute_index_range_for_datapath,
    get_index_candidates_for_dataset, get_invrel_for_offset, DatapathAnalyzer, DatasetAnalyzer,
    IndexAnalyzer, RelAnalyzer, SqlTranslateError, MIN_USER_DATASET_ID,
};

// Expected output:
// [
//  DatasetAnalyzer {
//    idataset: IDatasetPb {
//      path: ["hiidx"], key_val_idx: 0, dataset_key: 4261,
//    },
//    index_analyzers: [
//      IndexAnalyzer {
//        index: IIndexPb {
//          idx_type: Pkey, name: "hiidx", segments: ["abc"], key_val_idx: 0, schema_index_key: 0, root_id: 0, range: [],
//        },
//        dpth_analyzers: [
//          DatapathAnalyzer {
//            datapath: IDatapathPb {
//              phase: Predicate, path: ["abc"], path_str: "abc", key_val_idx: 0, val_idx: 1,
//            },
//            comparisons: [
//              IComparePb {
//                comp: Equal, left_val_idx: 1, right_val_idx: 2,
//              },
//    rel_analyzers: [
//      RelAnalyzer {
//        rel_name: "rel1", rel_id: 123, tgt_dataset_name: .., pk_segs (for insert), index_root_id (for insert)...
//    path_str_analyzers: [
//      PathStrAnalyzer {
//        path_str: "rel1.rel2.a.b.c", parent: [dsname, rel1, rel2], jsonpath [a, b, c]
//...
pub fn get_dataset_analyzers(
    ctxt: &impl SqlExeTrait,
    sqlplan: &SqlPlanPb,
) -> Result<Vec<DatasetAnalyzer>, SqlTranslateError> {
    let mut dataset_analyzers: Vec<DatasetAnalyzer> = Vec::new();
    let mut create_ds = false;
    for inst in &sqlplan.insts {
        match inst.inst.as_ref() {
            // If inverse or nested rels document the target dataset if different from the main dataset
            Some(Inst::Dpath(dpath)) => {
                debug!("Check DPath: {:?}", dpath);
                if dpath.ds_name == "dataset" {
                    continue;
                }
                // Path content:
                //   parent: [dsname]
                //   path: [rel1, rel2, a, b..]
                // We evaluate path segments as follows:
                //   1. parent: [dsname]
                //      path: [rel1]        - Create tgt ds analyzer if applicable
                //   2. parent: [dsname]
                //      path: [rel1, rel2]  - if rel1 found, navigate to tgt ds analyzer,
                //                            create tgt2 ds analyzer if applicable

                let mut parent_ds_name = dpath.parent_path[0].clone();
                // Get last rels to build summary if path is terminated with '*'
                let star_path = if dpath.jsonpath.len() == 1 && dpath.jsonpath[0] == "*" {
                    1
                } else {
                    0
                };
                for i in 0..dpath.pathsegs.len() + star_path {
                    let eval_rel_seg = if i < dpath.pathsegs.len() {
                        &dpath.pathsegs[i]
                    } else {
                        "*"
                    };
                    debug!(
                        "Check DPath segment {}: parent ds '{}', path seg '{}'",
                        i, parent_ds_name, eval_rel_seg
                    );
                    // Check if we have an inv rel at that offset
                    if let Some(inv_rel_ds) = get_invrel_for_offset(&dpath.invrels, i) {
                        debug!(
                            "Found inverse rel for segment '{:?}' offset:{} parent ds '{}'",
                            inv_rel_ds, i, parent_ds_name
                        );
                        parent_ds_name = inv_rel_ds;
                    }
                    let res = build_dataset_analyzer(
                        ctxt,
                        sqlplan,
                        &parent_ds_name,
                        -1, // Set by optimize pass for rel/inverse
                        &dataset_analyzers,
                    );
                    if let Ok(tgt_analyzer_opt) = res {
                        // Set the analyzer from result if just created
                        if let Some(tgt_analyzer) = tgt_analyzer_opt {
                            dataset_analyzers.push(tgt_analyzer);
                        }
                        // Get rel analyzer from parent ds analyzer
                        let rel_analyzers = dataset_analyzers
                            .iter()
                            .find(|a| a.dataset_desc.name == parent_ds_name)
                            .map(|a| a.rel_analyzers.clone())
                            .unwrap_or(vec![]);

                        // Check rel and get rel target ds
                        let mut found_rel = false;
                        for rel_analyzer in rel_analyzers {
                            if rel_analyzer.rel_name == *eval_rel_seg {
                                parent_ds_name = rel_analyzer.tgt_ds_name.clone();
                                found_rel = true;
                                break;
                            }
                        }
                        if !found_rel {
                            break;
                        }
                    } else {
                        return Err(SqlTranslateError::DatasetNotFound(parent_ds_name.clone()));
                    }
                }
            }
            Some(Inst::Dataset(d)) => {
                // Note that key_val_idx is valid only for initial datasets in from clause, we fill it in optimizer for rel successor datasets
                let res = build_dataset_analyzer(
                    ctxt,
                    sqlplan,
                    &d.name,
                    d.key_val_idx,
                    &dataset_analyzers,
                );
                if let Ok(analyzer_opt) = res {
                    // If None, we already have an analyzer
                    if let Some(analyzer) = analyzer_opt {
                        debug!("Dataset analyzer for dataset '{}': {:?}", d.name, analyzer);
                        dataset_analyzers.push(analyzer);
                    }
                } else {
                    return Err(SqlTranslateError::DatasetNotFound(d.name.clone()));
                }
            }
            Some(Inst::DdlUpdate(d)) => {
                if d.op == DdlOpPb::CreateDs as i32 {
                    create_ds = true;
                }
                // For alter dataset we also want to have the dataset analyzer on board, as we need to update the dataset details in the alter dataset inst
                let res = build_dataset_analyzer(
                    ctxt,
                    sqlplan,
                    &d.ds_name,
                    -1, // Set by optimize pass for rel/inverse
                    &dataset_analyzers,
                );
                if let Ok(analyzer_opt) = res {
                    if create_ds {
                        return Err(SqlTranslateError::DatasetAlreadyExists(d.ds_name.clone()));
                    }
                    // If None, we already have an analyzer
                    if let Some(analyzer) = analyzer_opt {
                        debug!(
                            "Dataset analyzer for dataset '{}': {:?}",
                            d.ds_name, analyzer
                        );
                        dataset_analyzers.push(analyzer);
                    }
                } else {
                    if !create_ds {
                        return Err(SqlTranslateError::DatasetNotFound(d.ds_name.clone()));
                    }
                }
            }
            Some(Inst::Insert(i)) => {
                // Note that key_val_idx is valid only for initial datasets in from clause, we fill it in optimizer for rel successor datasets
                let res = build_dataset_analyzer(
                    ctxt,
                    sqlplan,
                    &i.ds_name,
                    i.key_val_idx,
                    &dataset_analyzers,
                );
                if let Ok(analyzer_opt) = res {
                    // If None, we already have an analyzer
                    if let Some(analyzer) = analyzer_opt {
                        debug!(
                            "Dataset analyzer for dataset '{}': {:?}",
                            i.ds_name, analyzer
                        );
                        dataset_analyzers.push(analyzer);
                    }
                } else {
                    return Err(SqlTranslateError::DatasetNotFound(i.ds_name.clone()));
                }
            }
            _ => continue,
        }
    }
    Ok(dataset_analyzers)
}

// Build dataset analyzer if not already have one, return None if already there
fn build_dataset_analyzer(
    ctxt: &impl SqlExeTrait,
    sqlplan: &SqlPlanPb,
    ds_name: &String,
    dataset_key_val_idx: i32, // initial value for 'from list', to be updated for rel successor datasets
    dataset_analyzers: &Vec<DatasetAnalyzer>,
) -> Result<Option<DatasetAnalyzer>, SqlTranslateError> {
    debug!("Build dataset analyzer for dataset '{}'", ds_name);
    let res = build_ds_desc_analyzer(ctxt, ds_name, dataset_key_val_idx, dataset_analyzers);
    if let Ok(analyzer_opt) = res {
        if let Some(analyzer) = analyzer_opt {
            // Add index analyzers and rel analyzers for the dataset analyzer
            if analyzer.dataset_desc._id < MIN_USER_DATASET_ID {
                return Ok(Some(analyzer));
            };
            let rel_analyzers = build_rel_details_for_dataset(ctxt, &analyzer.dataset_desc);
            let index_analyzers =
                get_index_analyzers_for_dataset(sqlplan, &analyzer.dataset_desc, ds_name);
            let new_analyzer = DatasetAnalyzer {
                index_analyzers: index_analyzers,
                rel_analyzers: rel_analyzers,
                ..analyzer
            };
            return Ok(Some(new_analyzer));
        } else {
            // Analyzer already exists, skip
            return Ok(None);
        }
    }
    Err(SqlTranslateError::DatasetNotFound(ds_name.clone()))
}

fn build_rel_details_for_dataset(
    ctxt: &impl SqlExeTrait,
    dataset_desc: &DatasetDesc,
) -> Vec<RelAnalyzer> {
    let mut rel_analyzers = Vec::new();
    for rel_desc in &dataset_desc.rels {
        let rel_analyzer_opt = build_rel_details_for_reldesc(ctxt, rel_desc);
        if let Some(rel_analyzer) = rel_analyzer_opt {
            rel_analyzers.push(rel_analyzer);
        }
    }
    rel_analyzers
}

pub fn get_index_analyzers_for_dataset(
    sqlplan: &SqlPlanPb,
    dataset_desc: &DatasetDesc,
    ds_name: &String,
) -> Vec<IndexAnalyzer> {
    let iindex_candidates = get_index_candidates_for_dataset(&dataset_desc);

    let mut idx_analyzers = Vec::new();
    for index_inst in iindex_candidates {
        // For delete statement, we need all indexes on board
        if sqlplan.sqlstmt == SqlStmtPb::DeleteFrom as i32
            || sqlplan.sqlstmt == SqlStmtPb::Update as i32
            || sqlplan.sqlstmt == SqlStmtPb::InsertInto as i32
            || sqlplan.sqlstmt == SqlStmtPb::AlterDataset as i32
        {
            let index_analyzer = IndexAnalyzer {
                iindex: IIndexPb {
                    key_val_idx: 0, // Single loop, all insts with key_val_idx 0
                    ..index_inst.clone()
                },
                dpth_analyzers: vec![], // No use beyond this point, keep for tracing
            };
            idx_analyzers.push(index_analyzer);
            continue;
        }
        let index_analyzer = get_index_analyzer_for_candidate_index(sqlplan, &index_inst, ds_name);
        if let Some(analyzer) = index_analyzer {
            idx_analyzers.push(analyzer);
        }
    }
    debug!(
        "Index analyzers for dataset '{}': {:?}",
        dataset_desc.name, idx_analyzers
    );
    idx_analyzers
}

pub fn get_index_analyzer_for_candidate_index(
    sqlplan: &SqlPlanPb,
    iindex_inst: &IIndexPb,
    ds_name: &String,
) -> Option<IndexAnalyzer> {
    let mut dpth_analyzers = vec![];
    // loop on sqlplan, match datapath key with dataset key and populate the datapath map
    for inst in &sqlplan.insts {
        if let Some(Inst::Dpath(dpath)) = &inst.inst {
            if &dpath.parent_path[0] != &iindex_inst.ds_name {
                continue;
            }
            if dpath.phase == EvalPhasePb::Predicate as i32 && &dpath.parent_path[0] == ds_name {
                // Get one datapath analyzer per composite key segment, ordered by segment
                for seg_str in &iindex_inst.seg_strs {
                    debug!("get analyzer for index - check dpath key_val_idx={} ds name={} dpath={:?} jpath={:?} ix_seg_str={}",
                        dpath.key_val_idx, ds_name, dpath.pathsegs, dpath.jsonpath, seg_str);
                    // check if single segment path
                    if dpath.pathsegs.len() + dpath.jsonpath.len() != 1 {
                        continue;
                    }
                    let path_str = if !dpath.pathsegs.is_empty() {
                        dpath.pathsegs[0].clone()
                    } else {
                        dpath.jsonpath[0].clone()
                    };
                    // check if datapath path is part of index composite key
                    if &path_str != seg_str {
                        continue;
                    }
                    let comparisons = get_comparisons_for_datapath(sqlplan, dpath);
                    if comparisons.is_empty() {
                        continue;
                    }
                    debug!(
                        "push analyzer for index {} - dpath path={} seg_str={}",
                        iindex_inst.name, path_str, seg_str
                    );
                    dpth_analyzers.push(DatapathAnalyzer {
                        datapath: IDatapathPb { ..dpath.clone() },
                        comparisons,
                    });
                }
            }
        }
    }
    // Datapaths and associated comparisons for all composite key segments
    if dpth_analyzers.is_empty() {
        return None;
    }
    // Get the index ranges per segment, skip if none
    let ranges = compute_index_range_for_index(&sqlplan.values, iindex_inst, &dpth_analyzers);
    if ranges.is_empty() {
        return None;
    }
    Some(IndexAnalyzer {
        iindex: IIndexPb {
            range: Some(CompositeRangePb { ranges }),
            ..iindex_inst.clone()
        },
        dpth_analyzers: dpth_analyzers, // No use beyond this point, keep for tracing
    })
}

pub fn get_comparisons_for_datapath(sqlplan: &SqlPlanPb, dpath: &IDatapathPb) -> Vec<IComparePb> {
    let mut comparisons = Vec::new();
    for inst in &sqlplan.insts {
        if let Some(Inst::Comp(comp)) = &inst.inst {
            if comp.left_val_idx == dpath.val_idx || comp.right_val_idx == dpath.val_idx {
                let const_val_idx = if dpath.val_idx == comp.left_val_idx {
                    comp.right_val_idx
                } else {
                    comp.left_val_idx
                };
                // Make sure the other value is a constant, not an OidValue
                let val = &sqlplan.values[const_val_idx as usize];
                if !val.is_constant {
                    continue;
                }
                comparisons.push(comp.clone());
            }
        }
    }
    comparisons
}

// Compute index range for one index
pub fn compute_index_range_for_index(
    values: &Vec<SqlValuePb>,
    iindex: &IIndexPb,
    dpth_analyzers: &Vec<DatapathAnalyzer>,
) -> Vec<RangePb> {
    let mut ranges = vec![None; iindex.seg_strs.len()];

    // Structure is index -> list of dpath -> list of comparisons.
    // For index segments (a.b, c) the predicate a.b > 1 AND c < 2 would give back
    // the ranges for the segment that matches the datapath path.
    // If the first segment doesn't match any datapath we skip for now.
    let mut first_segment_match = false;
    for dpth_analyzer in dpth_analyzers {
        // check if single segment path
        let idpath = &dpth_analyzer.datapath;
        if idpath.pathsegs.len() + idpath.jsonpath.len() != 1 {
            continue;
        }
        let path_str = if !idpath.pathsegs.is_empty() {
            idpath.pathsegs[0].clone()
        } else {
            idpath.jsonpath[0].clone()
        };
        let mut first_segment = true;
        for (i, seg_str) in iindex.seg_strs.iter().enumerate() {
            // Exactly one datapath per composite key segment
            let range_opt = compute_index_range_for_datapath(
                &seg_str,
                &dpth_analyzer.datapath,
                &path_str,
                &dpth_analyzer.comparisons,
                &values,
            );
            debug!(
                "Computed range for index '{:?}', datapath '{:?}': {:?}",
                iindex, dpth_analyzer.datapath, range_opt
            );
            if let Some(r) = range_opt {
                if first_segment {
                    first_segment_match = true;
                }
                ranges[i] = Some(r);
            }
            first_segment = false;
        }
    }
    if first_segment_match {
        debug!(
            "Computed index ranges for index '{:?}': {:?}",
            iindex, ranges
        );
    } else {
        debug!(
            "No match for first segment of index '{:?}', skipping index optimization",
            iindex
        );
        return vec![];
    }
    ranges.into_iter().filter_map(|r| r).collect()
}
