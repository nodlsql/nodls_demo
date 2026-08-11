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

use jsonb::jsonpath;
use sqlinsts::SqlValuePb;

#[test]
fn test_jsonb() {
    let json_str = r#"
        {
            "name":"Fred",
            "phones":[
                {
                    "type":"home",
                    "number":3720453
                },
                {
                    "type": "work",
                    "number":5062051
                }
            ]
        }"#;

    // -1- Get Value from json string, then bytes vec, then Value from bytes vec

    // parse JSON string to jsonb value
    let jsonb_value = jsonb::parse_value(json_str.as_bytes()).unwrap();
    // encode jsonb value to jsonb binary value
    let jsonb_bytes = jsonb_value.to_vec();

    // y=Object({
    //   "name": String("Fred"),
    //   "phones": Array([Object({"number": UInt64(3720453), "type": String("home")}),
    //   Object({"number": UInt64(5062051), "type": String("work")})])
    // })
    let result_value = jsonb::from_slice(&jsonb_bytes).unwrap();
    println!("JSONB - value={:?}", &result_value);

    if let jsonb::Value::Object(obj) = &result_value {
        println!("JSONB - name={:?}", obj.get("name").unwrap());
        println!("JSONB - phones={:?}", obj.get("phones").unwrap());
        if let jsonb::Value::Array(phones) = obj.get("phones").unwrap() {
            for (i, phone) in phones.iter().enumerate() {
                // phone[0].type=String("home")
                // phone[0].number=UInt64(3720453)
                if let jsonb::Value::Object(phone_obj) = phone {
                    if let jsonb::Value::String(phone_type) = phone_obj.get("type").unwrap() {
                        println!("JSONB - phone[{}].type string={:?}", i, phone_type);
                    }
                    if let jsonb::Value::Number(phone_number) = phone_obj.get("number").unwrap() {
                        println!("JSONB - phone[{}].number number={:?}", i, phone_number);
                        if let Some(num) = phone_number.as_u64() {
                            println!("JSONB - phone[{}].number u64={}", i, num);
                        }
                    }
                }
            }
        }
    }

    // -2- Get OwnedJsonb and RawJsonb from json string

    let owned_jsonb = json_str.parse::<jsonb::OwnedJsonb>().unwrap();
    let raw_jsonb = owned_jsonb.as_raw();

    // -3- Select value by path

    let path_str = r#"$.name"#;
    let json_path = jsonpath::parse_json_path(path_str.as_bytes()).unwrap();
    // res = Ok(true)
    let res = raw_jsonb.path_exists(&json_path);
    println!("JSONB - path_exists={:?}", res);

    // res=Some(OwnedJsonb { data: [32, 0, 0, 0, 16, 0, 0, 4, 70, 114, 101, 100] })
    let res = raw_jsonb.select_value_by_path(&json_path).unwrap();
    println!("JSONB - select_value_by_path={:?}", res);

    // res = ="\"Fred\"";
    let res = res.unwrap().to_string();
    println!("JSONB - select_value_by_path to_string={:?}", res);
}

#[test]
fn test_path_to_jbpath() {
    let tests = vec![
        ("a", vec!["$".to_string(), "b".to_string()], "a.$.b"),
        ("", vec!["$".to_string(), "a".to_string()], "$.a"),
        ("", vec!["*".to_string(), "b".to_string()], "$.*.b"),
        ("", vec!["*".to_string()], "$"),
    ];
    for (path, jpath, expected) in tests {
        let jbpath = jbparse::path_to_jbpath(path, &jpath);
        assert_eq!(
            jbpath, expected,
            "Unexpected jbpath for path: {}, jpath: {:?}",
            path, jpath
        );
    }
}

