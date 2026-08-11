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

use crate::utils;
use crate::utils::{DatasetAnalyzer, RelAnalyzer};
use sqlinsts::{
    sql_inst_pb, IDatapathPb, IDatasetPb, IDdlPb, IDeletePb, IIndexPb, IInsertPb, IRelUpdPb,
    IUpdatePb, IYankPb, IndexOpPb, RelDescPb, SqlInstPb,
};
use tracing::debug;

pub fn push_irelupd(
    analyzer: &DatasetAnalyzer,
    rel_inst: &IRelUpdPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    for rel_analyzer in &analyzer.rel_analyzers {
        if rel_analyzer.rel_name == rel_inst.name {
            debug!("Updating irelupdate with dataset and key_val_idx details");
            optimized_insts.push(SqlInstPb {
                inst: Some(sql_inst_pb::Inst::RelUpdate(IRelUpdPb {
                    ds_name: analyzer.idataset.name.clone(),
                    rel_id: rel_analyzer.rel_id,
                    tgt_ds_name: rel_analyzer.tgt_ds_name.clone(),
                    tgt_ds_id: rel_analyzer.tgt_ds_id,
                    tgt_index_root_id: rel_analyzer.index_root_id,
                    key_val_idx: key_val_idx,
                    ..rel_inst.clone()
                })),
            });
            break;
        }
    }
}

// Push idataset or iindex if applicable.
pub fn push_idataset(
    analyzers: &Vec<DatasetAnalyzer>,
    idataset_inst: &IDatasetPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    for analyzer in analyzers {
        if idataset_inst.name == "dataset" {
            optimized_insts.push(SqlInstPb {
                inst: Some(sql_inst_pb::Inst::Dataset(IDatasetPb {
                    dataset_id: analyzer.idataset.dataset_id,
                    key_val_idx: key_val_idx,
                    ..idataset_inst.clone()
                })),
            });
            return;
        }
        if analyzer.idataset.name == idataset_inst.name {
            let mut has_idx = false;
            // TBD - We pick up first come index for now
            if let Some(index_analyzer) = analyzer.index_analyzers.first() {
                has_idx = true;
                optimized_insts.push(SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::Index(IIndexPb {
                        key_val_idx: key_val_idx,
                        ..index_analyzer.iindex.clone()
                    })),
                });
            }
            // For drop dataset we want to drop both the indexes and the dataset
            if !has_idx {
                // Pick up the dataset id from analyzer and udpate the dataset inst
                optimized_insts.push(SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::Dataset(IDatasetPb {
                        dataset_id: analyzer.idataset.dataset_id,
                        key_val_idx: key_val_idx,
                        ..idataset_inst.clone()
                    })),
                });
            }
            break;
        }
    }
}

pub fn push_iindex_for_path_update(
    analyzers: &Vec<DatasetAnalyzer>,
    iupdate_inst: &IUpdatePb,
    op: IndexOpPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // Filter by update jpaths to pick up the index candidates
    let iidx_insts =
        utils::get_index_insts_for_candidate_path(key_val_idx, &analyzers, &iupdate_inst.pathsegs);
    for iidx_inst in iidx_insts {
        optimized_insts.push(SqlInstPb {
            inst: Some(sql_inst_pb::Inst::Index(IIndexPb {
                op: op as i32,
                key_val_idx: key_val_idx,
                ..iidx_inst.clone()
            })),
        });
    }
}

pub fn push_iindex_for_path_yank(
    analyzers: &Vec<DatasetAnalyzer>,
    iyank_inst: &IYankPb,
    op: IndexOpPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // Filter by update jpaths to pick up the index candidates
    let iidx_insts =
        utils::get_index_insts_for_candidate_path(key_val_idx, &analyzers, &iyank_inst.pathsegs);
    for iidx_inst in iidx_insts {
        optimized_insts.push(SqlInstPb {
            inst: Some(sql_inst_pb::Inst::Index(IIndexPb {
                op: op as i32,
                key_val_idx: key_val_idx,
                ..iidx_inst.clone()
            })),
        });
    }
}

pub fn push_iupdate(
    analyzers: &Vec<DatasetAnalyzer>,
    iupdate_inst: &IUpdatePb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // Filter by update jpaths to pick up the index candidates
    // - add index insts before and after update to respectively
    // - delete the old keys and insert the new ones
    push_iindex_for_path_update(
        analyzers,
        iupdate_inst,
        IndexOpPb::DeleteKey,
        key_val_idx,
        optimized_insts,
    );
    optimized_insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Update(IUpdatePb {
            key_val_idx: key_val_idx,
            ..iupdate_inst.clone()
        })),
    });
    push_iindex_for_path_update(
        analyzers,
        iupdate_inst,
        IndexOpPb::InsertKey,
        key_val_idx,
        optimized_insts,
    );
}

