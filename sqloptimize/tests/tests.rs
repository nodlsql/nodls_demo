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

use sqlinsts::{
    sql_value_pb::Data, CompOperatorPb, EvalPhasePb, IComparePb, IDatapathPb, IIndexPb, IndexOpPb,
    IndexSegPb, IndexTypePb, SqlValuePb,
};

use sqloptimize::analyze::compute_index_range_for_index;
use sqloptimize::iutils;
use sqloptimize::utils::{
    compute_index_range_for_datapath, path_matches, DatapathAnalyzer, IndexAnalyzer, RelAnalyzer,
};

#[test]
fn test_compute_index_range_for_datapath() {
    // Create constant sql values
    let sqlvals = vec![
        // Dummy, this is the datapath idx_key_val position
        SqlValuePb {
            is_constant: false,
            data: Some(Data::OidValue(0)),
        },
        // datapath value at idx 1
        SqlValuePb {
            is_constant: true,
            data: Some(Data::Int64Value(11)),
        },
        // constant value at idx 2
        SqlValuePb {
            is_constant: true,
            data: Some(Data::Int64Value(22)),
        },
        // constant value at idx 3
        SqlValuePb {
            is_constant: true,
            data: Some(Data::Int64Value(33)),
        },
    ];
    // Create datapath instruction
    let dpth_inst = IDatapathPb {
        phase: EvalPhasePb::Predicate as i32,
        ds_name: "test_dataset".to_string(),
        alias: "".to_string(),
        parent_path: vec!["test_dataset".to_string()],
        pathsegs: vec!["a".to_string()],
        jsonpath: vec![],
        path_str: "a".to_string(),
        invrels: vec![],
        rel_descs: vec![],
        key_val_idx: 0,
        val_idx: 1,
    };
    let segments = vec!["a".to_string(), "c".to_string()];
    // Create equi comparison: a = 22
    let comp_insts = vec![IComparePb {
        comp: CompOperatorPb::Eq as i32,
        left_val_idx: 1,  // Datapath substituted at val_idx 1
        right_val_idx: 2, // Constant at val_idx 2
        right_val_cnt: 1,
    }];
    // Expected:
    // RangePb { lower_bound_val_idx: 2, lower_op: Eq, upper_bound_val_idx: 2, upper_op: Eq }
    let range_opt = compute_index_range_for_datapath(
        &segments[0],
        &dpth_inst,
        &dpth_inst.path_str,
        &comp_insts,
        &sqlvals,
    );
    println!("Computed equi range: {:?}", range_opt);
    assert!(range_opt.is_some());
    let range = range_opt.unwrap();
    assert_eq!(range.lower_bound_val_idx, 2);
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);

    // a > 22 AND a < 33
    let comp_insts = vec![
        IComparePb {
            comp: CompOperatorPb::Gt as i32,
            left_val_idx: 1,  // Matches datapath val_idx
            right_val_idx: 2, // Matches sqlval at index 2
            right_val_cnt: 1,
        },
        IComparePb {
            comp: CompOperatorPb::Gt as i32,
            left_val_idx: 3,  // Matches sqlval at index 2
            right_val_idx: 1, // Matches datapath val_idx
            right_val_cnt: 1,
        },
    ];
    // Expected:
    // RangePb { lower_bound_val_idx: -1, lower_op: Eq, upper_bound_val_idx: 2, upper_op: Lt }
    let range_opt = compute_index_range_for_datapath(
        &segments[0],
        &dpth_inst,
        &dpth_inst.path_str,
        &comp_insts,
        &sqlvals,
    );
    println!("Computed bounded range: {:?}", range_opt);
    assert!(range_opt.is_some());
    let range = range_opt.unwrap();
    assert_eq!(range.lower_bound_val_idx, 2);
    assert_eq!(range.upper_bound_val_idx, 3);
    assert_eq!(range.lower_op, CompOperatorPb::Gt as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Lt as i32);

    let comp_insts = vec![
        IComparePb {
            comp: CompOperatorPb::Gt as i32,
            left_val_idx: 1,  // Matches datapath val_idx
            right_val_idx: 3, // Matches sqlval at index 1
            right_val_cnt: 1,
        },
        IComparePb {
            comp: CompOperatorPb::Eq as i32,
            left_val_idx: 1,  // Matches datapath val_idx
            right_val_idx: 2, // Matches sqlval at index 2
            right_val_cnt: 1,
        },
        IComparePb {
            comp: CompOperatorPb::Lt as i32,
            left_val_idx: 3,  // Matches sqlval at index 2
            right_val_idx: 1, // Matches datapath val_idx
            right_val_cnt: 1,
        },
    ];
    // Expected:
    // { lower_bound_val_idx: 2, lower_op: Eq, upper_bound_val_idx: -1, upper_op: Eq }
    let range_opt = compute_index_range_for_datapath(
        &segments[0],
        &dpth_inst,
        &dpth_inst.path_str,
        &comp_insts,
        &sqlvals,
    );
    println!("Computed equi comparison range: {:?}", range_opt);
    assert!(range_opt.is_some());
    let range = range_opt.unwrap();
    // Verify equi match supersedes other predicates
    assert_eq!(range.lower_bound_val_idx, 2);
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);

    // Segment doesn't match datapath path, should return None
    let range_opt = compute_index_range_for_datapath(
        &segments[1],
        &dpth_inst,
        &dpth_inst.path_str,
        &comp_insts,
        &sqlvals,
    );
    println!("Computed range for non matching segment: {:?}", range_opt);
    assert!(range_opt.is_none());

    let comp_insts = vec![
        IComparePb {
            comp: CompOperatorPb::In as i32,
            left_val_idx: 1,  // Datapath substituted at val_idx 1
            right_val_idx: 2, // Constant at val_idx 2
            right_val_cnt: 3,
        },
        IComparePb {
            comp: CompOperatorPb::Lt as i32,
            left_val_idx: 1,  // Same datapath val_idx, should be ignored
            right_val_idx: 3, // Constant at val_idx 3
            right_val_cnt: 1,
        },
    ];
    // Expected:
    // { lower_bound_val_idx: 2, lower_op: Eq, upper_bound_val_idx: -1, upper_op: Eq }
    let range_opt = compute_index_range_for_datapath(
        &segments[0],
        &dpth_inst,
        &dpth_inst.path_str,
        &comp_insts,
        &sqlvals,
    );
    println!("Computed IN comparison range: {:?}", range_opt);
    assert!(range_opt.is_some());
    let range = range_opt.unwrap();
    // Verify equi match supersedes other predicates
    assert_eq!(range.lower_bound_val_idx, 2);
    assert_eq!(range.lower_bound_nb_vals, 3);
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::In as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);
}

