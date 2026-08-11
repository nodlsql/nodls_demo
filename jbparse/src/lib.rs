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

use jsonb::{self, core::JsonbItemType, jsonpath, Number, RawJsonb};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlinsts::{sql_value_pb, DecimalValuePb, SqlValuePb};
use thiserror::Error;
use tracing::debug;

#[derive(Error, Debug)]
pub enum JsonParseError {
    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
}

// Dataset descriptor
#[derive(Serialize, Deserialize, Debug)]
pub struct DatasetDesc {
    pub _id: u32,
    pub name: String,
    pub rels: Vec<RelDesc>,
    pub indexes: Vec<IndexDesc>,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct RelDesc {
    pub _id: u32,
    pub name: String,
    pub tgt_dataset: String,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct IndexDesc {
    pub _id: u32,
    pub name: String,
    pub idx_type: String,
    pub segs: Vec<String>,
}

pub fn jbstring_to_schema_stripped_sqlvalue(rjb: &RawJsonb<'_>) -> SqlValuePb {
    let res_str = rjb.to_string();
    // Strip '_id' fields from the value

    debug!("JBPARSE STRING - result string value={:?}", res_str);
    // Strip quotes if the result is a string
    let strip_str;
    let res_str = if res_str.starts_with('"') && res_str.ends_with('"') {
        &res_str[1..res_str.len() - 1]
    } else {
        strip_str = drop_key_from_jsonstr(&res_str, &"_id".to_string());
        &strip_str
    };
    SqlValuePb {
        is_constant: false,
        data: Some(sql_value_pb::Data::StringValue(res_str.to_string())),
    }
}

pub fn jbstring_to_sqlvalue(rjb: &RawJsonb<'_>) -> SqlValuePb {
    let res_str = rjb.to_string();
    debug!("JBPARSE STRING - result string value={:?}", res_str);
    // Strip quotes if the result is a string
    let res_str = if res_str.starts_with('"') && res_str.ends_with('"') {
        &res_str[1..res_str.len() - 1]
    } else {
        &res_str
    };
    SqlValuePb {
        is_constant: false,
        data: Some(sql_value_pb::Data::StringValue(res_str.to_string())),
    }
}

pub fn jbnumber_to_sqlvalue(rjb: &RawJsonb<'_>) -> SqlValuePb {
    let res_str = rjb.as_number();
    let mut sqlval = SqlValuePb {
        is_constant: false,
        data: None,
    };
    match res_str {
        Ok(num_option) => {
            debug!("JBPARSE NUMBER - result number option={:?}", num_option);
            match num_option {
                Some(number) => match number {
                    Number::Int64(i) => {
                        debug!("JBPARSE NUMBER - Int64 value={}", i);
                        sqlval = SqlValuePb {
                            is_constant: false,
                            data: Some(sql_value_pb::Data::Int64Value(number.as_i64().unwrap())),
                        }
                    }
                    Number::UInt64(u) => {
                        debug!("JBPARSE NUMBER - UInt64 value={}", u);
                        sqlval = SqlValuePb {
                            is_constant: false,
                            data: Some(sql_value_pb::Data::Int64Value(number.as_i64().unwrap())),
                        }
                    }
                    Number::Decimal64(d) => {
                        debug!("JBPARSE NUMBER - Decimal64 value={:?}", d);
                        sqlval = SqlValuePb {
                            is_constant: false,
                            data: Some(sql_value_pb::Data::DecimalValue(DecimalValuePb {
                                scale: d.scale as u32,
                                number: d.value,
                            })),
                        }
                    }
                    // Not supported by jsonb
                    Number::Float64(f) => {
                        debug!("JBPARSE NUMBER - Float64 value={:?}", f);
                    }
                    _ => debug!("JBPARSE NUMBER - other type: {:?}", number),
                },
                _ => debug!("JBPARSE NUMBER - value is None"),
            }
        }
        Err(e) => {
            debug!("JBPARSE NUMBER - error getting value: {:?}", e);
        }
    }
    sqlval
}

pub fn jsonb_to_dataset_desc(jsonb_bytes: &Vec<u8>) -> Option<DatasetDesc> {
    let res_val = match jsonb::from_slice(jsonb_bytes) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to decode dataset descriptor jsonb - {}", e);
            return None;
        }
    };
    let dataset_desc: DatasetDesc = match serde_json::from_str(&res_val.to_string()) {
        Ok(v) => v,
        Err(e) => {
            debug!("Failed to decode dataset descriptor - {}", e);
            return None;
        }
    };
    Some(dataset_desc)
}

