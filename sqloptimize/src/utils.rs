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

use sqlexet::{MtOidT, MtSchemaType, SqlExeTrait, STS_SUCCESS};

use jbparse::DatasetDesc;
use sqlinsts::sqlinsts::{
    CompOperatorPb, IComparePb, IDatapathPb, IDatasetPb, IIndexPb, IndexOpPb, IndexSegPb,
    IndexTypePb, InvRelEltPb, PathUpdPb, RangePb, RelUpdPb, SqlValuePb, ValRelsUpdPb,
};
use std::ffi::c_uint;
use thiserror::Error;
use tracing::debug;

// TBD - needs more accurate surrogate check
pub const MIN_USER_DATASET_ID: u32 = 4098; // nodls pseudo-dataset 0x1002

#[derive(Error, Debug, PartialEq)]
pub enum SqlAnalyzeError {
    #[error("Dataset {0} already exists")]
    DatasetAlreadyExists(String),
    #[error("Dataset {0} not found")]
    DatasetNotFound(String),
    #[error("Primary key not found {0}")]
    PrimaryKeyNotFound(String),
    #[error("Invalid relationship {0}")]
    InvalidRelationship(String),
    #[error("Inverse relationship not found for datapath {0}")]
    InvRelNotFound(String),
    #[error("Invalid update")]
    InvalidUpdate,
    #[error("Ambiguous datapath {0}")]
    AmbiguousDatapath(String),
    #[error("Invalid jsonpath {0}")]
    InvalidJsonPath(String),
}

#[derive(Debug)]
pub struct DatapathAnalyzer {
    // Datapaths analyzers for index matches with comparisons by val1 index
    // For instance, if the index is on (a, b) and the query has predicates a = 1 and b = 2,
    // there will be two DatapathAnalyzer entries, one for 'a' and one for 'b'.
    pub datapath: IDatapathPb,
    pub comparisons: Vec<IComparePb>,
}

#[derive(Debug, Clone)]
pub struct RelAnalyzer {
    pub rel_name: String,
    pub rel_id: MtOidT,
    pub inverse: bool,
    pub tgt_ds_name: String,
    pub tgt_ds_id: MtOidT,
    pub pk_segs: Vec<String>,
    pub index_root_id: MtOidT,
}

#[derive(Debug)]
pub struct IndexAnalyzer {
    pub iindex: IIndexPb,
    pub dpth_analyzers: Vec<DatapathAnalyzer>, // one datapath per segment
}

#[derive(Debug)]
pub struct DatasetAnalyzer {
    pub idataset: IDatasetPb,
    pub dataset_desc: DatasetDesc,
    // Plan comes in like [iclass, idatapath, icomp ...] we replace iclass by iindex if found
    pub index_analyzers: Vec<IndexAnalyzer>,
    // For insert rel plan comes like [iclass, irel], we replace by [iclass/iindex, irel],
    pub rel_analyzers: Vec<RelAnalyzer>,
}

// Generate index candidate insts for all the dataset indexes.
// Used:
// - in 'insert into' simple case as all indexes are updated, translate generates constant sqlvals
//   that hold the new item contents in the plan.
// - for other usages, this is a first cut that is refined later
pub fn get_index_candidates_for_dataset(dataset_desc: &DatasetDesc) -> Vec<IIndexPb> {
    let mut candidates = vec![];
    for idx_desc in &dataset_desc.indexes {
        let index_key_num = idx_desc._id as MtOidT;
        let index_name = idx_desc.name.clone();
        let composite_paths = idx_desc.segs.clone();
        debug!(
            "Index name: {:?}, root_key: {:?}, segs: {:?}",
            index_name, index_key_num, composite_paths
        );
        let index_type = match idx_desc.idx_type.as_str() {
            "pkey" => IndexTypePb::Pkey as i32,
            "unique" => IndexTypePb::Unique as i32,
            _ => IndexTypePb::Default as i32,
        };
        // For each path in composite paths, generate a segment with str and vec forms
        let index_seg_strs = composite_paths.clone();
        let index_seg_vecs = composite_paths
            .iter()
            .map(|p| IndexSegPb {
                seg_vec: p.split('.').map(|s| s.to_string()).collect(),
            })
            .collect::<Vec<IndexSegPb>>();
        let sqlinst = IIndexPb {
            idx_type: index_type,
            op: IndexOpPb::Scan as i32,
            ds_name: dataset_desc.name.clone(),
            name: index_name,
            root_id: index_key_num,
            key_val_idx: -1, // filled by optimizer in optimize pass
            seg_strs: index_seg_strs,
            seg_vecs: index_seg_vecs,
            range: None,
        };
        candidates.push(sqlinst);
    }
    candidates
}

