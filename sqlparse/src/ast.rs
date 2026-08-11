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

use std::fmt::{Debug, Display, Formatter, Result as FmtResult}; // For pretty-printing the AST

// Define the AST
#[derive(Debug)]
pub enum SqlStmt {
    Select(SelectStmt),
    DeleteFrom(DeleteFromStmt),
    InsertInto(InsertIntoStmt),
    Update(UpdateStmt),
    Yank(YankStmt),
    UpdateRel(UpdateRelStmt),
    CreateDataset(CreateDatasetStmt),
    DropDataset(DropDatasetStmt),
    DescribeDataset(DescribeDatasetStmt),
    AlterDataset(AlterDatasetStmt),
}

#[derive(Debug)]
pub struct CreateDatasetStmt {
    pub name: String, // single component path for class name
    pub actions: Vec<AlterAction>,
}

#[derive(Debug)]
pub struct DropDatasetStmt {
    pub name: String, // single component path for class name
}

#[derive(Debug)]
pub struct DescribeDatasetStmt {
    pub name: String, // single component path for class name
}

#[derive(Debug)]
pub enum AlterAction {
    AddRel(RelDef),
    DropRel(String), // relationship name
    AddIdx(IndexDef),
    DropIdx(String), // index name
}

#[derive(Debug)]
pub struct AlterDatasetStmt {
    pub ds_name: String, // single component path for class name
    pub actions: Vec<AlterAction>,
}

#[derive(Debug, PartialEq)]
pub enum IndexType {
    Pkey,
    Unique,
    NonUnique,
}

#[derive(Debug)]
pub struct IndexDef {
    pub name: String,
    pub idx_type: IndexType,
    pub update_type: UpdateType,
    pub fields: Vec<FieldSegments>, // Index seg paths like "a.b", "c"
}

#[derive(Debug)]
pub struct RelDef {
    pub update_type: UpdateType,
    pub name: String,
    pub tgt_dataset: String,
}

#[derive(Debug)]
pub struct InsertIntoStmt {
    pub ds_name: String,
    pub values: Vec<String>,
}

#[derive(Debug)]
pub struct DeleteFromStmt {
    pub ds_name: String,
    pub predicate_list: Vec<Predicate>,
}

#[derive(Debug)]
pub struct FromListItem {
    pub ds_name: String,
    pub alias: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum UpdateType {
    Insert,
    Delete,
    DdlAdd,
    DdlDrop,
}

#[derive(Debug)]
pub struct UpdateRelStmt {
    pub name: String,              // relationship name
    pub update_type: UpdateType,   // insert or delete
    pub values: Vec<RelSuccessor>, // list of successor PK segments
    pub from_list: Vec<FromListItem>,
    pub predicate_list: Vec<Predicate>, // filter the dataset item to insert the rel into
}

#[derive(Debug)]
pub struct SelectStmt {
    pub proj_list: Vec<Member>,       // Constant values and datapath paths
    pub from_list: Vec<FromListItem>, // dataset name and optional alias
    pub predicate_list: Vec<Predicate>,
}

#[derive(Debug)]
pub struct SetValue {
    pub fieldsegs: FieldSegments,
    pub value: ConstValue,
}

#[derive(Debug)]
pub struct UpdateStmt {
    pub ds_name: String,
    pub values: Vec<SetValue>,
    pub predicate_list: Vec<Predicate>, // filter the dataset items to update
}

#[derive(Debug)]
pub struct YankStmt {
    pub ds_name: String,
    pub fields: Vec<FieldSegments>,     // list of field segs to delete
    pub predicate_list: Vec<Predicate>, // filter the dataset items to update
}

#[derive(Debug)]
pub struct Predicate {
    pub left: Member,
    pub right: Member,
    pub comp_operator: CompOperator,
}

#[derive(Debug)]
pub struct Member {
    pub part: MemberPart,
}

// Projection or predicate member (e.g., a.b.c + d)
#[derive(Debug)]
pub enum MemberPart {
    Tree(Box<Member>, ArithOperator, Box<Member>),
    Path(PathSegments),
    Value(ConstValue),
    ValueList(Vec<ConstValue>),
}

#[derive(Debug)]
pub enum ConstValue {
    IsNull(),
    Null(),
    Bool(bool),
    Number(String),
    SingleQuotedString(String),
    DoubleQuotedString(String),
}

#[derive(Debug)]
pub struct PathSegment {
    pub name: String,
    pub target_ds: String, // Contains the target ds name if inverse rel
}

#[derive(Debug)]
pub struct PathSegments {
    pub segments: Vec<PathSegment>, // Segments preceding the final JSON path. e.g: a.rs.$.*
    pub jsonpath: Vec<String>,      // JSON path segments
}

#[derive(Debug)]
pub struct FieldSegments {
    pub segments: Vec<String>, // Simpler version for create index, rels ...
}

#[derive(Debug)]
pub enum PredLogicOperator {
    And,
    Or,
}

#[derive(Debug)]
pub enum ArithOperator {
    Plus,
    Minus,
    Multiply,
    Divide,
}

#[derive(Debug)]
pub enum CompOperator {
    In,
    NotIn,
    Like,
    NotLike,
    Regexp,
    NotRegexp,
    EqEq,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
}

// Successor composite PKey segments
#[derive(Debug)]
pub struct RelSuccessor {
    pub s: Vec<ConstValue>,
}

// Tree print implementations
impl SqlStmt {
    pub fn pretty_print(&self) -> String {
        format!("{}", self)
    }

