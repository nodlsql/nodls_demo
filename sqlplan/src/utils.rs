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

use sqlinsts::sqlinsts::{
    sql_inst_pb, sql_value_pb, CompOperatorPb, CompositeRangePb, DdlOpPb, DecimalValuePb, IDdlPb,
    IndexTypePb, RangePb, RelUpdPb, SqlInstPb, SqlPlanPb, SqlValuePb, UpdateOpPb, ValRelsUpdPb,
};
use sqlparse::ast;
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug, PartialEq)]
pub enum SqlTranslateError {
    #[error("Invalid update")]
    InvalidUpdate,
    #[error("Ambiguous datapath {0}")]
    AmbiguousDatapath(String),
    #[error("Decimal value too large")]
    DecimalOverflow,
}

const LIKE_REGEX_ESCAPE: [char; 13] = [
    '\\', '.', '+', '*', '?', '(', ')', '[', ']', '{', '}', '^', '$',
];

pub fn add_value(
    sqlplan: &mut SqlPlanPb,
    is_constant: bool,
    data: Option<sql_value_pb::Data>,
) -> i32 {
    let val_idx = sqlplan.max_value_idx;
    sqlplan.values.push(SqlValuePb {
        is_constant: is_constant,
        data: data,
    });
    sqlplan.max_value_idx += 1;
    val_idx
}

fn get_default_ddl_pb(ds_name: &str) -> IDdlPb {
    IDdlPb {
        op: DdlOpPb::CreateDs.into(),
        ds_name: ds_name.to_string(),
        ds_id: 0,
        name: "".to_string(),
        rs_tgt_name: "".to_string(),
        rs_tgt_id: 0,
        idx_type: IndexTypePb::Default.into(),
        seg_strs: vec![],
    }
}

pub fn ddl_create_dataset(sqlplan: &mut SqlPlanPb, ds_name: &str) -> () {
    debug!("DDL create dataset '{}'", ds_name);
    let inst = IDdlPb {
        ..get_default_ddl_pb(ds_name)
    };
    sqlplan.insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
    });
}

pub fn ddl_drop_dataset(sqlplan: &mut SqlPlanPb, ds_name: &str) -> () {
    debug!("DDL drop dataset '{}'", ds_name);
    let inst = IDdlPb {
        op: DdlOpPb::DropDs.into(),
        ..get_default_ddl_pb(ds_name)
    };
    sqlplan.insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
    });
}

pub fn ddl_describe_dataset(sqlplan: &mut SqlPlanPb, ds_name: &str) -> () {
    debug!("DDL describe dataset '{}'", ds_name);
    let inst = IDdlPb {
        op: DdlOpPb::DescribeDs.into(),
        ..get_default_ddl_pb(ds_name)
    };
    sqlplan.insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
    });
}

pub fn ddl_update_rels(
    sqlplan: &mut SqlPlanPb,
    ds_name: &str,
    actions: &Vec<ast::AlterAction>,
) -> Result<(), SqlTranslateError> {
    for action in actions {
        match action {
            ast::AlterAction::AddRel(rel) => {
                let inst = IDdlPb {
                    op: DdlOpPb::CreateRel.into(),
                    name: rel.name.clone(),
                    rs_tgt_name: rel.tgt_dataset.clone(),
                    rs_tgt_id: 0, // to be set by the optimizer
                    ..get_default_ddl_pb(ds_name)
                };
                sqlplan.insts.push(SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
                });
            }
            ast::AlterAction::DropRel(rel_name) => {
                let inst = IDdlPb {
                    op: DdlOpPb::DropRel.into(),
                    name: rel_name.clone(),
                    ..get_default_ddl_pb(ds_name)
                };
                sqlplan.insts.push(SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
                });
            }
            _ => {
                // Ignore other actions
            }
        }
    }
    Ok(())
}

pub fn ddl_update_indexes(
    sqlplan: &mut SqlPlanPb,
    ds_name: &str,
    actions: &Vec<ast::AlterAction>,
) -> Result<(), SqlTranslateError> {
    for action in actions {
        match action {
            ast::AlterAction::AddIdx(i) => {
                let iname = if i.name.is_empty() {
                    // same as ds name for pkey
                    ds_name
                } else {
                    &i.name
                };
                let mut seg_paths = vec![];
                for segs in &i.fields {
                    seg_paths.push(segs.segments.join("."));
                }
                let idx_type = match i.idx_type {
                    ast::IndexType::Pkey => IndexTypePb::Pkey as i32,
                    ast::IndexType::Unique => IndexTypePb::Unique as i32,
                    _ => IndexTypePb::Default as i32,
                };
                debug!("Primary key for dataset '{:?}': {:?}", ds_name, seg_paths);
                let index_seg_strs = seg_paths.clone();
                let inst = IDdlPb {
                    op: DdlOpPb::CreateIdx.into(),
                    idx_type: idx_type,
                    name: iname.to_string(), // TBD - pkey only for now
                    seg_strs: index_seg_strs,
                    ..get_default_ddl_pb(ds_name)
                };
                let iinst = SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
                };
                sqlplan.insts.push(iinst);
            }
            ast::AlterAction::DropIdx(idx_name) => {
                let mut idx_type = IndexTypePb::Default as i32;
                let iname = if idx_name.is_empty() {
                    idx_type = IndexTypePb::Pkey as i32;
                    // same as ds name for pkey
                    ds_name
                } else {
                    idx_name
                };
                let inst = IDdlPb {
                    op: DdlOpPb::DropIdx.into(),
                    idx_type: idx_type,
                    name: iname.to_string(),
                    ..get_default_ddl_pb(ds_name)
                };
                sqlplan.insts.push(SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
                });
            }
            _ => {
                // Ignore other actions
            }
        }
    }
    Ok(())
}

