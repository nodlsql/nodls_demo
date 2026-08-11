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

use ast::CompOperator;
use sqlexet::SqlExeTrait;
use sqlinsts::{
    sql_inst_pb, sql_value_pb, CompOperatorPb, DecimalValuePb, EvalPhasePb, IDatapathPb, IProjPb,
    IRelUpdPb, InvRelEltPb, RelOpPb, SqlInstPb, SqlPlanPb, SqlStmtPb, SqlValuePb,
};
use sqloptimize::{add_value, utils::SqlTranslateError};
use sqlparser::ast;
use tracing::debug;

pub mod utils;

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum StmtType {
    Select,
    CreateDataset,
    DropDataset,
    DescribeDataset,
    AlterDataset,
    InsertInto,
    UpdateRel,
    Projection,
}

#[derive(PartialEq)]
pub enum TranslatePhase {
    Predicates,
    Proj,
}

pub fn translate(
    ctxt: &mut impl SqlExeTrait,
    stmt: &mut ast::SqlStmt,
) -> Result<SqlPlanPb, SqlTranslateError> {
    let mut sqlplan;

    debug!("AST: {}", stmt.pretty_print());

    // Use ltime to fetch the schema, no need to end it as we deactivate task
    let ltime = ctxt.start_local_qtran();
    ctxt.set_ltime(ltime);
    match stmt {
        // Select statement
        ast::SqlStmt::Select(sel) => {
            sqlplan = SqlPlanPb {
                sqlstmt: SqlStmtPb::Select.into(),
                insts: vec![],
                values: vec![],
                max_value_idx: 0,
            };

            translate_from_list(&mut sqlplan, &mut sel.from_list)?;
            translate_predicates(&mut sqlplan, &mut sel.predicate_list, &sel.from_list)?;
            translate_proj(&mut sqlplan, &mut sel.proj_list, &sel.from_list)?;
            println!(
                "-------------------------------------------------------------------------------"
            );
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        // Create dataset statement
        ast::SqlStmt::CreateDataset(ds) => {
            sqlplan = translate_create_dataset(ds)?;
            // Check if dataset already exists
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        // Drop dataset statement
        ast::SqlStmt::DropDataset(ds) => {
            sqlplan = translate_drop_dataset(ds)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        // Describe dataset statement
        ast::SqlStmt::DescribeDataset(ds) => {
            sqlplan = translate_describe_dataset(ds)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        // Alter dataset statement
        ast::SqlStmt::AlterDataset(alter) => {
            sqlplan = translate_alter_dataset(alter)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        // Delete from statement
        ast::SqlStmt::DeleteFrom(del) => {
            sqlplan = SqlPlanPb {
                sqlstmt: SqlStmtPb::DeleteFrom.into(),
                insts: vec![],
                values: vec![],
                max_value_idx: 0,
            };
            translate_from_list(
                &mut sqlplan,
                &vec![ast::FromListItem {
                    ds_name: del.ds_name.clone(),
                    alias: None,
                }],
            )?;
            translate_predicates(
                &mut sqlplan,
                &mut del.predicate_list,
                &vec![ast::FromListItem {
                    ds_name: del.ds_name.clone(),
                    alias: None,
                }],
            )?;
            // Back to business
            translate_delete_from(del, &mut sqlplan)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        // Insert Into statement
        ast::SqlStmt::InsertInto(insert) => {
            sqlplan = translate_insert_into(insert)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        ast::SqlStmt::Update(update) => {
            sqlplan = SqlPlanPb {
                sqlstmt: SqlStmtPb::Update.into(),
                insts: vec![],
                values: vec![],
                max_value_idx: 0,
            };
            let from_list = vec![ast::FromListItem {
                ds_name: update.ds_name.clone(),
                alias: None,
            }];
            translate_from_list(&mut sqlplan, &from_list)?;
            translate_predicates(&mut sqlplan, &mut update.predicate_list, &from_list)?;
            translate_update(update, &mut sqlplan)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        ast::SqlStmt::Yank(yank) => {
            sqlplan = SqlPlanPb {
                sqlstmt: SqlStmtPb::Yank.into(),
                insts: vec![],
                values: vec![],
                max_value_idx: 0,
            };
            let from_list = vec![ast::FromListItem {
                ds_name: yank.ds_name.clone(),
                alias: None,
            }];
            translate_from_list(&mut sqlplan, &from_list)?;
            translate_predicates(&mut sqlplan, &mut yank.predicate_list, &from_list)?;
            translate_yank(yank, &mut sqlplan)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
        ast::SqlStmt::UpdateRel(update_rel) => {
            sqlplan = SqlPlanPb {
                sqlstmt: SqlStmtPb::UpdateRel.into(),
                insts: vec![],
                values: vec![],
                max_value_idx: 0,
            };
            translate_from_list(&mut sqlplan, &update_rel.from_list)?;
            translate_predicates(
                &mut sqlplan,
                &mut update_rel.predicate_list,
                &update_rel.from_list,
            )?;
            translate_update_rel(update_rel, &mut sqlplan)?;
            sqloptimize::optimize_plan(ctxt, &mut sqlplan)?;
        }
    }

    #[cfg(demoprt)]
    println!("SQL Plan:\n{}", sqloptimize::utils::pretty_print_plan(&sqlplan));

    Ok(sqlplan)
}

pub fn translate_create_dataset(
    dataset_stmt: &mut ast::CreateDatasetStmt,
) -> Result<SqlPlanPb, SqlTranslateError> {
    // Create dataset statement
    debug!("Translating CREATE DATASET statement: {:?}", dataset_stmt);
    let mut sqlplan = SqlPlanPb {
        sqlstmt: SqlStmtPb::CreateDataset.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 0,
    };
    utils::ddl_create_dataset(&mut sqlplan, &dataset_stmt.name);
    utils::ddl_update_indexes(&mut sqlplan, &dataset_stmt.name, &dataset_stmt.actions)?;
    utils::ddl_update_rels(&mut sqlplan, &dataset_stmt.name, &dataset_stmt.actions)?;
    Ok(sqlplan)
}

pub fn translate_drop_dataset(
    dataset_stmt: &mut ast::DropDatasetStmt,
) -> Result<SqlPlanPb, SqlTranslateError> {
    // Drop dataset statement
    debug!("Translating DROP DATASET statement: {:?}", dataset_stmt);
    let mut sqlplan = SqlPlanPb {
        sqlstmt: SqlStmtPb::AlterDataset.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 0,
    };
    utils::ddl_drop_dataset(&mut sqlplan, &dataset_stmt.name);
    Ok(sqlplan)
}

pub fn translate_describe_dataset(
    dataset_stmt: &mut ast::DescribeDatasetStmt,
) -> Result<SqlPlanPb, SqlTranslateError> {
    let mut sqlplan = SqlPlanPb {
        sqlstmt: SqlStmtPb::AlterDataset.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 0,
    }; // Describe dataset statement
    debug!("Translating DESCRIBE DATASET statement: {:?}", dataset_stmt);
    utils::ddl_describe_dataset(&mut sqlplan, &dataset_stmt.name);
    Ok(sqlplan)
}

pub fn translate_alter_dataset(
    alter_stmt: &mut ast::AlterDatasetStmt,
) -> Result<SqlPlanPb, SqlTranslateError> {
    // Alter dataset statement
    debug!("Translating ALTER DATASET statement: {:?}", alter_stmt);
    let mut sqlplan = SqlPlanPb {
        sqlstmt: SqlStmtPb::AlterDataset.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 0,
    };
    // IDataset instruction
    utils::ddl_update_rels(&mut sqlplan, &alter_stmt.ds_name, &alter_stmt.actions)?;
    utils::ddl_update_indexes(&mut sqlplan, &alter_stmt.ds_name, &alter_stmt.actions)?;
    Ok(sqlplan)
}

pub fn translate_delete_from(
    del: &ast::DeleteFromStmt,
    sqlplan: &mut SqlPlanPb,
) -> Result<(), SqlTranslateError> {
    debug!("Translating DELETE FROM statement: {:?}", del);
    let del_inst = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Delete(sqlinsts::IDeletePb {
            key_val_idx: -1, // Set by optimizer
        })),
    };
    sqlplan.insts.push(del_inst);
    Ok(())
}

pub fn translate_insert_into(insert: &ast::InsertIntoStmt) -> Result<SqlPlanPb, SqlTranslateError> {
    debug!(
        "Translating INSERT INTO statement with values: {:?}",
        insert.values
    );
    let mut sqlplan = SqlPlanPb {
        sqlstmt: SqlStmtPb::InsertInto.into(),
        insts: vec![],
        values: vec![],
        max_value_idx: 0,
    };
    // Allocate key_val_idx
    sqlplan.values.push(SqlValuePb {
        is_constant: true,
        data: None,
    });
    sqlplan.max_value_idx += 1;
    // Set values to insert
    let mut val_idxs = vec![];
    for val in &insert.values {
        // Values here are full json objects
        let val_idx = sqlplan.max_value_idx;
        sqlplan.values.push(SqlValuePb {
            is_constant: true,
            data: Some(sql_value_pb::Data::StringValue(val.clone())),
        });
        sqlplan.max_value_idx = val_idx + 1;
        val_idxs.push(val_idx);
    }
    let iinsert = sqlinsts::IInsertPb {
        ds_name: insert.ds_name.clone(),
        ds_id: 0,       // Set by the optimizer
        key_val_idx: 0, // Only one dataset
        val_idxs: val_idxs,
    };
    sqlplan.insts.push(SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Insert(iinsert)),
    });
    Ok(sqlplan)
}

pub fn translate_update(
    update: &ast::UpdateStmt,
    sqlplan: &mut SqlPlanPb,
) -> Result<(), SqlTranslateError> {
    let mut valixs = vec![];
    let mut updsegs = vec![];
    for setval in &update.values {
        let val_idx = add_constant_value(sqlplan, &setval.value)?;
        valixs.push(val_idx);
        let segs_str = setval.fieldsegs.segments.join(".");
        updsegs.push(segs_str);
    }
    let set_inst = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Update(sqlinsts::IUpdatePb {
            key_val_idx: 0,
            val_idxs: valixs,
            pathsegs: updsegs,
        })),
    };
    sqlplan.insts.push(set_inst);
    Ok(())
}

pub fn translate_yank(
    yank: &ast::YankStmt,
    sqlplan: &mut SqlPlanPb,
) -> Result<(), SqlTranslateError> {
    let mut yanksegs = vec![];
    for segs in &yank.fields {
        let segs_str = segs.segments.join(".");
        yanksegs.push(segs_str);
    }
    let set_inst = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Yank(sqlinsts::IYankPb {
            key_val_idx: 0,
            pathsegs: yanksegs,
        })),
    };
    sqlplan.insts.push(set_inst);
    Ok(())
}

pub fn translate_update_rel(
    update: &ast::UpdateRelStmt,
    sqlplan: &mut SqlPlanPb,
) -> Result<(), SqlTranslateError> {
    // TBD - should put response with number of successors added, make sure we don't
    // TBD - insert duplicates if same PK successor already there.

    // Loop through the stmt target PK values. For each RelSuccessor create a set of SqlValuePb and a
    // range to query the target dataset PK index.
    let mut composite_ranges = vec![];
    for successor in &update.values {
        let mut ranges = vec![];
        for val in &successor.s {
            // Generate SqlValuePb for this PK segment
            let val_idx = add_constant_value(sqlplan, val)?;
            ranges.push(sqlinsts::RangePb {
                lower_bound_val_idx: val_idx,
                lower_bound_nb_vals: 1,
                upper_bound_val_idx: val_idx,
                lower_op: CompOperatorPb::Eq as i32,
                upper_op: CompOperatorPb::Eq as i32,
            });
        }
        composite_ranges.push(sqlinsts::CompositeRangePb { ranges });
        debug!(
            "Composite range for successor: {:?}",
            composite_ranges.last().unwrap()
        );
    }
    let upd_type = match update.update_type {
        ast::UpdateType::Insert => RelOpPb::Insert as i32,
        ast::UpdateType::Delete => RelOpPb::Delete as i32,
        _ => return Err(SqlTranslateError::InvalidUpdate),
    };
    let irelupdate = IRelUpdPb {
        name: update.name.clone(),
        ds_name: "".to_string(), // Unused for DML
        upd_type: upd_type,
        rel_id: 0,                   // Set by the analyzer
        tgt_ds_name: "".to_string(), // Set by the analyzer
        tgt_ds_id: 0,                // Set by the analyzer
        tgt_index_root_id: 0,        // Set by the analyzer
        key_val_idx: -1,             // Set by the analyzer. Maps to iclass/iindex scan key val idx
        ranges: composite_ranges,
    };

    let relupdate_inst = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::RelUpdate(irelupdate)),
    };
    sqlplan.insts.push(relupdate_inst);
    Ok(())
}