    pub fn print_tree(&self) -> String {
        let mut result = String::new();
        match self {
            SqlStmt::Select(stmt) => {
                result.push_str("SELECT Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::CreateDataset(stmt) => {
                result.push_str("CREATE DATASET Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::DropDataset(stmt) => {
                result.push_str("DROP DATASET Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::DescribeDataset(stmt) => {
                result.push_str("DESCRIBE DATASET Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::AlterDataset(stmt) => {
                result.push_str("ALTER DATASET Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::DeleteFrom(stmt) => {
                result.push_str("DELETE FROM Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::InsertInto(stmt) => {
                result.push_str("INSERT INTO Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::UpdateRel(stmt) => {
                result.push_str("UPDATE RELATIONSHIP Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::Update(stmt) => {
                result.push_str("UPDATE Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
            SqlStmt::Yank(stmt) => {
                result.push_str("YANK Statement\n");
                result.push_str(&stmt.format_tree(1));
            }
        }
        result
    }
}

trait TreeFormatter {
    fn format_tree(&self, indent: usize) -> String;
}

impl TreeFormatter for CreateDatasetStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.name));
        result
    }
}

impl TreeFormatter for DescribeDatasetStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.name));
        result
    }
}

impl TreeFormatter for DropDatasetStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.name));
        result
    }
}

impl TreeFormatter for AlterDatasetStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.ds_name));
        if !self.actions.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("└─ Actions:\n");
            for (i, action) in self.actions.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.actions.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                match action {
                    AlterAction::AddRel(rel_def) => {
                        result
                            .push_str(&format!("{} Add Relationship: {}\n", prefix, rel_def.name));
                        for _ in 0..(indent + 2) {
                            result.push_str("  ");
                        }
                        result.push_str(&format!("Target Dataset: {}\n", rel_def.tgt_dataset));
                    }
                    AlterAction::DropRel(rel_name) => {
                        result.push_str(&format!("{} Drop Relationship: {}\n", prefix, rel_name));
                    }
                    AlterAction::AddIdx(idx_def) => {
                        result.push_str(&format!("{} Add Index: {}\n", prefix, idx_def.name));
                        for (j, segs) in idx_def.fields.iter().enumerate() {
                            for _ in 0..(indent + 2) {
                                result.push_str("  ");
                            }
                            let seg_prefix = if j == idx_def.fields.len() - 1 {
                                "└─"
                            } else {
                                "├─"
                            };
                            let path_str = segs.segments.join(".");
                            result.push_str(&format!("{} Path: {}\n", seg_prefix, path_str));
                        }
                    }
                    AlterAction::DropIdx(idx_name) => {
                        result.push_str(&format!("{} Drop Index: {}\n", prefix, idx_name));
                    }
                }
            }
        }
        result
    }
}

