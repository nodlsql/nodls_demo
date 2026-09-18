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

use rust_decimal::Decimal;
use sqlexet::{MtOidT, SqlExeTrait, MIN_USER_DATASET_ID, MTS_OBJNOTFOUND, STS_SUCCESS};
use sqlinsts::sqlinsts::ValRelsUpdPb;
use sqlinsts::sqlinsts::{
    sql_value_pb, sql_value_pb::Data, CompOperatorPb, DecimalValuePb, EvalPhasePb, OperPb,
    SqlValuePb, UpdateOpPb, RangePb,
};
use std::cell::RefCell;
use std::cmp::Ordering;
use std::ffi::c_uint;
use tracing::debug;

use relpart::SqlRelAccessError;
use sqloptimize::utils::SqlAnalyzeError;
use sqlparse::SqlParseError;
use sqlplan::utils::SqlTranslateError;
use thiserror::Error;

// i64 max as i128
const I64_MAX_AS_I128: i128 = i64::MAX as i128;

#[derive(Error, Debug)]
pub enum SqlExecError {
    #[error("SQL Parse Error: {0}")]
    ParseError(#[from] SqlParseError),
    #[error("SQL Translate Error: {0}")]
    TranslateError(#[from] SqlTranslateError),
    #[error("SQL Analyze Error: {0}")]
    AnalyzeError(#[from] SqlAnalyzeError),
    #[error("Execution Error: {0}")]
    ExecutionError(String),
    #[error("RelAccess Error: {0}")]
    RelAccessError(#[from] SqlRelAccessError),
}

fn adjust_decimal_scale(d: &Decimal) -> Option<Decimal> {
    let mut scale = d.scale();
    let mut mantissa = d.mantissa();
    // reduce mantissa and decrease scale down to max i64 mantissa
    while scale != 0 {
        if mantissa.abs() < I64_MAX_AS_I128 {
            break;
        }
        debug!(
            "Mantissa {} exceeds i64 max, reducing mantissa and decreasing scale",
            mantissa
        );
        mantissa /= 10;
        scale -= 1;
    }
    // If still too big, return None to indicate overflow
    if mantissa.abs() >= I64_MAX_AS_I128 {
        debug!(
            "Result mantissa {} still exceeds i64 max after reduction, returning None for overflow",
            mantissa
        );
        return None; // or handle as needed
    }
    let adjusted = Decimal::new(mantissa as i64, scale as u32);
    debug!(
        "Adjusted result to avoid overflow, mantissa: {} scale: {} adjusted: {}",
        mantissa, scale, adjusted
    );
    Some(adjusted)
}

fn evaluate_decimal_expr(d1: &Decimal, d2: &Decimal, op: OperPb) -> Option<Decimal> {
    let result = match op {
        OperPb::Add => *d1 + *d2,
        OperPb::Sub => *d1 - *d2,
        OperPb::Mul => *d1 * *d2,
        OperPb::Div => {
            if d2.is_zero() {
                return None;
            }
            if let Some(d) = d1.checked_div(*d2) {
                d
            } else {
                return None;
            }
        }
    };
    debug!(
        "Raw result of decimal operation {:?} on {} and {} is {}",
        op, d1, d2, result
    );
    // Adjust scale for i64 mantissa
    adjust_decimal_scale(&result)
}

pub fn evaluate_expr(left_val: &SqlValuePb, right_val: &SqlValuePb, op: OperPb) -> Data {
    match left_val.data.as_ref() {
        Some(Data::Int64Value(i1)) => match right_val.data.as_ref() {
            Some(Data::Int64Value(i2)) => match op {
                OperPb::Add => {
                    if i1.checked_add(*i2).is_none() {
                        return Data::NullValue(true);
                    }
                    Data::Int64Value(i1 + i2)
                }
                OperPb::Sub => {
                    if i1.checked_sub(*i2).is_none() {
                        return Data::NullValue(true);
                    }
                    Data::Int64Value(i1 - i2)
                }
                OperPb::Mul => {
                    if i1.checked_mul(*i2).is_none() {
                        return Data::NullValue(true);
                    }
                    Data::Int64Value(i1 * i2)
                }
                OperPb::Div => {
                    if i1.checked_div(*i2).is_none() {
                        return Data::NullValue(true);
                    }
                    Data::Int64Value(i1 / i2)
                }
            },
            Some(Data::DecimalValue(d2)) => {
                let v1 = Decimal::new(*i1, 0);
                let v2 = Decimal::new(d2.number, (d2.scale as u8).into());
                match evaluate_decimal_expr(&v1, &v2, op) {
                    Some(result) => Data::DecimalValue(DecimalValuePb {
                        number: result.mantissa() as i64,
                        scale: result.scale() as u32,
                    }),
                    None => Data::NullValue(true),
                }
            }
            Some(Data::StringValue(s)) => {
                // Convert int to string and concatenate
                let s1 = i1.to_string();
                Data::StringValue(format!("{}{}", s1, s))
            }
            _ => {
                debug!("Type mismatch in expression evaluation: expected Int64Value or DecimalValue on the right");
                Data::NullValue(true)
            }
        },
        Some(Data::DecimalValue(d1)) => {
            match right_val.data.as_ref() {
                Some(Data::DecimalValue(d2)) => {
                    let v1 = Decimal::new(d1.number, (d1.scale as u8).into());
                    let v2 = Decimal::new(d2.number, (d2.scale as u8).into());
                    match evaluate_decimal_expr(&v1, &v2, op) {
                        Some(result) => Data::DecimalValue(DecimalValuePb {
                            number: result.mantissa() as i64,
                            scale: result.scale() as u32,
                        }),
                        None => Data::NullValue(true),
                    }
                }
                Some(Data::Int64Value(i2)) => {
                    let v1 = Decimal::new(d1.number, (d1.scale as u8).into());
                    let v2 = Decimal::new(*i2, 0);
                    match evaluate_decimal_expr(&v1, &v2, op) {
                        Some(result) => Data::DecimalValue(DecimalValuePb {
                            number: result.mantissa() as i64,
                            scale: result.scale() as u32,
                        }),
                        None => Data::NullValue(true),
                    }
                }
                Some(Data::StringValue(s)) => {
                    // Convert decimal to string and concatenate
                    let s1 = Decimal::new(d1.number, (d1.scale as u8).into()).to_string();
                    Data::StringValue(format!("{}{}", s1, s))
                }
                _ => {
                    debug!("Type mismatch in expression evaluation: expected DecimalValue on the right");
                    Data::NullValue(true)
                }
            }
        }
        Some(Data::StringValue(s1)) => match right_val.data.as_ref() {
            Some(Data::StringValue(s2)) => match op {
                OperPb::Add => Data::StringValue(format!("{}{}", s1, s2)),
                _ => {
                    debug!("Unsupported operator {:?} for StringValue", op);
                    Data::NullValue(true)
                }
            },
            Some(Data::Int64Value(_)) | Some(Data::DecimalValue(_)) => {
                match op {
                    OperPb::Add => {
                        // Convert int or decimal to string
                        let s2 = match right_val.data.as_ref() {
                            Some(Data::Int64Value(i2)) => i2.to_string(),
                            Some(Data::DecimalValue(d2)) => {
                                Decimal::new(d2.number, (d2.scale as u8).into()).to_string()
                            }
                            _ => unreachable!(),
                        };
                        Data::StringValue(format!("{}{}", s1, s2))
                    }
                    _ => {
                        debug!("Unsupported operator {:?} for StringValue and numeric", op);
                        Data::NullValue(true)
                    }
                }
            }
            Some(Data::NullValue(_)) => Data::StringValue(format!("{}{}", s1, "null")),
            Some(Data::BoolValue(b)) => {
                let s2 = b.to_string();
                Data::StringValue(format!("{}{}", s1, s2))
            }
            _ => {
                debug!("Type mismatch in expression evaluation: expected StringValue on the right");
                Data::NullValue(true)
            }
        },
        _ => {
            debug!(
                "Unsupported data type in expression evaluation: {:?}",
                left_val.data
            );
            Data::NullValue(true)
        }
    }
}

pub fn compare_regex(left_val: &SqlValuePb, regex_pattern: &str) -> bool {
    if let Some(Data::StringValue(s)) = &left_val.data {
        match regex::Regex::new(regex_pattern) {
            Ok(re) => re.is_match(s),
            Err(e) => {
                debug!("Invalid regex pattern '{}': {}", regex_pattern, e);
                false
            }
        }
    } else {
        debug!(
            "Expected StringValue for regex comparison, got: {:?}",
            left_val.data
        );
        false
    }
}

pub fn compare_values(
    left_val: &SqlValuePb,
    right_val: &SqlValuePb,
    comp_op: CompOperatorPb,
) -> bool {
    // TBD - need to handle nulls properly
    // TBD - '{a: null}' has a Data::NullValue variant, where
    // TBD - 1) IS NULL predicate evaluate for everything that has either the '{a: null}' value or path is absent.
    // TBD - 2) The '{a: null}' value predicate is 'WHERE ... a = null'.
    // TBD - 3) For path is absent, maybe some new predicate 'IS UNKNOWN' or just 'a IS NULL and NOT a = null'
    //
    // Give missing value precedence over explicit null value
    let left_val_none = left_val.data.is_none();
    let right_val_none = right_val.data.is_none();
    let mut result = Ordering::Equal;
    if left_val_none && right_val_none {
        result = Ordering::Equal;
    } else if left_val_none {
        result = Ordering::Less;
    } else if right_val_none {
        result = Ordering::Greater;
    };
    if comp_op == CompOperatorPb::Like
        || comp_op == CompOperatorPb::NotLike
        || comp_op == CompOperatorPb::Regexp
        || comp_op == CompOperatorPb::NotRegexp
    {
        if let Some(Data::StringValue(pattern)) = right_val.data.as_ref() {
            let is_match = compare_regex(left_val, &pattern);
            return if comp_op == CompOperatorPb::Like || comp_op == CompOperatorPb::Regexp {
                is_match
            } else {
                !is_match
            };
        } else {
            debug!(
                "Expected StringValue for LIKE comparison, got: {:?}",
                right_val.data
            );
            return false;
        }
    }
    // Should skip none for most comparisons, in particular Ne
    let ignore_none = left_val_none || right_val_none;
    if !ignore_none {
        result = match (left_val.data.as_ref(), right_val.data.as_ref()) {
            // String value comparisons
            (Some(Data::StringValue(s1)), Some(Data::StringValue(s2))) => s1.cmp(s2),
            (Some(Data::StringValue(_)), Some(_)) => Ordering::Greater,
            (Some(_), Some(Data::StringValue(_))) => Ordering::Less,
            // Decimal value comparisons
            (Some(Data::DecimalValue(d1)), Some(Data::DecimalValue(d2))) => {
                // Compare based on the actual decimal value
                let v1 = Decimal::new(d1.number, (d1.scale as u8).into());
                let v2 = Decimal::new(d2.number, (d2.scale as u8).into());
                v1.cmp(&v2)
            }
            (Some(Data::DecimalValue(d)), Some(Data::Int64Value(i))) => {
                let v1 = Decimal::new(d.number, (d.scale as u8).into());
                let v2 = Decimal::new(*i, 0);
                v1.cmp(&v2)
            }
            (Some(Data::Int64Value(i)), Some(Data::DecimalValue(d))) => {
                let v1 = Decimal::new(*i, 0);
                let v2 = Decimal::new(d.number, (d.scale as u8).into());
                v1.cmp(&v2)
            }
            // Int64 value comparisons
            (Some(Data::Int64Value(i1)), Some(Data::Int64Value(i2))) => i1.cmp(i2),
            (Some(Data::Int64Value(_)), Some(_)) => Ordering::Greater,
            (Some(_), Some(Data::Int64Value(_))) => Ordering::Less,
            // Bool value comparisons
            (Some(Data::BoolValue(b1)), Some(Data::BoolValue(b2))) => b1.cmp(b2),
            (Some(Data::BoolValue(_)), Some(_)) => Ordering::Greater,
            (Some(_), Some(Data::BoolValue(_))) => Ordering::Less,
            // null is considered less than any non-null value
            (Some(Data::NullValue(_)), Some(Data::NullValue(_))) => Ordering::Equal,
            (Some(Data::NullValue(_)), Some(_)) => Ordering::Less,
            (Some(_), Some(Data::NullValue(_))) => Ordering::Greater,
            // For simplicity, consider different types as not equal
            _ => {
                debug!(
                    "Unexpected comparison - left: {:?} right: {:?}",
                    left_val, right_val
                );
                return false;
            }
        }
    };
    let is_a_match = {
        if comp_op == CompOperatorPb::Ne || comp_op == CompOperatorPb::NotIn {
            result != Ordering::Equal
        } else if result == Ordering::Equal {
            comp_op == CompOperatorPb::Eq
                || comp_op == CompOperatorPb::In
                || comp_op == CompOperatorPb::Le
                || comp_op == CompOperatorPb::Ge
        } else if result == Ordering::Less {
            comp_op == CompOperatorPb::Lt
                || comp_op == CompOperatorPb::Le
                || comp_op == CompOperatorPb::Ne
                || comp_op == CompOperatorPb::NotIn
        } else {
            comp_op == CompOperatorPb::Gt
                || comp_op == CompOperatorPb::Ge
                || comp_op == CompOperatorPb::Ne
                || comp_op == CompOperatorPb::NotIn
        }
    };
    is_a_match
}

pub fn get_rel_target_pkvals(
    ctxt: &mut impl SqlExeTrait,
    target_id: u32,
    pk_segs: &Vec<String>,
) -> Option<Vec<SqlValuePb>> {
    let mut tgt_data_part: [u8; 32000] = [0; 32000];
    let mut tgt_data_size: c_uint = tgt_data_part.len() as c_uint;
    let mut dataset_id: u32 = 0;
    let sts = ctxt.get_datapart(
        ctxt.get_ltime(),
        target_id,
        &mut dataset_id,
        &mut tgt_data_part,
        &mut tgt_data_size,
    );
    if sts != STS_SUCCESS {
        debug!(
            "Failed to get datapart for target item ID 0x{:x}",
            target_id
        );
        return None;
    }
    let tgt_data_slice: &[u8] = &tgt_data_part[..tgt_data_size as usize];
    // Display the target content
    let sqlval = jbparse::jsonb_to_sqlvalue("$", &tgt_data_slice.to_vec());
    debug!("Target id 0x{:x} content : {:?}", target_id, sqlval);
    // Get sqlvalues from list of pk segments
    let mut sqlvals = vec![];
    for pkseg in pk_segs {
        let pk_sqlval = jbparse::jsonb_to_sqlvalue(&pkseg, &tgt_data_slice.to_vec());
        sqlvals.push(pk_sqlval);
    }
    debug!("Target id 0x{:x} values: {:?}", target_id, sqlvals);
    Some(sqlvals)
}

// Populate PK values for the element items into json_val
pub fn populate_rel_elts_pkvals(
    ctxt: &mut impl SqlExeTrait,
    rel_name: &String,
    succs: &Vec<u32>,
    pk_segs: &Vec<String>,
    json_val: &String,
) -> String {
    let mut result = json_val.clone();
    for succ_id in succs {
        // Get the pk values for the element target
        let vals = get_rel_target_pkvals(ctxt, *succ_id, pk_segs);
        if vals.is_none() {
            debug!(
                "Failed to get PK values for element target id 0x{:x} of rel '{}'",
                succ_id, rel_name
            );
            continue;
        }
        let sqlvals = vals.unwrap();

        // Convert into json
        let jsonval = jbparse::sqlvalues_to_summary_jsonvalue(sqlvals);
        // Insert into result map
        let new_res_str = jbparse::insert_summary_into_jsonstr(&result, &rel_name, &jsonval);
        debug!(
            "Rel: {} target id 0x{:x} result: {:?}",
            rel_name, succ_id, new_res_str
        );
        result = new_res_str;
    }
    result
}

// Add or delete rel elements for the primary item rels
pub fn update_rels(
    ctxt: &mut impl SqlExeTrait,
    upd_op: UpdateOpPb,
    inverse: bool,
    rel_id: u32,
    curr_id: u32,
    succs: &Vec<u32>,
) -> Result<String, SqlExecError> {
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size: c_uint = data_part.len() as c_uint;
    let mut set_id: c_uint = 0;

    // Verify target item exists
    let sts = ctxt.get_datapart(
        ctxt.get_ltime(),
        curr_id,
        &mut set_id,
        &mut data_part,
        &mut data_size,
    );
    debug!(
        "update_rels - get datapart for upd {:?} relid {}, target id {}, succs to update {:?} inverse: {} size: {}",
         upd_op, rel_id, curr_id, succs, inverse, data_size
    );
    if sts != STS_SUCCESS {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to get datapart for relationship update - sts: {}",
            sts
        )));
    }
    match upd_op {
        UpdateOpPb::Insert => {
            if let Err(e) = relpart::add_rel_elts(ctxt, rel_id, inverse, curr_id, succs) {
                return Err(SqlExecError::ExecutionError(e.to_string()));
            } else if !inverse {
                ctxt.increment_count(sqlexet::UpdCounter::AddElt(succs.len() as i32));
            }
        }
        UpdateOpPb::Delete => {
            if let Err(e) = relpart::rm_rel_elts(ctxt, rel_id, inverse, curr_id, succs) {
                return Err(SqlExecError::ExecutionError(e.to_string()));
            } else if !inverse {
                ctxt.increment_count(sqlexet::UpdCounter::RmElt(succs.len() as i32));
            }
        }
        _ => {
            return Err(SqlExecError::ExecutionError(format!(
                "Unsupported update operation: {:?}",
                upd_op
            )));
        }
    }
    Ok("".to_string())
}

pub fn delete_inv_rel_predecessors(
    ctxt: &mut impl SqlExeTrait,
    item_id: u32,
) -> Result<String, SqlExecError> {
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size: c_uint = data_part.len() as c_uint;
    let sts = ctxt.get_relpart(
        ctxt.get_ltime(),
        item_id,
        false, // direct rel
        &mut data_part,
        &mut data_size,
    );
    if sts == MTS_OBJNOTFOUND {
        // No rel part, so no predecessors, nothing to do
        debug!(
            "No rel part found for item ID {} during delete, assuming no predecessors",
            item_id
        );
        return Ok("".to_string());
    }
    if sts != STS_SUCCESS {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to get rel part for item ID {} during delete, sts: 0x{:x}",
            item_id, sts
        )));
    }
    // TBD - need to check sts errors besides obj not found
    let rels_data_slice: &[u8] = &data_part[..data_size as usize];
    let res = relpart::get_rels(&rels_data_slice.to_vec());
    if let Err(e) = res {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to parse rel part for item ID {} during delete: {}",
            item_id, e
        )));
    }
    let rels = res.unwrap();
    let mut inv_data_part: [u8; 32000] = [0; 32000];
    let mut inv_data_size: c_uint = data_part.len() as c_uint;
    let item_ids = vec![item_id];
    for (relid, succs) in rels {
        for succ in succs {
            debug!("Deleting rel ID: {}, element item: {}", relid, succ);
            // Fetch the rel part of the element items
            let sts = ctxt.get_relpart(
                ctxt.get_ltime(),
                succ, // just peek one element to get the rel part format, assuming same rel part format for all elements
                true, // inverse rel
                &mut inv_data_part,
                &mut inv_data_size,
            );
            if sts != STS_SUCCESS {
                return Err(SqlExecError::ExecutionError(format!(
                            "Failed to get inverse rel part for element item ID {} during delete, sts: 0x{:x}",
                            succ, sts
                        )));
            }
            // delete the where clause ids from the element inverse rel
            match relpart::rm_rel_elts(ctxt, relid, true, succ, &item_ids) {
                Ok(c) => {
                    debug!(
                        "Deleted {} inverse rel elements for element item ID {}",
                        c, succ
                    );
                }
                Err(e) => {
                    debug!("Failed to delete inverse rel element: {}", e);
                }
            }
        }
    }

    return Ok("".to_string());
}