// Load dataset desc into analyzer, return None if already have one, error if not found
pub fn build_ds_desc_analyzer(
    ctxt: &impl SqlExeTrait,
    ds_name: &String,
    dataset_key_val_idx: i32,
    analyzers: &Vec<DatasetAnalyzer>,
) -> Result<Option<DatasetAnalyzer>, SqlAnalyzeError> {
    // Check if already in analyzers
    for analyzer in analyzers {
        if analyzer.idataset.name == *ds_name {
            // Done for the dataset, dataset and rel details to be found there
            return Ok(None);
        }
    }
    // Get dataset id from schema
    let tgt_ds_id_opt = get_schema_key(ctxt, ds_name, MtSchemaType::KeyDataset);
    let tgt_ds_id = match tgt_ds_id_opt {
        Some(id) => id,
        None => return Err(SqlAnalyzeError::DatasetNotFound(ds_name.clone())),
    };
    if tgt_ds_id < MIN_USER_DATASET_ID {
        return Ok(None);
    }

    // Get dataset descriptor
    let dataset_desc_opt = if ds_name == "dataset" {
        None
    } else {
        get_dataset_desc(ctxt, tgt_ds_id)
    };
    let dataset_desc = match dataset_desc_opt {
        Some(d) => d,
        None => {
            if ds_name == "dataset" {
                DatasetDesc {
                    name: ds_name.clone(),
                    _id: tgt_ds_id,
                    rels: vec![],
                    indexes: vec![],
                }
            } else {
                return Err(SqlAnalyzeError::DatasetNotFound(ds_name.clone()));
            }
        }
    };
    Ok(Some(DatasetAnalyzer {
        idataset: IDatasetPb {
            name: ds_name.clone(),
            dataset_id: tgt_ds_id,
            key_val_idx: dataset_key_val_idx,
        },
        dataset_desc: dataset_desc,
        index_analyzers: vec![], // to be populated later
        rel_analyzers: vec![],   // to be populated later
    }))
}

pub fn get_dataset_desc(ctxt: &impl SqlExeTrait, dataset_id: MtOidT) -> Option<DatasetDesc> {
    // Fetch the class descriptor
    let mut mt_class_id: MtOidT = 0;
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size: c_uint = data_part.len() as c_uint;
    let sts = ctxt.get_datapart(
        ctxt.get_ltime(),
        dataset_id,
        &mut mt_class_id,
        &mut data_part,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        debug!("Failed to fetch dataset descriptor for '{}'", dataset_id);
        return None;
    }
    // Get index name and segs from json descriptor.
    let dataset_desc =
        match jbparse::jsonb_to_dataset_desc(&data_part[..data_size as usize].to_vec()) {
            Some(v) => v,
            None => {
                println!("Failed to decode dataset descriptor for '{}'", dataset_id);
                return None;
            }
        };
    Some(dataset_desc)
}