impl TreeFormatter for DeleteFromStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.ds_name));

        if !self.predicate_list.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("└─ Predicates:\n");
            for (i, predicate) in self.predicate_list.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.predicate_list.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                result.push_str(&format!("{} Predicate[{}]\n", prefix, i));
                result.push_str(&predicate.format_tree(indent + 2));
            }
        }

        result
    }
}

impl TreeFormatter for InsertIntoStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str("├─ Values:\n");
        for (i, value) in self.values.iter().enumerate() {
            for _ in 0..(indent + 1) {
                result.push_str("  ");
            }
            let prefix = if i == self.values.len() - 1 {
                "└─"
            } else {
                "├─"
            };
            result.push_str(&format!("{} {}\n", prefix, value));
        }
        result
    }
}

impl TreeFormatter for UpdateRelStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str("├─ Relationship Name: ");
        result.push_str(&self.name);
        result.push_str("\n");

        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str("├─ Target Dataset: ");
        for (i, member) in self.from_list.iter().enumerate() {
            for _ in 0..(indent + 1) {
                result.push_str("  ");
            }
            let prefix = if i == self.from_list.len() - 1 {
                "└─"
            } else {
                "├─"
            };
            result.push_str(&format!(
                "{} Dataset[{}] {} (alias: {:?})\n",
                prefix, i, member.ds_name, member.alias
            ));
        }
        result.push_str("\n");

        if !self.values.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("├─ Values:\n");
            for (i, rel_succ) in self.values.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.values.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                let succ_values: Vec<String> =
                    rel_succ.s.iter().map(|v| format!("{}", v)).collect();
                result.push_str(&format!("{} [{}]\n", prefix, succ_values.join(", ")));
            }
        }

        if !self.predicate_list.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("└─ Predicates:\n");
            for (i, predicate) in self.predicate_list.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.predicate_list.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                result.push_str(&format!("{} Predicate[{}]\n", prefix, i));
                result.push_str(&predicate.format_tree(indent + 2));
            }
        }

        result
    }
}

impl TreeFormatter for UpdateStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.ds_name));

        if !self.values.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("├─ Set Values:\n");
            for (i, set_value) in self.values.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.values.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                // join path segments name with dots
                let path_str = set_value.fieldsegs.segments.join(".");
                result.push_str(&format!(
                    "{} Path: {} = Value: {}\n",
                    prefix, path_str, set_value.value
                ));
            }
        }

        if !self.predicate_list.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("└─ Predicates:\n");
            for (i, predicate) in self.predicate_list.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.predicate_list.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                result.push_str(&format!("{} Predicate[{}]\n", prefix, i));
                result.push_str(&predicate.format_tree(indent + 2));
            }
        }
        result
    }
}

impl TreeFormatter for YankStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Dataset Name: {:?}\n", self.ds_name));

        if !self.fields.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("├─ Yanked Paths:\n");
            for (i, yanked_path) in self.fields.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.fields.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                // join path segments name with dots
                let path_str = yanked_path.segments.join(".");
                result.push_str(&format!("{} Path: {}\n", prefix, path_str));
            }
        }

        if !self.predicate_list.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("└─ Predicates:\n");
            for (i, predicate) in self.predicate_list.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.predicate_list.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                result.push_str(&format!("{} Predicate[{}]\n", prefix, i));
                result.push_str(&predicate.format_tree(indent + 2));
            }
        }
        result
    }
}
impl TreeFormatter for SelectStmt {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();

        // Projection list
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str("├─ SELECT\n");