// For 'SELECT a FROM b WHERE a.c = 'd'
// 1. Loop on from list, generate IDataset and V0 to hold the key val
pub fn translate_from_list(
    sqlplan: &mut SqlPlanPb,
    from_list: &Vec<ast::FromListItem>,
) -> Result<(), SqlTranslateError> {
    for dataset_item in from_list.iter() {
        translate_idataset(sqlplan, &dataset_item)?;
    }
    Ok(())
}

pub fn translate_idataset(
    sqlplan: &mut SqlPlanPb,
    dataset_item: &ast::FromListItem,
) -> Result<(), SqlTranslateError> {
    // Add placeholder for item keys retrieved from dataset nested loop
    let idx = add_value(sqlplan, false, None);
    let idataset = sqlinsts::IDatasetPb {
        name: dataset_item.ds_name.to_string(),
        dataset_id: 0, // set by the optimizer
        key_val_idx: idx,
    };
    let inst = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Dataset(idataset)),
    };
    debug!("IDatasetPb: {:?}", inst);
    sqlplan.insts.push(inst);
    Ok(())
}

pub fn translate_predicates(
    sqlplan: &mut SqlPlanPb,
    predicates: &mut Vec<ast::Predicate>,
    from_list: &Vec<ast::FromListItem>,
) -> Result<(), SqlTranslateError> {
    for pred in predicates.iter_mut() {
        let mut members = vec![&mut pred.left, &mut pred.right];
        let mut val_idx = vec![-1, -1];
        let mut val_idx_elt = 0;
        let mut right_val_cnt = 1;
        for member in members.iter_mut() {
            match &member.part {
                ast::MemberPart::Value(const_val) => {
                    val_idx[val_idx_elt] = add_constant_value(sqlplan, const_val)?;
                    right_val_cnt = 1;
                }
                ast::MemberPart::ValueList(values) => {
                    for val in values.iter() {
                        let idx = add_constant_value(sqlplan, val)?;
                        if val_idx[val_idx_elt] == -1 {
                            val_idx[val_idx_elt] = idx;
                        }
                    }
                    right_val_cnt = values.len() as i32;
                }
                ast::MemberPart::Path(path_segments) => {
                    val_idx[val_idx_elt] = translate_idatapath(
                        sqlplan,
                        from_list,
                        path_segments,
                        EvalPhasePb::Predicate,
                    )?;
                }
                ast::MemberPart::Tree(left, op, right) => {
                    val_idx[val_idx_elt] = translate_expr(
                        sqlplan,
                        from_list,
                        op,
                        &left,
                        &right,
                        EvalPhasePb::Predicate,
                    )?;
                }
            }
            val_idx_elt += 1;
        }
        translate_icompare(sqlplan, pred, val_idx[0], val_idx[1], right_val_cnt);
    }
    Ok(())
}