// Compute range for one index, one datapath matching one composite key segment
pub fn compute_index_range_for_datapath(
    segment: &String,
    idpth: &IDatapathPb,
    path: &String,
    icomps: &Vec<IComparePb>,
    sqlvals: &Vec<SqlValuePb>,
) -> Option<RangePb> {
    // Return none if datapath path doesn't match the segment
    if path != segment {
        return None;
    }
    let mut range = RangePb {
        lb_val_idx: -1,
        lb_nb_vals: 1,
        ub_val_idx: -1,
        lb_op: 0,
        ub_op: 0,
    };

    // For 'IN(x, y)' we expect only one comparison operator
    for icomp in icomps {
        if icomp.comp as i32 == CompOperatorPb::In as i32 {
            range.lb_val_idx = icomp.right_val_idx;
            range.lb_nb_vals = icomp.right_val_cnt;
            range.lb_op = CompOperatorPb::In as i32;
            return Some(range);
        }
    }

    let lb_comp = compute_lb_for_datapath(idpth, icomps, sqlvals);
    // We are done if any equi comparison for the datapath
    if let Some(lb_res) = lb_comp {
        range.lb_val_idx = lb_res.0;
        range.lb_op = lb_res.1;
        if lb_res.1 == CompOperatorPb::Eq as i32 {
            return Some(range);
        }
    }
    let ub_comp = compute_ub_for_datapath(idpth, icomps, sqlvals);
    if let Some(ub_res) = ub_comp {
        range = RangePb {
            ub_val_idx: ub_res.0,
            ub_op: ub_res.1,
            ..range.clone()
        };
        return Some(range);
    }
    if range.lb_val_idx != -1 || range.ub_val_idx != -1 {
        return Some(range);
    }
    None
}

// Get schema item key by name and type
pub fn get_schema_key(
    ctxt: &impl SqlExeTrait,
    schema_name: &str,
    _schema_type: MtSchemaType,
) -> Option<u32> {
    let mut schema_id: MtOidT = 0;
    // Primary key for class 'Member { key_val_idx: 0, val_idx: 0, part: Path([PathSegment { name: "hiidx" }]) }': ["abc"]
    // Created root node for index 'hiidx.hiidx', root_id=0x10a9
    // Created index '{"name":"hiidx","segs":["abc"]}', key=0x10a4
    // Created class '{"name":"hiidx","indexes":[{"name":"hiidx","segs":["abc"]}]}', key=0x10a5
    let sts = ctxt.get_schema_item(
        ctxt.get_tranid(),
        ctxt.get_ltime(),
        schema_name,
        &mut schema_id,
    );
    if sts == STS_SUCCESS {
        debug!("Found schema ID for '{}' 0x{:x}", schema_name, schema_id);
        Some(schema_id)
    } else {
        debug!("Failed to get schema ID for '{}'", schema_name);
        None
    }
}

// Compute either equi match or lower bound - a > 3
fn compute_lb_for_datapath(
    idpth: &IDatapathPb,
    icomps: &Vec<IComparePb>,
    sqlvals: &Vec<SqlValuePb>,
) -> Option<(i32, i32)> {
    let mut candidate_lb = None;
    for icomp in icomps {
        // Skip if we don't have an datapath
        if idpth.val_idx != icomp.left_val_idx && idpth.val_idx != icomp.right_val_idx {
            continue;
        }
        let dpth_on_left = if idpth.val_idx == icomp.left_val_idx {
            true
        } else {
            false
        };
        let const_val_idx = if dpth_on_left {
            icomp.right_val_idx
        } else {
            icomp.left_val_idx
        };
        let const_val = &sqlvals[const_val_idx as usize];
        // Skip if we don't have a constant
        if !const_val.is_constant {
            continue;
        }
        if icomp.comp as i32 == CompOperatorPb::Eq as i32
            || icomp.comp as i32 == CompOperatorPb::In as i32
        {
            // Skip other predicates if we got an equi match
            return Some((const_val_idx, icomp.comp));
        }
        if dpth_on_left && icomp.comp as i32 == CompOperatorPb::Gt as i32
            || icomp.comp as i32 == CompOperatorPb::Ge as i32
        {
            candidate_lb = Some((const_val_idx, icomp.comp));
        } else if !dpth_on_left && icomp.comp as i32 == CompOperatorPb::Lt as i32
            || icomp.comp as i32 == CompOperatorPb::Le as i32
        {
            let flipped_comp = if icomp.comp as i32 == CompOperatorPb::Lt as i32 {
                CompOperatorPb::Gt
            } else {
                CompOperatorPb::Ge
            };
            candidate_lb = Some((const_val_idx, flipped_comp as i32));
        }
    }
    return candidate_lb;
}