        for (i, member) in self.proj_list.iter().enumerate() {
            for _ in 0..(indent + 1) {
                result.push_str("  ");
            }
            let prefix = if i == self.proj_list.len() - 1 {
                "└─"
            } else {
                "├─"
            };
            result.push_str(&format!("{} Projection[{}]\n", prefix, i));
            result.push_str(&member.format_tree(indent + 2));
        }

        // FROM clause
        if !self.from_list.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("├─ FROM\n");

            for (i, member) in self.from_list.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.from_list.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                result.push_str(&format!(
                    "{} Dataset[{}] {} (alias: {:?})\n",
                    prefix, i, member.ds_name, member.alias
                ));
            }
        }

        // WHERE clause
        if !self.predicate_list.is_empty() {
            for _ in 0..indent {
                result.push_str("  ");
            }
            result.push_str("└─ WHERE\n");

            for (i, predicate) in self.predicate_list.iter().enumerate() {
                for _ in 0..(indent + 1) {
                    result.push_str("  ");
                }
                let prefix = if i == self.predicate_list.len() - 1 {
                    "└─"
                } else {
                    "├─"
                };
                result.push_str(&format!("{} Predicate[{}]\n", prefix, i));
                result.push_str(&predicate.format_tree(indent + 2));
            }
        }
        result
    }
}

impl TreeFormatter for Predicate {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();

        // Operator
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str(&format!("├─ Operator: {}\n", self.comp_operator));

        // Left operand
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str("├─ Left\n");
        result.push_str(&self.left.format_tree(indent + 1));

        // Right operand
        for _ in 0..indent {
            result.push_str("  ");
        }
        result.push_str("└─ Right\n");
        result.push_str(&self.right.format_tree(indent + 1));

        result
    }
}

impl TreeFormatter for Member {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();

        // Member content
        result.push_str(&self.part.format_tree(indent));

        result
    }
}

impl TreeFormatter for MemberPart {
    fn format_tree(&self, indent: usize) -> String {
        let mut result = String::new();

        match self {
            MemberPart::Tree(left, op, right) => {
                for _ in 0..indent {
                    result.push_str("  ");
                }
                result.push_str(&format!("├─ Expression: {}\n", op));

                for _ in 0..indent {
                    result.push_str("  ");
                }
                result.push_str("├─ Left\n");
                result.push_str(&left.format_tree(indent + 1));

                for _ in 0..indent {
                    result.push_str("  ");
                }
                result.push_str("└─ Right\n");
                result.push_str(&right.format_tree(indent + 1));
            }
            MemberPart::Path(segments) => {
                for _ in 0..indent {
                    result.push_str("  ");
                }
                let path: &Vec<PathSegment> = &segments.segments;
                result.push_str(&format!(
                    "├─ Path: {} JsonPath: {}\n",
                    path.iter()
                        .map(|seg| seg.name.clone())
                        .collect::<Vec<String>>()
                        .join("."),
                    segments.jsonpath.join(".")
                ));

                let mut i = 0;
                for segment in &segments.segments {
                    for _ in 0..(indent + 1) {
                        result.push_str("  ");
                    }
                    let prefix = if i == segments.segments.len() - 1 {
                        "└─"
                    } else {
                        "├─"
                    };
                    result.push_str(&format!("{} Segment[{}]: {:?}\n", prefix, i, &segment));
                    i += 1;
                }
            }
            MemberPart::Value(value) => {
                for _ in 0..indent {
                    result.push_str("  ");
                }
                result.push_str(&format!("└─ Value: {}\n", value));
            }
            MemberPart::ValueList(values) => {
                for _ in 0..indent {
                    result.push_str("  ");
                }
                result.push_str("└─ Value List:\n");
                for (i, value) in values.iter().enumerate() {
                    for _ in 0..(indent + 1) {
                        result.push_str("  ");
                    }
                    let prefix = if i == values.len() - 1 {
                        "└─"
                    } else {
                        "├─"
                    };
                    result.push_str(&format!("{} Value[{}]: {}\n", prefix, i, value));
                }
            }
        }

        result
    }
}

