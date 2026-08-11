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

use demodata::tstdata;

#[test]
fn test_add_set() {
    let schema = tstdata::SchemaPb { 
        datasets: vec![], 
        indexes: vec![], 
        rs_items: vec![],
        invrs_items: vec![],
        next_dataset_id: 0,
        next_item_id: 0,
    };
    let mut set = tstdata::DatasetPb {
        id: 0,
        name: "myset".to_string(),
        descriptor: Some(tstdata::ItemPb { id: 0, jb_content: b"desc".to_vec() }),
        items: vec![],
    };
    let item = tstdata::ItemPb {
        id: 1,
        jb_content: b"test".to_vec(),
    };
    set.items.push(item);
    let mut schema = schema;
    schema.datasets.push(set);
    assert_eq!(schema.datasets.len(), 1);
    assert_eq!(schema.datasets[0].items.len(), 1);
    assert_eq!(schema.datasets[0].items[0].jb_content, b"test".to_vec());
}