// Upper bound - a < 3
fn compute_ub_for_datapath(
    idpth: &IDatapathPb,
    icomps: &Vec<IComparePb>,
    sqlvals: &Vec<SqlValuePb>,
) -> Option<(i32, i32)> {
    for icomp in icomps {
        // Skip if we don't have an datapath
        if idpth.val_idx != icomp.left_val_idx && idpth.val_idx != icomp.right_val_idx {
            continue;
        }
        let dpth_on_left = if idpth.val_idx == icomp.left_val_idx {
            true
        } else {
            false
        };
        let const_val_idx = if dpth_on_left {
            icomp.right_val_idx
        } else {
            icomp.left_val_idx
        };
        let const_val = &sqlvals[const_val_idx as usize];
        // Skip if we don't have a constant
        if !const_val.is_constant {
            continue;
        }
        if dpth_on_left && icomp.comp == CompOperatorPb::Lt as i32
            || icomp.comp as i32 == CompOperatorPb::Le as i32
        {
            return Some((const_val_idx, icomp.comp));
        } else if !dpth_on_left
            && (icomp.comp == CompOperatorPb::Gt as i32 || icomp.comp == CompOperatorPb::Ge as i32)
        {
            let flipped_comp = if icomp.comp == CompOperatorPb::Gt as i32 {
                CompOperatorPb::Lt
            } else {
                CompOperatorPb::Le
            };
            return Some((const_val_idx, flipped_comp as i32));
        }
    }
    return None;
}

pub fn get_pkey_index_for_dataset(
    ds_name: &String,
    analyzers: &Vec<DatasetAnalyzer>,
) -> Option<IIndexPb> {
    let dataset_analyzer_opt = analyzers.iter().find(|a| a.dataset_desc.name == *ds_name);
    let dataset_analyzer = match dataset_analyzer_opt {
        Some(analyzer) => analyzer,
        None => return None,
    };
    for idx_desc in &dataset_analyzer.dataset_desc.indexes {
        if idx_desc.idx_type == "pkey" {
            let index_id = idx_desc._id as MtOidT;
            let index_name = idx_desc.name.clone();
            let composite_paths = idx_desc.segs.clone();
            debug!(
                "Found target dataset PK index for '{}': {:?}",
                ds_name, idx_desc
            );
            let index_type = IndexTypePb::Pkey as i32;
            // For each path in composite paths, generate a segment with str and vec forms
            let index_seg_strs = composite_paths.clone();
            let index_seg_vecs = composite_paths
                .iter()
                .map(|p| IndexSegPb {
                    seg_vec: p.split('.').map(|s| s.to_string()).collect(),
                })
                .collect::<Vec<IndexSegPb>>();
            return Some(IIndexPb {
                idx_type: index_type,
                op: IndexOpPb::Scan as i32,
                ds_name: ds_name.clone(),
                name: index_name,
                root_id: index_id,
                key_val_idx: -1, // filled by optimizer in optimize pass
                seg_strs: index_seg_strs,
                seg_vecs: index_seg_vecs,
                range: None,
            });
        }
    }
    None
}

// Get target datasets details for rel
pub fn build_rel_details_for_reldesc(
    ctxt: &impl SqlExeTrait,
    rel_desc: &jbparse::RelDesc,
) -> Option<RelAnalyzer> {
    // Get target dataset id
    let tgt_ds_id = rel_desc._tgt_id as MtOidT;
    // Get target dataset descriptor
    let tgt_dataset_desc_opt = get_dataset_desc(ctxt, tgt_ds_id);
    let tgt_dataset_desc = match tgt_dataset_desc_opt {
        Some(desc) => desc,
        None => return None,
    };
    // Get target dataset PK index details
    for i in &tgt_dataset_desc.indexes {
        if i.idx_type == "pkey" {
            debug!(
                "Found target dataset PK index for relationship '{}': {:?}",
                rel_desc.name, i
            );
            return Some(RelAnalyzer {
                rel_name: rel_desc.name.clone(),
                rel_id: rel_desc._id,
                inverse: false, // TBD - to revisit
                tgt_ds_name: tgt_dataset_desc.name.clone(),
                tgt_ds_id: tgt_ds_id,
                index_root_id: i._id,
                pk_segs: i.segs.clone(),
            });
        };
    }
    None
}

pub fn get_invrel_for_offset(jpath: &Vec<InvRelEltPb>, offset: usize) -> Option<String> {
    for (i, elt) in jpath.iter().enumerate() {
        if i == offset {
            return Some(elt.target_ds.clone());
        }
    }
    None
}