pub fn translate_expr(
    sqlplan: &mut SqlPlanPb,
    from_list: &Vec<ast::FromListItem>,
    op: &ast::ArithOperator,
    left: &ast::Member,
    right: &ast::Member,
    eval_phase: EvalPhasePb,
) -> Result<i32, SqlTranslateError> {
    let lval_idx = match &left.part {
        ast::MemberPart::ValueList(_) => -1, // do nothing
        ast::MemberPart::Value(const_val) => add_constant_value(sqlplan, const_val)?,
        ast::MemberPart::Path(path_segments) => {
            translate_idatapath(sqlplan, from_list, path_segments, eval_phase)?
        }
        ast::MemberPart::Tree(left, op, right) => {
            translate_expr(sqlplan, from_list, op, &left, &right, eval_phase)?
        }
    };
    let rval_idx = match &right.part {
        ast::MemberPart::ValueList(_) => -1, // do nothing
        ast::MemberPart::Value(const_val) => add_constant_value(sqlplan, const_val)?,
        ast::MemberPart::Path(path_segments) => {
            translate_idatapath(sqlplan, from_list, path_segments, eval_phase)?
        }
        ast::MemberPart::Tree(left, op, right) => {
            translate_expr(sqlplan, from_list, op, &left, &right, eval_phase)?
        }
    };
    // For simplicity we only handle binary expressions with comparison operators here
    let math_op = match op {
        ast::ArithOperator::Plus => sqlinsts::OperPb::Add,
        ast::ArithOperator::Minus => sqlinsts::OperPb::Sub,
        ast::ArithOperator::Multiply => sqlinsts::OperPb::Mul,
        ast::ArithOperator::Divide => sqlinsts::OperPb::Div,
    };
    // Add a placeholder value to hold the result
    let resval_idx = add_value(sqlplan, false, None);

    let iexpr = sqlinsts::IExprPb {
        lval_idx,
        rval_idx,
        resval_idx,
        op: math_op as i32,
    };
    let inst = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Expr(iexpr)),
    };
    debug!("IExprPb: {:?}", inst);
    sqlplan.insts.push(inst);
    // Return the value index of the expression result for use in higher level expressions
    Ok(resval_idx)
}