#[test]
fn test_jbparse() {
    let json_data = r#"
        {
            "name":"Fred",
            "phones":[
                {
                    "type":"home",
                    "number":3720453
                },
                {
                    "type": "work",
                    "number":5062051
                }
            ]
        }"#;

    let tests = vec![
        "name",
        "phones[0].type",
        "phones[0].number",
        "phones[1].type",
        "phones[1].number",
        "phones[*].type",
        "phones[0, 1].number",
        // "phones?(@.type == \"home\")",         // -- no result
        "phones[*]?(@.number == 3720453)",
        "phones[0,1]?(@.number == 3720453)",
        "phones[*]?(@.type == \"home\")",
        // Not supported by jbparse yet
        //"phones[*]?(@.number > 12.45)",         // -- decimal number comparison
        //"phones..type",
        //"name|phones",
    ];
    for path in tests {
        let res = jbparse::jsonstr_to_jsonb(json_data);
        assert!(
            res.is_ok(),
            "Failed to convert path {} err: {:?}",
            path,
            res.err()
        );
        let jsonb_bytes = res.unwrap();
        let sql_value = jbparse::jsonb_to_sqlvalue(path, &jsonb_bytes);
        assert!(
            sql_value.data.is_some(),
            "Failed to get value for path {}",
            path
        );
        println!(
            "TEST JBPARSE - path: {} SQL Value: {:?}",
            path,
            sql_value.data.unwrap()
        );
    }
}