impl MemberPart {
    pub fn as_dot_delimited_path(&self) -> Option<String> {
        match self {
            MemberPart::Path(segments) => Some(
                segments
                    .segments
                    .iter()
                    .map(|seg| seg.name.clone())
                    .collect::<Vec<String>>()
                    .join("."),
            ),
            _ => None,
        }
    }
}

// Keep the original Display implementations for backward compatibility
impl Display for SqlStmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.print_tree())
    }
}

impl Display for CreateDatasetStmt {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "CREATE DATASET {:?}", self.name)
    }
}

impl Display for ConstValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ConstValue::IsNull() => write!(f, "is null"),       // is null comparison
            ConstValue::Null() => write!(f, "null"),            // 'xxx = null' comparison
            ConstValue::Bool(b) => write!(f, "{}", b),
            ConstValue::Number(n) => write!(f, "{}", n),
            ConstValue::SingleQuotedString(s) => write!(f, "'{}'", s),
            ConstValue::DoubleQuotedString(s) => write!(f, "\"{}\"", s),
        }
    }
}

impl Display for CompOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            CompOperator::In => write!(f, "IN"),
            CompOperator::NotIn => write!(f, "NOT IN"),
            CompOperator::Like => write!(f, "LIKE"),
            CompOperator::NotLike => write!(f, "NOT LIKE"),
            CompOperator::Regexp => write!(f, "REGEXP"),
            CompOperator::NotRegexp => write!(f, "NOT REGEXP"),
            CompOperator::EqEq => write!(f, "=="),
            CompOperator::Eq => write!(f, "="),
            CompOperator::Ne => write!(f, "!="),
            CompOperator::Gt => write!(f, ">"),
            CompOperator::Lt => write!(f, "<"),
            CompOperator::Ge => write!(f, ">="),
            CompOperator::Le => write!(f, "<="),
        }
    }
}

impl Display for ArithOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            ArithOperator::Plus => write!(f, "+"),
            ArithOperator::Minus => write!(f, "-"),
            ArithOperator::Multiply => write!(f, "*"),
            ArithOperator::Divide => write!(f, "/"),
        }
    }
}

impl Display for PredLogicOperator {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            PredLogicOperator::And => write!(f, "AND"),
            PredLogicOperator::Or => write!(f, "OR"),
        }
    }
}

// Example usage function
pub fn demo_pretty_print() {
    let select_stmt = SelectStmt {
        proj_list: vec![Member {
            part: MemberPart::Path(PathSegments {
                segments: vec![
                    PathSegment {
                        name: "user".to_string(),
                        target_ds: "User".to_string(),
                    },
                    PathSegment {
                        name: "name".to_string(),
                        target_ds: "User".to_string(),
                    },
                ],
                jsonpath: vec![],
            }),
        }],
        from_list: vec![FromListItem {
            ds_name: "User".to_string(),
            alias: None,
        }],
        predicate_list: vec![Predicate {
            left: Member {
                part: MemberPart::Path(PathSegments {
                    segments: vec![
                        PathSegment {
                            name: "user".to_string(),
                            target_ds: "User".to_string(),
                        },
                        PathSegment {
                            name: "age".to_string(),
                            target_ds: "User".to_string(),
                        },
                    ],
                    jsonpath: vec![],
                }),
            },
            comp_operator: CompOperator::Gt,
            right: Member {
                part: MemberPart::Value(ConstValue::Number("18".to_string())),
            },
        }],
    };

    let stmt = SqlStmt::Select(select_stmt);
    println!("{}", stmt.print_tree());
}

// TBD - must error out if inverse rel within jsonpath
pub fn append_jsonpath(head: PathSegments, elt: Vec<String>) -> PathSegments {
    // Only terminal non-jsonpath segment has usual 'SELECT *' behavior.
    // If anticipate '*.xxx' then move '*' to jsonpath.
    let mut j = head.jsonpath;
    j.extend(elt);
    PathSegments { jsonpath: j, ..head }
}