#[test]
pub fn test_compute_index_range_for_index() {
    // 1 - missing first segment but 2nd or more segments there, for now it should not
    // optimize.
    // TBD - Expect later to implement full index scan.
    let dpth_analyzers = vec![
        DatapathAnalyzer {
            datapath: IDatapathPb {
                phase: EvalPhasePb::Predicate as i32,
                ds_name: "test_dataset".to_string(),
                parent_path: vec!["test_dataset".to_string()],
                key_val_idx: 0,
                val_idx: 1,
                pathsegs: vec!["b".to_string()],
                jsonpath: vec![],
                path_str: "b".to_string(),
                rel_descs: vec![],
                alias: "".to_string(),
                invrels: vec![],
            },
            comparisons: vec![IComparePb {
                comp: CompOperatorPb::Eq as i32,
                left_val_idx: 1,  // Matches datapath val_idx
                right_val_idx: 2, // Matches constant at index 2
                right_val_cnt: 1,
            }],
        },
        DatapathAnalyzer {
            datapath: IDatapathPb {
                phase: EvalPhasePb::Predicate as i32,
                ds_name: "test_dataset".to_string(),
                parent_path: vec!["test_dataset".to_string()],
                key_val_idx: 3,
                val_idx: 4,
                pathsegs: vec!["c".to_string()],
                jsonpath: vec![],
                invrels: vec![],
                path_str: "c".to_string(),
                rel_descs: vec![],
                alias: "".to_string(),
            },
            comparisons: vec![IComparePb {
                comp: CompOperatorPb::Eq as i32,
                left_val_idx: 4,  // Matches datapath val_idx
                right_val_idx: 5, // Matches constant at index 5
                right_val_cnt: 1,
            }],
        },
    ];
    let sqlvals = vec![
        // datapath idx_key_val at idx 0
        SqlValuePb {
            is_constant: false,
            data: Some(Data::OidValue(0)),
        },
        // datapath value at idx 1
        SqlValuePb {
            is_constant: false,
            data: Some(Data::Int64Value(11)),
        },
        // constant value at idx 2
        SqlValuePb {
            is_constant: true,
            data: Some(Data::Int64Value(22)),
        },
        // datapath idx_key_val at idx 3
        SqlValuePb {
            is_constant: false,
            data: Some(Data::OidValue(3)),
        },
        // datapath value at idx 4
        SqlValuePb {
            is_constant: false,
            data: Some(Data::Int64Value(44)),
        },
        // constant value at idx 5
        SqlValuePb {
            is_constant: true,
            data: Some(Data::Int64Value(55)),
        },
    ];
    let iindex = IIndexPb {
        idx_type: IndexTypePb::Pkey.into(),
        op: IndexOpPb::Scan as i32,
        ds_name: "test_dataset".to_string(),
        name: "test_index".to_string(),
        seg_strs: vec!["a".to_string(), "b".to_string()],
        seg_vecs: vec![
            IndexSegPb {
                seg_vec: vec!["a".to_string()],
            },
            IndexSegPb {
                seg_vec: vec!["b".to_string()],
            },
        ],
        root_id: 0,
        key_val_idx: 0,
        range: None,
    };
    let ranges = compute_index_range_for_index(&sqlvals, &iindex, &dpth_analyzers);
    println!("Computed range for missing first segment:\n{:?}", ranges);
    assert!(ranges.is_empty());

    // 2 - first segment matches, 2nd segment doesn't match, should return range for first segment
    let iindex = IIndexPb {
        idx_type: IndexTypePb::Pkey.into(),
        op: IndexOpPb::Scan as i32,
        ds_name: "test_dataset".to_string(),
        name: "test_index".to_string(),
        seg_strs: vec!["b".to_string(), "a".to_string()],
        seg_vecs: vec![
            IndexSegPb {
                seg_vec: vec!["b".to_string()],
            },
            IndexSegPb {
                seg_vec: vec!["a".to_string()],
            },
        ],
        root_id: 0,
        key_val_idx: 0,
        range: None,
    };
    let ranges = compute_index_range_for_index(&sqlvals, &iindex, &dpth_analyzers);
    println!("Computed range for first segment match only:\n{:?}", ranges);
    assert!(!ranges.is_empty());
    assert_eq!(ranges.len(), 1);
    let range = &ranges[0];
    assert_eq!(range.lower_bound_val_idx, 2);
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);

    // 3 - both segments match
    let iindex = IIndexPb {
        idx_type: IndexTypePb::Pkey.into(),
        op: IndexOpPb::Scan as i32,
        ds_name: "test_dataset".to_string(),
        name: "test_index".to_string(),
        seg_strs: vec!["b".to_string(), "c".to_string()],
        seg_vecs: vec![
            IndexSegPb {
                seg_vec: vec!["b".to_string()],
            },
            IndexSegPb {
                seg_vec: vec!["c".to_string()],
            },
        ],
        root_id: 0,
        key_val_idx: 0,
        range: None,
    };
    let ranges = compute_index_range_for_index(&sqlvals, &iindex, &dpth_analyzers);
    println!("Computed range for both segments match:\n{:?}", ranges);
    assert!(!ranges.is_empty());
    assert_eq!(ranges.len(), 2);
    let range = &ranges[0];
    assert_eq!(range.lower_bound_val_idx, 2); // Same as before
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);
    let range = &ranges[1];
    assert_eq!(range.lower_bound_val_idx, 5);
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);

    // 4 - present the datapath the other way round
    let dpth_analyzers = vec![
        DatapathAnalyzer {
            datapath: IDatapathPb {
                phase: EvalPhasePb::Predicate as i32,
                ds_name: "test_dataset".to_string(),
                key_val_idx: 3,
                val_idx: 4,
                pathsegs: vec!["c".to_string()],
                jsonpath: vec![],
                path_str: "c".to_string(),
                rel_descs: vec![],
                invrels: vec![],
                parent_path: vec!["test_dataset".to_string()],
                alias: "".to_string(),
            },
            comparisons: vec![IComparePb {
                comp: CompOperatorPb::Eq as i32,
                left_val_idx: 4,  // Matches datapath val_idx
                right_val_idx: 5, // Matches constant at index 5
                right_val_cnt: 1,
            }],
        },
        DatapathAnalyzer {
            datapath: IDatapathPb {
                phase: EvalPhasePb::Predicate as i32,
                ds_name: "test_dataset".to_string(),
                key_val_idx: 0,
                val_idx: 1,
                pathsegs: vec!["b".to_string()],
                jsonpath: vec![],
                path_str: "b".to_string(),
                rel_descs: vec![],
                invrels: vec![],
                parent_path: vec!["test_dataset".to_string()],
                alias: "".to_string(),
            },
            comparisons: vec![IComparePb {
                comp: CompOperatorPb::Eq as i32,
                left_val_idx: 1,  // Matches datapath val_idx
                right_val_idx: 2, // Matches constant at index 2
                right_val_cnt: 1,
            }],
        },
    ];
    let ranges = compute_index_range_for_index(&sqlvals, &iindex, &dpth_analyzers);
    println!(
        "Computed range for both segments match, reverse order of datapath:\n{:?}",
        ranges
    );
    assert!(!ranges.is_empty());
    assert_eq!(ranges.len(), 2);
    let range = &ranges[0];
    assert_eq!(range.lower_bound_val_idx, 2); // Same as before
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);
    let range = &ranges[1];
    assert_eq!(range.lower_bound_val_idx, 5);
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);

    // 5 - first segment with GT comparison, 2nd segment with EQ comparison, should return range for first segment only
    let dpth_analyzers = vec![
        DatapathAnalyzer {
            datapath: IDatapathPb {
                phase: EvalPhasePb::Predicate as i32,
                ds_name: "test_dataset".to_string(),
                key_val_idx: 3,
                val_idx: 4,
                pathsegs: vec!["c".to_string()],
                jsonpath: vec![],
                path_str: "c".to_string(),
                rel_descs: vec![],
                invrels: vec![],
                parent_path: vec!["test_dataset".to_string()],
                alias: "".to_string(),
            },
            comparisons: vec![IComparePb {
                comp: CompOperatorPb::Eq as i32,
                left_val_idx: 4,  // Matches datapath val_idx
                right_val_idx: 5, // Matches constant at index 5
                right_val_cnt: 1,
            }],
        },
        DatapathAnalyzer {
            datapath: IDatapathPb {
                phase: EvalPhasePb::Predicate as i32,
                ds_name: "test_dataset".to_string(),
                key_val_idx: 0,
                val_idx: 1,
                pathsegs: vec!["b".to_string()],
                jsonpath: vec![],
                path_str: "b".to_string(),
                rel_descs: vec![],
                invrels: vec![],
                parent_path: vec!["test_dataset".to_string()],
                alias: "".to_string(),
            },
            comparisons: vec![IComparePb {
                comp: CompOperatorPb::Gt as i32,
                left_val_idx: 1,  // Matches datapath val_idx
                right_val_idx: 2, // Matches constant at index 2
                right_val_cnt: 1,
            }],
        },
    ];
    let ranges = compute_index_range_for_index(&sqlvals, &iindex, &dpth_analyzers);
    println!(
        "Computed range for both segments match, first segment GT comparison:\n{:?}",
        ranges
    );
    assert!(!ranges.is_empty());
    assert_eq!(ranges.len(), 2);
    let range = &ranges[0];
    assert_eq!(range.lower_bound_val_idx, 2); // Same as before
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Gt as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);
    let range = &ranges[1];
    assert_eq!(range.lower_bound_val_idx, 5); // Same as before
    assert_eq!(range.upper_bound_val_idx, -1);
    assert_eq!(range.lower_op, CompOperatorPb::Eq as i32);
    assert_eq!(range.upper_op, CompOperatorPb::Eq as i32);
}