pub fn translate_update_rels(
    relsupds: &Vec<ast::RelsUpdate>,
    sqlplan: &mut SqlPlanPb,
) -> Result<Vec<RelUpdPb>, SqlTranslateError> {
    let mut upb = vec![];
    for relsupd in relsupds {
        let mut vrs = vec![];
        for rs in &relsupd.rels {
            let vrelpb = translate_update_one_rel(relsupd.op, rs, sqlplan)?;
            vrs.push(vrelpb);
        }
        upb.extend(vrs);
    }
    Ok(upb)
}

pub fn translate_update_one_rel(
    upd_op: ast::UpdateOp,
    relupd: &ast::RelUpdate,
    sqlplan: &mut SqlPlanPb,
) -> Result<RelUpdPb, SqlTranslateError> {
    let upd_type = match upd_op {
        ast::UpdateOp::Insert => UpdateOpPb::Insert as i32,
        ast::UpdateOp::Delete => UpdateOpPb::Delete as i32,
        _ => return Err(SqlTranslateError::InvalidUpdate),
    };
    // Loop through the stmt target PK values. For each element create a set of SqlValuePb and a
    // range to query the target dataset PK index.
    // 'rs (a, b) (c, d) ...'
    let mut composite_ranges = vec![];
    for elt in &relupd.elts {
        let mut ranges = vec![];
        for seg in &elt.segs {
            // Generate SqlValuePb for this PK segment
            let val_idx = add_constant_value(sqlplan, seg)?;
            ranges.push(RangePb {
                lb_val_idx: val_idx,
                lb_nb_vals: 1,
                ub_val_idx: val_idx,
                lb_op: CompOperatorPb::Eq as i32,
                ub_op: CompOperatorPb::Eq as i32,
            });
        }
        composite_ranges.push(CompositeRangePb { ranges });
        debug!(
            "Composite range for element: {:?}",
            composite_ranges.last().unwrap()
        );
    }
    let relupdate = RelUpdPb {
        name: relupd.name.clone(),
        ds_name: "".to_string(), // Unused for DML
        upd_op: upd_type,
        rel_id: 0,                   // Set by the analyzer
        tgt_ds_name: "".to_string(), // Set by the analyzer
        tgt_ds_id: 0,                // Set by the analyzer
        tgt_index_root_id: 0,        // Set by the analyzer
        ranges: composite_ranges,
    };
    Ok(relupdate)
}

pub fn add_constant_value(
    sqlplan: &mut SqlPlanPb,
    const_val: &ast::ConstValue,
) -> Result<i32, SqlTranslateError> {
    let val_idx = sqlplan.max_value_idx;
    match const_val {
        ast::ConstValue::IsNull() => {
            add_value(sqlplan, true, None);
        }
        ast::ConstValue::Null() => {
            add_value(sqlplan, true, Some(sql_value_pb::Data::NullValue(true)));
        }
        ast::ConstValue::Bool(bool_val) => {
            add_value(
                sqlplan,
                true,
                Some(sql_value_pb::Data::BoolValue(*bool_val)),
            );
        }
        ast::ConstValue::Number(num_str) => {
            if num_str.contains('.') {
                // Get number and scale for decimal value
                let parts: Vec<&str> = num_str.split('.').collect();
                let number = parts[0].to_string() + parts[1];
                let scale = parts[1].len() as u32;
                // First convert number to i64 with overflow check
                let number_i: i64 = number
                    .parse()
                    .map_err(|_| SqlTranslateError::DecimalOverflow)?;
                add_value(
                    sqlplan,
                    true,
                    Some(sql_value_pb::Data::DecimalValue(DecimalValuePb {
                        number: number_i,
                        scale,
                    })),
                );
            } else {
                add_value(
                    sqlplan,
                    true,
                    Some(sql_value_pb::Data::Int64Value(
                        num_str
                            .parse()
                            .map_err(|_| SqlTranslateError::DecimalOverflow)?,
                    )),
                );
            }
        }
        ast::ConstValue::SingleQuotedString(s) => {
            add_value(
                sqlplan,
                true,
                Some(sql_value_pb::Data::StringValue(s.clone())),
            );
        }
        ast::ConstValue::DoubleQuotedString(_) => {
            // unused outside of jsonpath
        }
    }
    Ok(val_idx)
}