#[test]
fn test_sqlvalues_to_summary_jsonvalue() {
    // Single value
    let sql_value = SqlValuePb {
        is_constant: false,
        data: Some(sqlinsts::sql_value_pb::Data::StringValue(
            "Hello".to_string(),
        )),
    };
    let json_value = jbparse::sqlvalues_to_summary_jsonvalue(vec![sql_value.clone()]);
    println!(
        "TEST SQLVALUE TO JSON - SQL Value: {:?} JSON Value: {:?}",
        sql_value, json_value
    );
    assert_eq!(json_value, serde_json::json!("Hello"));

    // Multiple values
    let sql_value_int = SqlValuePb {
        is_constant: false,
        data: Some(sqlinsts::sql_value_pb::Data::Int64Value(123)),
    };
    let sql_values = vec![sql_value, sql_value_int];
    let json_values = jbparse::sqlvalues_to_summary_jsonvalue(sql_values.clone());
    println!(
        "TEST SQLVALUE TO JSON - SQL Values: {:?} JSON Values: {:?}",
        sql_values, json_values
    );
    assert_eq!(json_values, serde_json::json!("Hello 123"));

    // Insert single value into existing JSON string
    let json_value = serde_json::json!("value");
    let json_string = r#"{}"#;
    let updated_json_string = jbparse::insert_summary_into_jsonstr(
        &json_string.to_string(),
        &"key".to_string(),
        &json_value,
    );
    println!(
        "TEST INSERT INTO JSON STRING - Original JSON: {:?} Updated JSON: {:?}",
        json_string, updated_json_string
    );
    assert_eq!(updated_json_string, r#"{"key":"value"}"#);

    // Insert one more value into existing JSON string
    let json_value = serde_json::json!("other");
    let updated_json_string =
        jbparse::insert_summary_into_jsonstr(&updated_json_string, &"key".to_string(), &json_value);
    println!(
        "TEST INSERT INTO JSON STRING - Original JSON: {:?} Updated JSON: {:?}",
        json_string, updated_json_string
    );
    assert_eq!(updated_json_string, r#"{"key":["value","other"]}"#);

    // Insert array value into existing JSON string
    let json_string = r#"{}"#;
    let json_value = serde_json::json!(["a", "b"]);
    let updated_json_string = jbparse::insert_summary_into_jsonstr(
        &json_string.to_string(),
        &"key".to_string(),
        &json_value,
    );
    println!(
        "TEST INSERT INTO JSON STRING - Original JSON: {:?} Updated JSON: {:?}",
        json_string, updated_json_string
    );
    assert_eq!(updated_json_string, r#"{"key":["a","b"]}"#);

    // Insert one more array value into existing JSON string
    let json_value = serde_json::json!(["c", "d"]);
    let updated_json_string =
        jbparse::insert_summary_into_jsonstr(&updated_json_string, &"key".to_string(), &json_value);
    println!(
        "TEST INSERT INTO JSON STRING - Original JSON: {:?} Updated JSON: {:?}",
        json_string, updated_json_string
    );
    assert_eq!(updated_json_string, r#"{"key":[["a","b"],["c","d"]]}"#);
}

#[test]
fn test_sqlvalue_to_jsonvalue() {
    let sql_value = SqlValuePb {
        is_constant: false,
        data: Some(sqlinsts::sql_value_pb::Data::StringValue(
            "Hello".to_string(),
        )),
    };
    let json_value = jbparse::sqlvalue_to_jsonvalue(&sql_value);
    println!(
        "TEST SQLVALUE TO JSON - SQL Value: {:?} JSON Value: {:?}",
        sql_value, json_value
    );
    assert_eq!(json_value, serde_json::json!("Hello"));

    let sql_value_int = SqlValuePb {
        is_constant: false,
        data: Some(sqlinsts::sql_value_pb::Data::Int64Value(123)),
    };
    let json_value_int = jbparse::sqlvalue_to_jsonvalue(&sql_value_int);
    println!(
        "TEST SQLVALUE TO JSON - SQL Value: {:?} JSON Value: {:?}",
        sql_value_int, json_value_int
    );
    assert_eq!(json_value_int, serde_json::json!(123));
}

#[test]
fn test_update_jsonstr() {
    // Single field update
    let json_string = r#"{"name":"Alice","hobby":"reading"}"#;
    let key_path = vec!["name".to_string()];
    let value = SqlValuePb {
        is_constant: true,
        data: Some(sqlinsts::sql_value_pb::Data::StringValue("Bob".to_string())),
    };

    let drop_json_string = jbparse::drop_from_jsonstr(&json_string.to_string(), &key_path).unwrap();
    assert_eq!(drop_json_string, r#"{"hobby":"reading"}"#);
    let updated_json_string =
        jbparse::insert_into_jsonstr(&drop_json_string.to_string(), &key_path, &value).unwrap();
    assert_eq!(updated_json_string, r#"{"hobby":"reading","name":"Bob"}"#);

    // Object field update
    let json_string = r#"{"name":"Alice", "address":{"city":"NY", "zip":"10001"}}"#;
    let key_path = vec!["address".to_string()];
    let value = SqlValuePb {
        is_constant: true,
        data: Some(sqlinsts::sql_value_pb::Data::StringValue(
            r#"{"city":"LA", "zip":90001}"#.to_string(),
        )),
    };

    let drop_json_string = jbparse::drop_from_jsonstr(&json_string.to_string(), &key_path).unwrap();
    assert_eq!(drop_json_string, r#"{"name":"Alice"}"#);
    let updated_json_string =
        jbparse::insert_into_jsonstr(&drop_json_string.to_string(), &key_path, &value).unwrap();
    assert_eq!(
        updated_json_string,
        r#"{"name":"Alice","address":{"city":"LA","zip":90001}}"#
    );

    // Path field update
    let json_string = r#"{"name":"Alice", "address":{"city":"NY", "zip":"10001"}}"#;
    let value = SqlValuePb {
        is_constant: true,
        data: Some(sqlinsts::sql_value_pb::Data::Int64Value(10002)),
    };
    let key_path = vec!["address".to_string(), "zip".to_string()];

    let drop_json_string = jbparse::drop_from_jsonstr(&json_string.to_string(), &key_path).unwrap();
    assert_eq!(
        drop_json_string,
        r#"{"name":"Alice","address":{"city":"NY"}}"#
    );
    let updated_json_string =
        jbparse::insert_into_jsonstr(&drop_json_string.to_string(), &key_path, &value).unwrap();
    assert_eq!(
        updated_json_string,
        r#"{"name":"Alice","address":{"city":"NY","zip":10002}}"#
    );
}

#[test]
fn test_schema_stripped_to_sqlvalue() {
    let dataset = serde_json::json!({
        "_id": 1,
        "name": "Dataset1",
        "rels": [
            {
                "_id": 10,
                "name": "rs1",
                "tgt_dataset": "ds1",
            },
            {
                "_id": 20,
                "name": "rs2",
                "tgt_dataset": "ds2",
            }
        ],
        "indexes": [
            {
                "_id": 100,
                "name": "idx1",
                "idx_type": "Pkey",
                "segs": ["userid", "name"],
            }
        ]
    });
    // Convert to jsonb bytes
    let jsonb_bytes = jbparse::jsonstr_to_jsonb(&dataset.to_string()).unwrap();
    // Convert to SQL value
    let sql_value = jbparse::jsonb_to_schema_stripped_sqlvalue(&jsonb_bytes);
    println!(
        "TEST SCHEMA STRIPPED TO SQLVALUE - Dataset: {:?} SQL Value: {:?}",
        dataset, sql_value
    );
    match sql_value {
        Some(value) => match value.data.as_ref().unwrap() {
            sqlinsts::sql_value_pb::Data::StringValue(s) => {
                // Expect the SQL value to contain all fields of the dataset
                assert_eq!(s, "{\"name\":\"Dataset1\",\
    \"rels\":[{\"name\":\"rs1\",\"tgt_dataset\":\"ds1\"},{\"name\":\"rs2\",\"tgt_dataset\":\"ds2\"}],\
    \"indexes\":[{\"name\":\"idx1\",\"segs\":[\"userid\",\"name\"],\"idx_type\":\"Pkey\"}]}");
            }
            _ => panic!("Expected StringValue"),
        },
        None => panic!("Expected Some value"),
    };

    // Strip empty arrays
    let dataset = serde_json::json!({
        "_id": 1,
        "name": "Dataset1",
        "rels": [],
        "indexes": []
    });
    let jsonb_bytes = jbparse::jsonstr_to_jsonb(&dataset.to_string()).unwrap();
    let sql_value = jbparse::jsonb_to_schema_stripped_sqlvalue(&jsonb_bytes);
    println!(
        "TEST SCHEMA STRIPPED TO SQLVALUE - Dataset with empty arrays: {:?} SQL Value: {:?}",
        dataset, sql_value
    );
    let value = sql_value.unwrap();
    let value_str = match value.data.as_ref().unwrap() {
        sqlinsts::sql_value_pb::Data::StringValue(s) => s,
        _ => panic!("Expected StringValue"),
    };
    assert_eq!(value_str, "{\"name\":\"Dataset1\"}");
}

#[test]
fn test_schema_stripped_key() {
    let dataset = serde_json::json!({
        "_id": 1,
        "name": "Dataset1",
        "rels": [
            {
                "_id": 10,
                "name": "rs1",
                "tgt_dataset": "ds1",
            },
            {
                "_id": 20,
                "name": "rs2",
                "tgt_dataset": "ds2",
            }
        ],
        "indexes": [
            {
                "_id": 100,
                "name": "idx1",
                "idx_type": "Pkey",
                "segs": ["userid", "name"],
            }
        ]
    });
    // Convert to jsonb bytes
    let res = jbparse::drop_key_from_jsonstr(&dataset.to_string(), &"_id".to_string());
    println!(
        "TEST JSON STRIPPED TO SQLVALUE - dataset: {:?} res: {:?}",
        dataset, res
    );
    let expected = serde_json::json!({
        "name": "Dataset1",
        "rels": [
            {
                "name": "rs1",
                "tgt_dataset": "ds1",
            },
            {
                "name": "rs2",
                "tgt_dataset": "ds2",
            }
        ],
        "indexes": [
            {
                "name": "idx1",
                "idx_type": "Pkey",
                "segs": ["userid", "name"],
            }
        ]
    });
    // convert res to serde_json::Value
    let res_json: serde_json::Value = serde_json::from_str(&res).unwrap();
    assert_eq!(res_json, expected);
}

#[test]
fn test_numeric_values() {
    let json_data = r#"
        {
            "int_value": 123,
            "decimal_value": 3.14,
            "negative_int": -456,
            "negative_decimal": -2.718
        }"#;

    let jsonb_bytes = jbparse::jsonstr_to_jsonb(json_data).unwrap();

    let paths = vec![
        "int_value",
        "decimal_value",
        "negative_int",
        "negative_decimal",
    ];

    let expected_values = vec![
        sqlinsts::sql_value_pb::Data::Int64Value(123),
        sqlinsts::sql_value_pb::Data::DecimalValue(sqlinsts::DecimalValuePb {
            scale: 2,
            number: 314,
        }),
        sqlinsts::sql_value_pb::Data::Int64Value(-456),
        sqlinsts::sql_value_pb::Data::DecimalValue(sqlinsts::DecimalValuePb {
            scale: 3,
            number: -2718,
        }),
    ];

    for (i, path) in paths.iter().enumerate() {
        let sql_value = jbparse::jsonb_to_sqlvalue(path, &jsonb_bytes);
        let expected_value = &expected_values[i];
        assert_eq!(sql_value.data.as_ref().unwrap(), expected_value,);
    }
}
