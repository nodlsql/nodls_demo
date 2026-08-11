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
use sqlinsts::{
    sql_inst_pb, sql_value_pb, CompOperatorPb, IComparePb, IDatapathPb, IDatasetPb, IIndexPb,
    IndexOpPb, IndexSegPb, IndexTypePb, InvRelEltPb, RangePb, SqlInstPb, SqlPlanPb, SqlValuePb,
};
use std::ffi::c_uint;
use thiserror::Error;
use tracing::debug;

// TBD - needs more accurate surrogate check
pub const MIN_USER_DATASET_ID: u32 = 0x1060;

#[derive(Error, Debug)]
pub enum SqlTranslateError {
    #[error("Dataset {0} already exists")]
    DatasetAlreadyExists(String),
    #[error("Dataset {0} not found")]
    DatasetNotFound(String),
    #[error("Invalid relationship {0}")]
    InvalidRelationship(String),
    #[error("Inverse relationship not found for datapath {0}")]
    InvRelNotFound(String),
    #[error("Invalid update")]
    InvalidUpdate,
    #[error("Ambiguous datapath {0}")]
    AmbiguousDatapath(String),
    #[error("Decimal value too large")]
    DecimalOverflow,
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
) -> Result<Option<DatasetAnalyzer>, SqlTranslateError> {
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
        None => return Err(SqlTranslateError::DatasetNotFound(ds_name.clone())),
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
                return Err(SqlTranslateError::DatasetNotFound(ds_name.clone()));
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
        lower_bound_val_idx: -1,
        lower_bound_nb_vals: 1,
        upper_bound_val_idx: -1,
        lower_op: 0,
        upper_op: 0,
    };

    // For 'IN(x, y)' we expect only one comparison operator
    for icomp in icomps {
        if icomp.comp as i32 == CompOperatorPb::In as i32 {
            range.lower_bound_val_idx = icomp.right_val_idx;
            range.lower_bound_nb_vals = icomp.right_val_cnt;
            range.lower_op = CompOperatorPb::In as i32;
            return Some(range);
        }
    }

    let lb_comp = compute_lb_for_datapath(idpth, icomps, sqlvals);
    // We are done if any equi comparison for the datapath
    if let Some(lb_res) = lb_comp {
        range.lower_bound_val_idx = lb_res.0;
        range.lower_op = lb_res.1;
        if lb_res.1 == CompOperatorPb::Eq as i32 {
            return Some(range);
        }
    }
    let ub_comp = compute_ub_for_datapath(idpth, icomps, sqlvals);
    if let Some(ub_res) = ub_comp {
        range = RangePb {
            upper_bound_val_idx: ub_res.0,
            upper_op: ub_res.1,
            ..range.clone()
        };
        return Some(range);
    }
    if range.lower_bound_val_idx != -1 || range.upper_bound_val_idx != -1 {
        return Some(range);
    }
    None
}