pub fn get_sqlval_for_path(
    ctxt: &mut impl SqlExeTrait,
    phase: i32,
    item_id: u32,
    jbpath: &String,
) -> Result<SqlValuePb, SqlExecError> {
    let mut data_part: [u8; 32000] = [0; 32000];
    let mut data_size: c_uint = data_part.len() as c_uint;
    let mut dataset_id: c_uint = 0;
    let sts = ctxt.get_datapart(
        ctxt.get_ltime(),
        item_id,
        &mut dataset_id,
        &mut data_part,
        &mut data_size,
    );
    if sts != STS_SUCCESS {
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to get datapart for item ID {} and path '{}' , sts: 0x{:x}",
            item_id, jbpath, sts
        )));
    }
    let data_slice: &[u8] = &data_part[..data_size as usize];
    if item_id < MIN_USER_DATASET_ID {
        debug!("item ID: {} data_part: {:?}", item_id, data_slice);
        let res_val = SqlValuePb {
            is_constant: false,
            data: Some(Data::StringValue(format!(r#""item_id":{}"#, item_id))),
        };
        return Ok(res_val.clone());
    }
    // Special case for dataset desc
    if phase == EvalPhasePb::StrippedProjection as i32 {
        if let Some(stripped_data_slice) =
            jbparse::jsonb_to_schema_stripped_jsonb(&data_slice.to_vec())
        {
            let sqlval = jbparse::jsonb_to_sqlvalue(&jbpath, &stripped_data_slice.to_vec());
            return Ok(sqlval);
        }
    }
    let sqlval = jbparse::jsonb_to_sqlvalue(&jbpath, &data_slice.to_vec());
    Ok(sqlval)
}

pub fn describe_ds(ds_desc: &jbparse::DatasetDesc) -> String {
    let mut result = format!("CREATE DATASET {}", ds_desc.name);
    let mut defs: Vec<String> = Vec::new();

    for idx in &ds_desc.indexes {
        let segs = idx.segs.join(", ");
        let idx_type = idx.idx_type.to_ascii_lowercase();
        if idx_type == "pkey" || idx_type == "primary" {
            defs.push(format!("    PRIMARY KEY({})", segs));
        }
    }

    for idx in &ds_desc.indexes {
        if idx.idx_type.eq_ignore_ascii_case("unique") {
            let segs = idx.segs.join(", ");
            defs.push(format!("    UNIQUE INDEX {}({})", idx.name, segs));
        }
    }

    for idx in &ds_desc.indexes {
        let idx_type = idx.idx_type.to_ascii_lowercase();
        if idx_type != "pkey" && idx_type != "primary" && idx_type != "unique" {
            let segs = idx.segs.join(", ");
            defs.push(format!("    INDEX {}({})", idx.name, segs));
        }
    }

    for rel in &ds_desc.rels {
        defs.push(format!(
            "    RELATIONSHIP {}({})",
            rel.name, rel.tgt_dataset
        ));
    }

    if defs.is_empty() {
        result.push_str(";\n");
        return result;
    }

    result.push('\n');
    result.push_str(&defs.join("\n"));
    result.push_str(";\n");
    result
}

pub fn insert_value(
    ctxt: &mut impl SqlExeTrait,
    valrels: &ValRelsUpdPb,
    values: &Vec<RefCell<SqlValuePb>>,
    ds_id: MtOidT,
) -> Result<MtOidT, SqlExecError> {
    let valborrow = values[valrels.val_idx as usize].borrow();
    let value = valborrow.data.as_ref().unwrap();
    debug!("Inserting value {:?}: {:?}", valrels.val_idx, value);
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
    let mut obj_key: MtOidT = 0;
    let sts = ctxt.create_item(
        ctxt.get_tranid(),
        ds_id,
        &mut obj_key,
        &data_binary,
        data_size,
    );
    if sts != STS_SUCCESS {
        // If any error create_item does abort the transaction
        return Err(SqlExecError::ExecutionError(format!(
            "Failed to insert value {} into dataset '{:?}', sts=0x{:x}",
            val_str, ds_id, sts
        )));
    }
    return Ok(obj_key);
}

pub fn build_index_scan_keys(
    ranges: &Vec<RangePb>,
    values: &Vec<RefCell<SqlValuePb>>,
    in_val_offset: i32,
) -> (
    Vec<SqlValuePb>,
    Vec<CompOperatorPb>,
    Vec<SqlValuePb>,
    Vec<CompOperatorPb>,
) {
    let mut start_keys = vec![];
    let mut start_cmps = vec![];
    let mut end_keys = vec![];
    let mut end_cmps = vec![];
    for r in ranges {
        if r.lb_val_idx < 0 {
            continue;
        }
        let val = values[(r.lb_val_idx + in_val_offset) as usize].borrow();
        start_keys.push(val.clone());
        start_cmps.push(CompOperatorPb::try_from(r.lb_op).unwrap());
        if r.ub_val_idx < 0 {
            continue;
        }
        let val = values[r.ub_val_idx as usize].borrow();
        end_keys.push(val.clone());
        end_cmps.push(CompOperatorPb::try_from(r.ub_op).unwrap());
    }
    debug!("Index scan start keys: {:?}", start_keys);
    debug!("Index scan start cmps: {:?}", start_cmps);
    debug!("Index scan end keys: {:?}", end_keys);
    debug!("Index scan end cmps: {:?}", end_cmps);
    (start_keys, start_cmps, end_keys, end_cmps)
}