pub fn jsonb_to_schema_stripped_sqlvalue(jsonb_bytes: &Vec<u8>) -> Option<SqlValuePb> {
    let dataset_desc = match jsonb_to_dataset_desc(jsonb_bytes) {
        Some(v) => v,
        None => return None,
    };
    // Create json value with:
    // id fields stripped out and empty array fields stripped out
    let mut json_desc = json!({
        "name": dataset_desc.name,
    });
    if !dataset_desc.rels.is_empty() {
        json_desc["rels"] = json!([]);
        let json_desc_arr = json_desc["rels"].as_array_mut().unwrap();
        for rel in dataset_desc.rels {
            // add RelDesc to json with id stripped out
            let rel_json = json!({
                "name": rel.name,
                "tgt_dataset": rel.tgt_dataset,
            });
            // append to rels array in json_desc
            json_desc_arr.push(rel_json);
        }
    }
    // same thing for indexes
    if !dataset_desc.indexes.is_empty() {
        json_desc["indexes"] = json!([]);
        let json_desc_arr = json_desc["indexes"].as_array_mut().unwrap();
        for index in dataset_desc.indexes {
            let mut index_json = json!({
                "name": index.name,
                "segs": index.segs,
            });
            if index.idx_type != "default" {
                index_json = json!({
                    "name": index.name,
                    "segs": index.segs,
                    "idx_type": index.idx_type,
                });
            }
            // append to indexes array in json_desc
            json_desc_arr.push(index_json);
        }
    }
    // For simplicity, convert the stripped jsonb to string and return as sqlvalue
    let res_str = json_desc.to_string();
    debug!("JBPARSE - stripped jsonb string value={:?}", res_str);
    Some(SqlValuePb {
        is_constant: false,
        data: Some(sql_value_pb::Data::StringValue(res_str)),
    })
}