// Get schema item key by name and type
pub fn get_schema_key(
    ctxt: &impl SqlExeTrait,
    schema_name: &str,
    schema_type: MtSchemaType,
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
        schema_type, // This is ignored for now, afaik all schema descriptors are in same dict
        &mut schema_id,
    );
    if sts == STS_SUCCESS {
        debug!(
            "Found schema ID for '{}' type {}: 0x{:x}",
            schema_name, schema_type as i32, schema_id
        );
        Some(schema_id)
    } else {
        debug!(
            "Failed to get schema ID for '{}' type {}",
            schema_name, schema_type as i32
        );
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

// Get target datasets details for rel
pub fn build_rel_details_for_reldesc(
    ctxt: &impl SqlExeTrait,
    rel_desc: &jbparse::RelDesc,
) -> Option<RelAnalyzer> {
    // Get target dataset id
    let tgt_ds_id_opt = get_schema_key(ctxt, &rel_desc.tgt_dataset, MtSchemaType::KeyDataset);
    let tgt_ds_id = match tgt_ds_id_opt {
        Some(id) => id,
        None => return None,
    };
    // Get target dataset descriptor
    let tgt_dataset_desc_opt = get_dataset_desc(ctxt, tgt_ds_id);
    let tgt_dataset_desc = match tgt_dataset_desc_opt {
        Some(desc) => desc,
        None => return None,
    };
    // Get target dataset PK index details
    for i in &tgt_dataset_desc.indexes {
        if i.name == rel_desc.tgt_dataset {
            debug!(
                "Found target dataset PK index for relationship '{}': {:?}",
                rel_desc.name, i
            );
            return Some(RelAnalyzer {
                rel_name: rel_desc.name.clone(),
                rel_id: rel_desc._id,
                inverse: false, // TBD - to revisit
                tgt_ds_name: rel_desc.tgt_dataset.clone(),
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

// Pretty-print a SqlPlanPb with 4-space indentation and one line per SqlInstPb or SqlValuePb
pub fn pretty_print_plan(plan: &SqlPlanPb) -> String {
    let mut out = String::new();

    // Instructions
    for inst in &plan.insts {
        out.push_str("    ");
        out.push_str(&fmt_inst(inst));
        out.push('\n');
    }

    // Values
    for (i, val) in plan.values.iter().enumerate() {
        out.push_str("    ");
        out.push_str(&format!("val[{}]: {}", i, fmt_value(val)));
        out.push('\n');
    }
    out
}

fn fmt_inst(inst: &SqlInstPb) -> String {
    match inst.inst.as_ref() {
        Some(sql_inst_pb::Inst::Options(o)) => {
            format!(
                "ISetOptions limit_cnt={} limit_cnt_grp={} start_offset={} start_offset_grp={}",
                o.limit_cnt, o.limit_cnt_grp, o.start_offset, o.start_offset_grp
            )
        }
        Some(sql_inst_pb::Inst::DdlUpdate(d)) => {
            format!(
                "IDdlUpdate op={} name={} ds_name={} type={}",
                d.op, d.name, d.ds_name, d.idx_type
            )
        }
        Some(sql_inst_pb::Inst::Dataset(c)) => {
            format!(
                "IDataset name={} key_val_idx={} dataset_id={}",
                c.name, c.key_val_idx, c.dataset_id
            )
        }
        Some(sql_inst_pb::Inst::Dpath(a)) => {
            format!(
                "IDatapath pathstr={} pathsegs={:?} jsonpath={:?} parent_path={:?} ds_name={} alias={} key_val_idx={} val_idx={} phase={} rels={:?}",
                a.path_str, a.pathsegs, a.jsonpath, a.parent_path, a.ds_name, a.alias, a.key_val_idx, a.val_idx, a.phase, a.rel_descs
            )
        }
        Some(sql_inst_pb::Inst::Rel(r)) => {
            format!(
                "IRel name={} ds_name={} rel_id={} inverse={} tgt_ds_name={} key_val_idx={} tgt_key_val_idx={}",
                r.name, r.ds_name, r.rel_id, r.inverse, r.tgt_ds_name, r.key_val_idx, r.tgt_key_val_idx
            )
        }
        Some(sql_inst_pb::Inst::RelUpdate(r)) => {
            format!(
                "IRelUpdate name={} ds_name={} rel_id={} tgt_ds_name={} tgt_ds_id={} key_val_idx={} ranges={:?}",
                r.name, r.ds_name, r.rel_id, r.tgt_ds_name, r.tgt_ds_id, r.key_val_idx, r.ranges
            )
        }
        Some(sql_inst_pb::Inst::Comp(c)) => {
            format!(
                "ICompare comp={} left_val_idx={} right_val_idx={} right_val_cnt={}",
                c.comp, c.left_val_idx, c.right_val_idx, c.right_val_cnt
            )
        }
        Some(sql_inst_pb::Inst::Proj(p)) => {
            format!(
                "IProj name={} path={:?} val_idx={} col_num={}",
                p.proj_name, p.path, p.val_idx, p.col_num
            )
        }
        Some(sql_inst_pb::Inst::Index(i)) => {
            let segments = if i.seg_strs.is_empty() {
                "".to_string()
            } else {
                // Get comma-separated segment strings for display
                i.seg_strs
                    .iter()
                    .map(|s| s.clone())
                    .collect::<Vec<String>>()
                    .join(",")
            };
            let mut rg_str = "".to_string();
            // Format ranges as: 'ranges: (lbix=2, lbop=Gt, ubix=0, ubop=Eq), (...)'
            if let Some(r) = &i.range {
                for rg in &r.ranges {
                    let rg_str_part = format!(
                        "(lbix={}, lbcnt={}, lbop={}, ubix={}, ubop={}), ",
                        rg.lower_bound_val_idx, rg.lower_bound_nb_vals, rg.lower_op, rg.upper_bound_val_idx, rg.upper_op
                    );
                    rg_str = format!("{}{}", rg_str, rg_str_part);
                }
            }
            format!(
                "IIndex type={} op={} name={} segments={} root_id=0x{:x} key_val_idx={} ranges={}",
                i.idx_type, i.op, i.name, segments, i.root_id, i.key_val_idx, rg_str
            )
        }
        Some(sql_inst_pb::Inst::Insert(i)) => {
            format!(
                "IInsert ds_id={} key_val_idx={} val_idxs={:?}",
                i.ds_id, i.key_val_idx, i.val_idxs
            )
        }
        Some(sql_inst_pb::Inst::Delete(d)) => {
            format!("IDelete key_val_idx={}", d.key_val_idx)
        }
        Some(sql_inst_pb::Inst::Update(u)) => {
            format!(
                "IUpdate key_val_idx={} val_idxs={:?} pathsegs={:?}",
                u.key_val_idx, u.val_idxs, u.pathsegs
            )
        }
        Some(sql_inst_pb::Inst::Yank(u)) => {
            format!(
                "IYank key_val_idx={} pathsegs={:?}",
                u.key_val_idx, u.pathsegs
            )
        }
        Some(sql_inst_pb::Inst::Expr(e)) => {
            format!(
                "IExpr op={} lval={} rval={} resval={}",
                e.op, e.lval_idx, e.rval_idx, e.resval_idx
            )
        }
        None => "<empty-inst>".to_string(),
    }
}

fn fmt_value(v: &SqlValuePb) -> String {
    match v.data.as_ref() {
        None => "None".to_string(),
        Some(sql_value_pb::Data::BoolValue(b)) => format!("Bool({})", b),
        Some(sql_value_pb::Data::Int64Value(i)) => format!("Int64({})", i),
        Some(sql_value_pb::Data::OidValue(o)) => format!("OidValue({})", o),
        Some(sql_value_pb::Data::DecimalValue(d)) => format!("Decimal({:?})", d),
        Some(sql_value_pb::Data::StringValue(s)) => format!("String(\"{}\")", s),
        Some(sql_value_pb::Data::NullValue(_)) => "Null".to_string(),
    }
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
    paths: &Vec<String>, // multiple paths for one update, e.g. 'a', 'b.c'
) -> Vec<IIndexPb> {
    let mut index_insts = vec![];
    debug!(
        "Looking for index matches for paths {:?} with key_val_idx {}",
        paths, key_val_idx
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
                for seg_str in paths {
                    debug!("Check seg {:?} against index segments", seg_str);
                    let segs = seg_str
                        .split('.')
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>();
                    // Update the index if any update jpath matches
                    if path_matches(&segs, &iseg_vec.seg_vec) {
                        debug!(
                            "Path segs {:?} matches index segments {:?}",
                            seg_str, iseg_vec.seg_vec
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