#[test]
fn test_get_proj_rels_for_proj() {
    // Create rel analyzer
    let rel_analyzer = RelAnalyzer {
        rel_name: "test_rel".to_string(),
        tgt_ds_id: 1,
        tgt_ds_name: "test_dataset".to_string(),
        inverse: false,
        pk_segs: vec!["a".to_string()],
        rel_id: 2,
        index_root_id: 3,
    };
    let rel_analyzers = vec![rel_analyzer];

    // Create index candidates
    let iindex = IIndexPb {
        idx_type: IndexTypePb::Pkey.into(),
        ds_name: "test_dataset".to_string(),
        name: "test_index".to_string(),
        seg_strs: vec!["a".to_string()],
        seg_vecs: vec![IndexSegPb {
            seg_vec: vec!["a".to_string()],
        }],
        root_id: 0,
        key_val_idx: 0,
        range: None,
        op: IndexOpPb::Scan as i32,
    };
    let idx_analyzer = IndexAnalyzer {
        iindex: iindex.clone(),
        dpth_analyzers: vec![],
    };
    let _idx_analyzers = vec![idx_analyzer];

    let idpath = IDatapathPb {
        phase: EvalPhasePb::Projection as i32,
        ds_name: "test_dataset".to_string(),
        pathsegs: vec![],
        jsonpath: vec!["*".to_string()],
        path_str: "*".to_string(),
        rel_descs: vec![],
        key_val_idx: 0,
        val_idx: 1,
        invrels: vec![],
        parent_path: vec!["test_dataset".to_string()],
        alias: "".to_string(),
    };

    // Call function to get projection with rels
    let result_proj = iutils::get_proj_rels(&idpath, &rel_analyzers);
    println!("Projection with rels: {:?}", result_proj);
    assert_eq!(result_proj.rel_descs.len(), 1);
    let rel_desc = &result_proj.rel_descs[0];
    assert_eq!(rel_desc.name, "test_rel");
    assert_eq!(rel_desc.id, 2);
    assert_eq!(rel_desc.pk_segs, vec!["a".to_string()]);
}

#[test]
fn test_path_matching() {
    let test_data = vec![
        (
            vec!["a".to_string()],
            vec!["a".to_string(), "b".to_string()],
            true,
        ),
        (
            vec!["a".to_string(), "b".to_string()],
            vec!["a".to_string()],
            true,
        ),
        (
            vec!["a".to_string(), "b".to_string()],
            vec!["a".to_string(), "b".to_string()],
            true,
        ),
        (
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
            vec!["a".to_string(), "b".to_string()],
            true,
        ),
        (
            vec!["a".to_string(), "c".to_string()],
            vec!["a".to_string(), "b".to_string()],
            false,
        ),
    ];
    for (path, index_path, expected) in test_data {
        let result = path_matches(&path, &index_path);
        println!(
            "Testing path {:?} against index path {:?}, expected: {}, got: {}",
            path, index_path, expected, result
        );
        assert_eq!(result, expected);
    }
}
