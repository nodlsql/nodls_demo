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

use sqlinsts::{sql_inst_pb, DdlOpPb, IDdlPb, IndexTypePb, SqlInstPb, SqlPlanPb};
use sqloptimize::utils::SqlTranslateError;
use sqlparser::ast;
use tracing::debug;

const LIKE_REGEX_ESCAPE: [char; 13] = [
    '\\',
    '.',
    '+',
    '*',
    '?',
    '(',
    ')',
    '[',
    ']',
    '{',
    '}',
    '^',
    '$',
];

fn get_default_ddl_pb(ds_name: &str) -> IDdlPb {
    IDdlPb {
        op: DdlOpPb::CreateDs.into(),
        ds_name: ds_name.to_string(),
        ds_id: 0,
        name: "".to_string(),
        tgt_ds_name: "".to_string(),
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
                    tgt_ds_name: rel.tgt_dataset.clone(),
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
                    name: iname.to_string(),   // TBD - pkey only for now
                    seg_strs: index_seg_strs,
                    ..get_default_ddl_pb(ds_name)
                };
                let iinst = SqlInstPb {
                    inst: Some(sql_inst_pb::Inst::DdlUpdate(inst)),
                };
                sqlplan.insts.push(iinst);
            }
            ast::AlterAction::DropIdx(idx_name) => {
                let iname = if idx_name.is_empty() {
                    // same as ds name for pkey
                    ds_name
                } else {
                    idx_name
                };
                let inst = IDdlPb {
                    op: DdlOpPb::DropIdx.into(),
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
            },
        }
    }
    regex_pattern.push('$');
    regex_pattern
}