pub fn translate_icompare(
    sqlplan: &mut SqlPlanPb,
    pred: &ast::Predicate,
    left_val_idx: i32,
    right_val_idx: i32,
    right_val_cnt: i32,
) {
    // For Like/NotLike convert the right value pattern to a regex pattern and store it in a constant value
    match pred.comp_operator {
        CompOperator::Like | CompOperator::NotLike => {
            if let Some(sql_value_pb::Data::StringValue(pattern)) =
                sqlplan.values[right_val_idx as usize].data.clone()
            {
                let regex_pattern = utils::like_pattern_to_regex(&pattern);
                sqlplan.values[right_val_idx as usize].data =
                    Some(sql_value_pb::Data::StringValue(regex_pattern));
            }
        }
        CompOperator::Regexp | CompOperator::NotRegexp => {
            // Do nothing
        }
        _ => {}
    }
    let icomp = sqlinsts::IComparePb {
        left_val_idx: left_val_idx,
        right_val_idx: right_val_idx,
        right_val_cnt: right_val_cnt,
        comp: match pred.comp_operator {
            CompOperator::In => CompOperatorPb::In as i32,
            CompOperator::NotIn => CompOperatorPb::NotIn as i32,
            CompOperator::Like => CompOperatorPb::Like as i32,
            CompOperator::NotLike => CompOperatorPb::NotLike as i32,
            CompOperator::Regexp => CompOperatorPb::Regexp as i32,
            CompOperator::NotRegexp => CompOperatorPb::NotRegexp as i32,
            CompOperator::Eq => CompOperatorPb::Eq as i32,
            CompOperator::Ne => CompOperatorPb::Ne as i32,
            CompOperator::Lt => CompOperatorPb::Lt as i32,
            CompOperator::Le => CompOperatorPb::Le as i32,
            CompOperator::Gt => CompOperatorPb::Gt as i32,
            CompOperator::Ge => CompOperatorPb::Ge as i32,
            // Unused in predicates
            CompOperator::EqEq => CompOperatorPb::Eq as i32,
        },
    };
    let msg = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Comp(icomp)),
    };
    debug!("IComparePb: {:?}", msg);
    sqlplan.insts.push(msg.clone());
}