pub fn vrels_target_details(
    analyzer: &DatasetAnalyzer,
    valrels: &Vec<ValRelsUpdPb>,
) -> Result<Vec<ValRelsUpdPb>, SqlAnalyzeError> {
    let mut vrels = vec![];
    for vrel in valrels {
        let mut rd = vec![];
        for r in &vrel.relupd {
            let rupd = relupd_target_details(analyzer, r);
            if rupd.is_err() {
                return Err(rupd.err().unwrap());
            }
            let rupd = rupd.unwrap();
            rd.push(rupd);
        }
        let vr = ValRelsUpdPb {
            relupd: rd,
            ..vrel.clone()
        };
        vrels.push(vr);
    }
    Ok(vrels)
}

// Set target dataset details for one rel update
pub fn relupd_target_details(
    analyzer: &DatasetAnalyzer,
    rel_inst: &RelUpdPb,
) -> Result<RelUpdPb, SqlAnalyzeError> {
    for rel_analyzer in &analyzer.rel_analyzers {
        if rel_analyzer.rel_name == rel_inst.name {
            debug!("Updating relupdate with dataset and key_val_idx details");
            let ru = RelUpdPb {
                ds_name: analyzer.idataset.name.clone(),
                rel_id: rel_analyzer.rel_id,
                tgt_ds_name: rel_analyzer.tgt_ds_name.clone(),
                tgt_ds_id: rel_analyzer.tgt_ds_id,
                tgt_index_root_id: rel_analyzer.index_root_id,
                ..rel_inst.clone()
            };
            return Ok(ru);
        }
    }
    Err(SqlAnalyzeError::InvalidRelationship(rel_inst.name.clone()))
}

// Identify path against index segment path, return true if path matches or is empty
pub fn path_matches(path: &Vec<String>, index_path: &Vec<String>) -> bool {
    for (i, p) in path.iter().enumerate() {
        if i >= index_path.len() {
            break;
        }
        // Update index if 100% matches in same depth segments.
        // For example:
        // p 'a', ip 'a.b' -> match
        // p 'a.b', ip 'a' -> match
        // p 'a.c', ip 'a.b' -> no match
        let ip = &index_path[i];
        if p != ip {
            return false;
        }
    }
    true
}

// Check if candidate path matches index segments, or empty segments for all indexes,
// return matched data paths datapath instances
pub fn get_index_insts_for_candidate_path(
    key_val_idx: i32,
    analyzers: &Vec<DatasetAnalyzer>,
    pathupds: &Vec<PathUpdPb>, // multiple paths for one update, e.g. 'a', 'b.c'
) -> Vec<IIndexPb> {
    let mut index_insts = vec![];
    debug!(
        "Looking for index matches for paths {:?} with key_val_idx {}",
        pathupds, key_val_idx
    );
    for analyzer in analyzers {
        if analyzer.idataset.key_val_idx != key_val_idx {
            continue;
        }
        for index_analyzer in &analyzer.index_analyzers {
            let index_seg_vecs = &index_analyzer.iindex.seg_vecs;
            let mut path_match = false;
            // Check all segs, for instance 'a', 'b.c'.
            for iseg_vec in index_seg_vecs {
                debug!(
                    "Check segs against index segment {:?} for index {:?}",
                    iseg_vec, index_analyzer.iindex.name
                );
                for pathupd in pathupds {
                    debug!("Check seg {:?} against index segments", pathupd.pathsegs);
                    let segs = pathupd
                        .pathsegs
                        .split('.')
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>();
                    // Update the index if any update jpath matches
                    if path_matches(&segs, &iseg_vec.seg_vec) {
                        debug!(
                            "Path segs {:?} matches index segments {:?}",
                            pathupd.pathsegs, iseg_vec.seg_vec
                        );
                        path_match = true;
                        break;
                    }
                }
            }
            // Generate index inst if any path seg matches
            if path_match {
                index_insts.push(IIndexPb {
                    key_val_idx: key_val_idx,
                    ..index_analyzer.iindex.clone()
                });
            }
        }
    }
    return index_insts;
}