pub fn like_pattern_to_regex(pattern: &String) -> String {
    let mut regex_pattern = String::from("^");
    for ch in pattern.chars() {
        match ch {
            '%' => regex_pattern.push_str(".*"),
            '_' => regex_pattern.push('.'),
            _ => {
                if LIKE_REGEX_ESCAPE.contains(&ch) {
                    regex_pattern.push('\\');
                }
                regex_pattern.push(ch);
            }
        }
    }
    regex_pattern.push('$');
    regex_pattern
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

fn fmt_range(crange: &CompositeRangePb) -> String {
    let mut rg_str = "".to_string();
    // Format ranges as: 'ranges: (lbix=2, lbop=Gt, ubix=0, ubop=Eq), (...)'
    for rg in &crange.ranges {
        let rg_str_part = format!(
            "(lbix={}, lbcnt={}, lbop={}, ubix={}, ubop={}), ",
            rg.lb_val_idx, rg.lb_nb_vals, rg.lb_op, rg.ub_val_idx, rg.ub_op
        );
        rg_str = format!("{}{}", rg_str, rg_str_part);
    }
    rg_str
}

fn fmt_relupd(val_rels_upd_pbs: &Vec<ValRelsUpdPb>) -> String {
    let mut relupds_str = "".to_string();
    for ru in val_rels_upd_pbs {
        let mut rup_str = "".to_string();
        let mut ranges_str = "".to_string();
        for (i, rup) in ru.relupd.iter().enumerate() {
            for range in &rup.ranges {
                let range_str = fmt_range(&range);
                ranges_str = format!("{}{}", ranges_str, range_str);
            }
            if i > 0 {
                rup_str = format!("{}\n        ", rup_str);
            }
            let rup_str_part = format!(
                "{{name={} ds={} upd={:?} rid={} tgt={} tgt_id={} tgt_root={} ranges={}}},",
                rup.name,
                rup.ds_name,
                rup.upd_op,
                rup.rel_id,
                rup.tgt_ds_name,
                rup.tgt_ds_id,
                rup.tgt_index_root_id,
                ranges_str
            );
            rup_str = format!("{}{}", rup_str, rup_str_part);
        }
        relupds_str = format!("{}{}", relupds_str, rup_str);
    }
    relupds_str
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
            let mut rels= "".to_string();
            for r in &a.rel_descs {
                rels = format!("{}{{name={} id={} pk_segs={:?}}},", rels, r.name, r.id, r.pk_segs);
            }
            format!(
                "IDatapath pathstr=\"{}\" pathsegs={:?} jsonpath={:?} parent_path={:?} ds_name={} alias={} key_val_idx={} val_idx={} phase={} rels={}",
                a.path_str, a.pathsegs, a.jsonpath, a.parent_path, a.ds_name, a.alias, a.key_val_idx, a.val_idx, a.phase, rels
            )
        }
        Some(sql_inst_pb::Inst::Rel(r)) => {
            format!(
                "IRel name={} ds_name={} rel_id={} inverse={} tgt_ds_name={} key_val_idx={} tgt_key_val_idx={}",
                r.name, r.ds_name, r.rel_id, r.inverse, r.tgt_ds_name, r.key_val_idx, r.tgt_key_val_idx
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
            if let Some(r) = &i.range {
                rg_str = fmt_range(r);
            }
            format!(
                "IIndex type={} op={} name={} segments={} root_id=0x{:x} key_val_idx={} ranges={}",
                i.idx_type, i.op, i.name, segments, i.root_id, i.key_val_idx, rg_str
            )
        }
        Some(sql_inst_pb::Inst::Insert(i)) => {
            let mut valsrels_str = "".to_string();
            for vr in &i.valsrels {
                let relupd_str = fmt_relupd(&i.valsrels);
                let vr_str_part = format!(
                    "{{key_val_idx={} val_idx={} relupd={}}}, ",
                    vr.key_val_idx, vr.val_idx, relupd_str
                );
                valsrels_str = format!("{}{}", valsrels_str, vr_str_part);
            }
            format!(
                "IInsert ds_name={} ds_id={} vals={}",
                i.ds_name, i.ds_id, valsrels_str
            )
        }
        Some(sql_inst_pb::Inst::Delete(d)) => {
            format!("IDelete key_val_idx={}", d.key_val_idx)
        }
        Some(sql_inst_pb::Inst::Update(u)) => {
            let mut pathupds_str = "".to_string();
            for pu in &u.pathupds {
                let pu_str_part = format!("{{upd_op={:?} val_idx={} pathsegs={:?}}},", pu.upd_op, pu.val_idx, pu.pathsegs);
                pathupds_str = format!("{}{}", pathupds_str, pu_str_part);
            }
            let relupds_str = fmt_relupd(&u.relupds);
            format!(
                "IUpdate key_val_idx={} pathupds={} relupds={}",
                u.key_val_idx, pathupds_str, relupds_str
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