fn translate_proj(
    sqlplan: &mut SqlPlanPb,
    proj_list: &mut Vec<ast::Member>,
    from_list: &Vec<ast::FromListItem>,
) -> Result<(), SqlTranslateError> {
    let mut val_idx;
    let mut col_num = 0;
    for member in proj_list.iter_mut() {
        let mut proj_name = "_".to_string();
        let mut proj_path = vec![];
        match &member.part {
            ast::MemberPart::ValueList(_) => {
                val_idx = -1; // do nothing
            }
            ast::MemberPart::Value(const_val) => {
                val_idx = add_constant_value(sqlplan, const_val)?;
            }
            ast::MemberPart::Path(path_segs) => {
                val_idx =
                    translate_idatapath(sqlplan, from_list, path_segs, EvalPhasePb::Projection)?;
                proj_path = path_segs
                    .segments
                    .iter()
                    .map(|seg| seg.name.clone())
                    .collect();
                proj_name = proj_path.join(".");
            }
            ast::MemberPart::Tree(left, op, right) => {
                val_idx = translate_expr(
                    sqlplan,
                    from_list,
                    op,
                    &left,
                    &right,
                    EvalPhasePb::Projection,
                )?;
            }
        }
        let proj_inst = IProjPb {
            proj_name: proj_name,
            path: proj_path,
            col_num: col_num,
            val_idx: val_idx,
        };
        let inst = SqlInstPb {
            inst: Some(sql_inst_pb::Inst::Proj(proj_inst)),
        };
        // Append to plan main insts
        sqlplan.insts.push(inst);
        col_num += 1;
    }
    Ok(())
}