pub fn path_to_jbpath(
    path: &str, // path segments before jsonpath specific element shows up, e.g. "a.b.c"
    jpath: &Vec<String>, // path after jsonpath specific element like '$', or dual use '*'
) -> String {
    let jbpath;
    if jpath.is_empty() {
        if path.len() == 0 {
            // If path is empty string, treat it as '$'. This can happen when only segment is a rel name.
            jbpath = "$".to_string();
        } else {
            jbpath = format!(r#"$.{}"#, path);
        }
    } else {
        // if jpath non empty there is some '$' root element, implement jsonpath logic
        if path == "" {
            // If path is just '$', then we want to select the whole jsonb, so use '$' as the path
            if jpath.len() == 1 && jpath[0] == "*" {
                jbpath = "$".to_string();
            } else {
                if jpath[0] == "$" {
                    jbpath = format!(r#"{}"#, jpath.join("."));
                } else {
                    jbpath = format!(r#"$.{}"#, jpath.join("."));
                }
            }
        } else {
            if jpath[0] == "$" {
                jbpath = format!(r#"{}.{}"#, path, jpath.join("."));
            } else {
                jbpath = format!(r#"$.{}.{}"#, path, jpath.join("."));
            }
        }
    }
    debug!(
        "JBPARSE - path: {} jpath: {:?} jbpath: {} ",
        path, jpath, jbpath
    );
    jbpath
}

pub fn check_json_path(jbpath: &str) -> Result<(), String> {
    let res = jsonpath::parse_json_path(jbpath.as_bytes());
    match res {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Invalid jsonpath '{}': {:?}", jbpath, e)),
    }
}

// jpath is already integrated in the path here, we pass it to figure out how to handle special cases like '*' and '$' root.
pub fn jsonb_to_sqlvalue(jbpath: &str, jsonb_bytes: &Vec<u8>) -> SqlValuePb {
    // Construct path
    let json_path = jsonpath::parse_json_path(jbpath.as_bytes()).unwrap();

    // Get raw jsonb
    let owned_jsonb = jsonb::OwnedJsonb::new(jsonb_bytes.clone());
    let raw_jsonb = owned_jsonb.as_raw();
    let mut sqlval = SqlValuePb {
        is_constant: false,
        data: None,
    };
    match raw_jsonb.path_exists(&json_path).unwrap() {
        true => {
            debug!("JBPARSE - path_exists={:?}", json_path);
            let res = raw_jsonb.select_value_by_path(&json_path).unwrap();
            match &res {
                Some(v) => {
                    debug!("JBPARSE - select_value_by_path={:?}", v);
                    let rv = v.as_raw();
                    match rv.jsonb_item_type() {
                        Err(e) => debug!("JBPARSE - result type error: {:?}", e),
                        Ok(jt) => {
                            debug!("JBPARSE - result success type={:?}", jt);
                            match jt {
                                JsonbItemType::Null => {
                                    debug!("JBPARSE - result is NULL");
                                    sqlval = SqlValuePb {
                                        is_constant: false,
                                        data: Some(sql_value_pb::Data::NullValue(true)),
                                    };
                                }
                                JsonbItemType::Number => {
                                    debug!("JBPARSE - result is NUMBER: num={:?}", rv);
                                    sqlval = jbnumber_to_sqlvalue(&rv);
                                }
                                JsonbItemType::String => {
                                    debug!("JBPARSE - result is STRING");
                                    sqlval = jbstring_to_sqlvalue(&rv);
                                }
                                JsonbItemType::Boolean => {
                                    debug!("JBPARSE - result is BOOLEAN");
                                    sqlval = SqlValuePb {
                                        is_constant: false,
                                        data: Some(sql_value_pb::Data::BoolValue(
                                            rv.as_bool().unwrap().unwrap(),
                                        )),
                                    };
                                }
                                JsonbItemType::Array(_) => {
                                    debug!("JBPARSE - result is ARRAY");
                                    sqlval = jbstring_to_sqlvalue(&rv);
                                }
                                JsonbItemType::Object(_) => {
                                    debug!("JBPARSE - result is OBJECT");
                                    sqlval = jbstring_to_sqlvalue(&rv);
                                }
                                _ => debug!("JBPARSE - result is OTHER"),
                            }
                        }
                    }
                }
                None => debug!("JBPARSE - select_value_by_path=None"),
            };
        }
        false => {
            // Entity doesn't contain the specified path
            debug!("JBPARSE - path does not exist");
        }
    }
    sqlval
}

pub fn jsonstr_to_jsonb(json_str: &str) -> Result<Vec<u8>, JsonParseError> {
    // Get json value as object, if not return error
    let res_val = serde_json::from_str::<serde_json::Value>(json_str);
    match res_val {
        Ok(v) => {
            if !v.is_object() {
                return Err(JsonParseError::InvalidJson(
                    "JSON value is not an object".to_string(),
                ));
            }
        }
        Err(e) => {
            println!("Failed to parse JSON string: {:?}", e);
            return Err(JsonParseError::InvalidJson(e.to_string()));
        }
    }

    let res = json_str.parse::<jsonb::OwnedJsonb>();
    match res {
        Ok(jsonb) => {
            let jsonb_bytes = jsonb.to_vec();
            Ok(jsonb_bytes)
        }
        Err(e) => return Err(JsonParseError::InvalidJson(e.to_string())),
    }
}

pub fn sqlvalues_to_summary_jsonvalue(sqlvals: Vec<SqlValuePb>) -> serde_json::Value {
    if sqlvals.len() == 1 {
        let value = match &sqlvals[0].data {
            Some(sql_value_pb::Data::Int64Value(v)) => json!(v),
            Some(sql_value_pb::Data::StringValue(v)) => json!(v),
            Some(sql_value_pb::Data::BoolValue(v)) => json!(v),
            Some(sql_value_pb::Data::NullValue(_)) => json!(null),
            _ => json!(null),
        };
        // If single value, return it directly
        return value;
    } else {
        // if multiple values, make it a string, append with space separator
        let mut s = String::new();
        for (i, sqlval) in sqlvals.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            // Convert value to string and append to s
            let val_str = match &sqlval.data {
                Some(sql_value_pb::Data::Int64Value(v)) => v.to_string(),
                Some(sql_value_pb::Data::StringValue(v)) => v.clone(),
                Some(sql_value_pb::Data::BoolValue(v)) => v.to_string(),
                Some(sql_value_pb::Data::NullValue(_)) => "null".to_string(),
                _ => "null".to_string(),
            };
            s.push_str(&val_str);
        }
        return json!(s);
    }
}

pub fn sqlvalue_to_jsonvalue(sqlval: &SqlValuePb) -> serde_json::Value {
    match &sqlval.data {
        Some(sql_value_pb::Data::Int64Value(v)) => json!(v),
        Some(sql_value_pb::Data::StringValue(v)) => {
            // try to parse object from value string, if fails, insert as string
            if let Ok(obj) = serde_json::from_str::<serde_json::Value>(v.as_str()) {
                obj
            } else {
                json!(v)
            }
        }
        Some(sql_value_pb::Data::BoolValue(v)) => json!(v),
        Some(sql_value_pb::Data::NullValue(_)) => json!(null),
        _ => json!(null),
    }
}

// Insert rel summary value into JSON string. If key already exists, convert to array and append new value.
// If new value is an array, merge with existing value should produce an array of arrays.
pub fn insert_summary_into_jsonstr(
    jsonstr: &String,
    keyname: &String,
    value: &serde_json::Value,
) -> String {
    let mut json_value: serde_json::Value = serde_json::from_str(jsonstr).unwrap();
    match json_value.get_mut(keyname) {
        Some(existing_value) => {
            let mut new_values;
            if value.is_array() {
                // if fist element in existing value is already an array, merge with new array
                if existing_value.is_array()
                    && existing_value.as_array().unwrap().len() > 0
                    && existing_value.as_array().unwrap()[0].is_array()
                {
                    let mut merged_array = existing_value.as_array().unwrap().clone();
                    merged_array.extend(value.as_array().unwrap().clone());
                    new_values = serde_json::Value::Array(merged_array);
                } else {
                    // Existing value is not an array of arrays, convert to array of arrays
                    new_values =
                        serde_json::Value::Array(vec![existing_value.clone(), value.clone()]);
                }
            } else {
                if !existing_value.is_array() {
                    // Both existing and new value are not arrays, convert to array
                    new_values =
                        serde_json::Value::Array(vec![existing_value.clone(), value.clone()]);
                } else {
                    // Existing value is an array, append new value to it
                    new_values = existing_value.clone();
                    if let Some(arr) = new_values.as_array_mut() {
                        arr.push(value.clone());
                    }
                }
            }
            json_value[keyname] = new_values;
        }
        None => {
            // Key doesn't exist, insert new value
            json_value[keyname] = value.clone();
        }
    }
    json_value.to_string()
}

pub fn insert_into_jsonstr(
    jsonstr: &String,
    keypath: &Vec<String>,
    value: &SqlValuePb,
) -> Result<String, serde_json::Error> {
    let res = serde_json::from_str(jsonstr);
    if let Err(e) = res {
        return Err(e);
    }
    let mut json_value: serde_json::Value = res.unwrap();
    let mut json_value_ref = &mut json_value;
    let mut keycount = 0;
    for key in keypath {
        if json_value_ref.get(key).is_none() {
            if keycount != keypath.len() - 1 {
                // Noop, we only insert at the end of the path
                return Ok(json_value.to_string());
            }
            let new_value = sqlvalue_to_jsonvalue(value);
            json_value_ref[key] = new_value;
        } else {
            // Navigate to the next level of the path
            json_value_ref = json_value_ref.get_mut(key).unwrap();
        }
        keycount += 1;
    }
    Ok(json_value.to_string())
}

// Drop a field from JSON string
pub fn drop_from_jsonstr(
    jsonstr: &String,
    pathsegs: &Vec<String>,
) -> Result<String, serde_json::Error> {
    let res = serde_json::from_str(jsonstr);
    if let Err(e) = res {
        return Err(e);
    }
    let mut json_value: serde_json::Value = res.unwrap();
    let mut json_value_ref = &mut json_value;
    let mut key_count = 0;
    for key in pathsegs {
        if json_value_ref.get(key).is_none() {
            // Noop, path doesn't exist
            return Ok(json_value.to_string());
        }
        if key_count == pathsegs.len() - 1 {
            // Last key in the path, remove it
            json_value_ref.as_object_mut().unwrap().remove(key);
            return Ok(json_value.to_string());
        }
        // Navigate to the next level of the path
        json_value_ref = json_value_ref.get_mut(key).unwrap();
        key_count += 1;
    }
    // Not found, return original string
    Ok(json_value.to_string())
}

// Recursively drop a key from JSON string, if it exists at any level
pub fn drop_key_from_jsonstr(jsonstr: &String, keyname: &String) -> String {
    let res = serde_json::from_str(jsonstr);
    if let Err(_e) = res {
        // no op if it doesn't parse
        return jsonstr.clone();
    }
    let mut json_value: serde_json::Value = res.unwrap();
    drop_key_recursive(&mut json_value, keyname);
    json_value.to_string()
}

pub fn drop_key_recursive(json_value: &mut serde_json::Value, keyname: &String) {
    match json_value {
        serde_json::Value::Object(map) => {
            map.remove(keyname);
            for (_, v) in map.iter_mut() {
                drop_key_recursive(v, keyname);
            }
        }
        serde_json::Value::Array(arr) => {
            for v in arr.iter_mut() {
                drop_key_recursive(v, keyname);
            }
        }
        _ => {}
    }
}