pub fn push_iyank(
    analyzers: &Vec<DatasetAnalyzer>,
    iyank_inst: &IYankPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // Filter by update jpaths to pick up the index candidates
    // - add index insts before and after update to respectively
    // - delete the old keys and insert the new ones
    push_iindex_for_path_yank(
        analyzers,
        iyank_inst,
        IndexOpPb::DeleteKey,
        key_val_idx,
        optimized_insts,
    );
    optimized_insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Yank(IYankPb {
            key_val_idx: key_val_idx,
            ..iyank_inst.clone()
        })),
    });
}

// Index for insert/delete
pub fn push_iindex_for_insdel(
    analyzer: &DatasetAnalyzer,
    op: IndexOpPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // Only one dataset expected
    let ds_desc = &analyzer.dataset_desc;
    let iidx_insts = utils::get_index_candidates_for_dataset(ds_desc);

    // Append index insts that fetch the index segs contents from items being deleted and
    // update from there.
    for iidx_inst in iidx_insts {
        optimized_insts.push(SqlInstPb {
            inst: Some(sql_inst_pb::Inst::Index(IIndexPb {
                op: op as i32,
                key_val_idx: key_val_idx,
                ..iidx_inst.clone()
            })),
        });
    } // Filter by update jpaths to pick up the index candidates
}

pub fn push_idelete(
    analyzer: &DatasetAnalyzer,
    idelete_inst: &IDeletePb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // add index inst before update to delete the old keys
    push_iindex_for_insdel(analyzer, IndexOpPb::DeleteKey, key_val_idx, optimized_insts);
    optimized_insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Delete(IDeletePb {
            key_val_idx: key_val_idx,
            ..idelete_inst.clone()
        })),
    });
}

pub fn push_iinsert(
    analyzer: &DatasetAnalyzer,
    iinsert_inst: &IInsertPb,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    optimized_insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Insert(IInsertPb {
            key_val_idx: key_val_idx,
            ds_id: analyzer.idataset.dataset_id,
            ..iinsert_inst.clone()
        })),
    });
    push_iindex_for_insdel(analyzer, IndexOpPb::InsertKey, key_val_idx, optimized_insts);
}

pub fn push_ddlupdate(
    analyzers: &Vec<DatasetAnalyzer>,
    ddlupdate_inst: &IDdlPb,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    if analyzers.is_empty() {
        // no ds analyzer for create ds
        optimized_insts.push(SqlInstPb {
            inst: Some(sql_inst_pb::Inst::DdlUpdate(IDdlPb {
                ..ddlupdate_inst.clone()
            })),
        });
    } else {
        // single ds analyzer expected for alter stmts.
        optimized_insts.push(SqlInstPb {
            inst: Some(sql_inst_pb::Inst::DdlUpdate(IDdlPb {
                ds_id: analyzers[0].idataset.dataset_id,
                ..ddlupdate_inst.clone()
            })),
        });
    }
}

pub fn get_proj_rels(idpath: &IDatapathPb, rel_analyzers: &Vec<RelAnalyzer>) -> IDatapathPb {
    // TBD - Only 'select *' projection for now
    let mut rdescs = vec![];
    if idpath.jsonpath.len() == 1 && idpath.jsonpath[0] == "*" {
        for r in rel_analyzers {
            let rdesc = RelDescPb {
                name: r.rel_name.clone(),
                id: r.rel_id,
                pk_segs: r.pk_segs.clone(),
            };
            rdescs.push(rdesc);
        }
        return IDatapathPb {
            rel_descs: rdescs,
            ..idpath.clone()
        };
    }
    return idpath.clone();
}

pub fn push_idatapath(
    analyzers: &Vec<DatasetAnalyzer>,
    idatapath_inst: &IDatapathPb,
    pathsegs: &Vec<String>,
    path_str: &String, // produced by jbparse
    current_parent: &String,
    top_parent_path: &Vec<String>,
    key_val_idx: i32,
    optimized_insts: &mut Vec<SqlInstPb>,
) -> () {
    // Overwrite IDatapath inst key_val_idx
    let mut opt_idpath = IDatapathPb {
        ds_name: current_parent.clone(),
        key_val_idx: key_val_idx,
        path_str: path_str.clone(),
        pathsegs: pathsegs.clone(),
        parent_path: top_parent_path.clone(),
        ..idatapath_inst.clone()
    };
    debug!(
        "After processing rel segs, parent path: {:?} path: {} jpath: {:?} key_val_idx: {}",
        top_parent_path, path_str, idatapath_inst.jsonpath, key_val_idx
    );
    for analyzer in analyzers {
        if analyzer.idataset.name == opt_idpath.ds_name {
            // If applicable update IDatapath with rel summary for 'select *'
            opt_idpath = get_proj_rels(&opt_idpath, &analyzer.rel_analyzers);
            break;
        }
    }
    optimized_insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Dpath(opt_idpath)),
    });
}