// For 'SELECT a FROM b WHERE a.c = 'd'
// 1. Loop on from list, generate IDataset and V0 to hold the key val
// 2. loop on pred list to generate Idatapath constant V2
//   a. set V0 to key val idx from IDataset
//   b. generate V1 placeholder for datapath value and set V1
// 3. loop on pred list to generate IComp V1 V2
//
// Produces:
// IDataset key idx V0
// IConst V1 'd'
// Idatapath idx V0 val idx V2
// IComp V1 V2
// IProj key idx V0 val idx V1
pub fn translate_idatapath(
    sqlplan: &mut SqlPlanPb,
    from_list: &Vec<ast::FromListItem>,
    path: &ast::PathSegments,
    phase: EvalPhasePb,
) -> Result<i32, SqlTranslateError> {
    // Extract the name part from each PathSegment
    let mut path_segs: Vec<String> = path.segments.iter().map(|seg| seg.name.clone()).collect();

    debug!(
        "Translating IDatapath for path: {:?} jsonpath: {:?} phase: {:?}",
        path_segs, path.jsonpath, phase
    );
    // Get the target name and offset for inverse rels in the path segments
    let mut invrel_segs = vec![];
    let mut path_offset = 0;
    for seg in &path.segments {
        if !seg.target_ds.is_empty() {
            invrel_segs.push(InvRelEltPb {
                path_offset: path_offset,
                target_ds: seg.target_ds.clone(),
            });
        }
        path_offset += 1;
    }
    // Original path string
    let path_str: String = path_segs.join(".");
    let mut ds_matched = false;
    // Default to single dataset in from list if no match at head of path
    let ds_name = from_list[0].ds_name.clone();
    let mut alias = "".to_string();
    let mut parent_path = vec![ds_name.clone()];
    for ds_item in from_list {
        debug!("Checking from list dataset_name {}", ds_item.ds_name);
        alias = if let Some(a) = ds_item.alias.clone() {
            debug!("Dataset alias: '{}'", a);
            a
        } else {
            "".to_string()
        };
        if !&path.segments.is_empty() {
            let seg0 = &path.segments[0];
            if seg0.name == ds_item.ds_name || seg0.name == alias {
                // If the first segment of the path matches the dataset name, we can skip it in the datapath
                parent_path = vec![ds_item.ds_name.clone()];
                path_segs = path_segs[1..].to_vec();
                ds_matched = true;
                for inv_rel in &mut invrel_segs {
                    inv_rel.path_offset -= 1;
                }
                break;
            }
        }
    }
    if !ds_matched && from_list.len() > 1 {
        return Err(SqlTranslateError::AmbiguousDatapath(path_str));
    }
    let dpth_val_idx = add_value(sqlplan, false, None);
    let idatapath = IDatapathPb {
        phase: phase as i32,
        alias: alias.clone(),
        ds_name: parent_path[0].clone(),
        parent_path: parent_path.clone(),
        // At this stage path contains all segments beyond the dataset/alias,
        // for inv rel it includes the target dsname.
        pathsegs: path_segs.clone(),
        jsonpath: path.jsonpath.clone(),
        path_str: path_str.clone(),
        invrels: invrel_segs.clone(),
        rel_descs: vec![], // Set by the analyzer
        key_val_idx: -1,   // set by analyzer
        val_idx: dpth_val_idx,
    };
    let msg = SqlInstPb {
        inst: Some(sql_inst_pb::Inst::Dpath(idatapath)),
    };
    debug!("IDatapathPb: {:?}", msg);
    sqlplan.insts.push(msg.clone());
    // TBD - handle multiple from datasets
    Ok(dpth_val_idx)
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
