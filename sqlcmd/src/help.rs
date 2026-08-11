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

const HELP_BOOK: &str = include_str!("help.txt");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpTopic {
    Root,
    // Dataset
    Dataset,
    CreateDataset,
    AlterDataset,
    DropDataset,
    DescribeDataset,
    // Select
    Select,
    SelectList,
    SelectPredicates,
    SelectProjection,
    // Insert
    Insert,
    // Update
    Update,
    // Delete
    Delete,
    // Relationships
    Relationships,
    RelationshipCreate,
    RelationshipDrop,
    RelationshipInsert,
    RelationshipDelete,
    RelationshipPredicate,
    RelationshipProjection,
    // Jsonpath
    Jsonpath,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpAction {
    Show(HelpTopic),
    InvalidSelection,
    NotHelp,
}

impl HelpTopic {
    fn section_name(self) -> &'static str {
        match self {
            HelpTopic::Root => "help",
            // Dataset
            HelpTopic::Dataset => "help dataset",
            HelpTopic::CreateDataset => "help create dataset",
            HelpTopic::AlterDataset => "help alter dataset",
            HelpTopic::DropDataset => "help drop dataset",
            HelpTopic::DescribeDataset => "help describe dataset",
            // Select
            HelpTopic::Select => "help select",
            HelpTopic::SelectList => "help select list",
            HelpTopic::SelectPredicates => "help select predicates",
            HelpTopic::SelectProjection => "help select projection",
            // Insert
            HelpTopic::Insert => "help insert",
            // Update
            HelpTopic::Update => "help update",
            // Delete
            HelpTopic::Delete => "help delete",
            // Relationships
            HelpTopic::Relationships => "help relationships",
            HelpTopic::RelationshipCreate => "help relationship create",
            HelpTopic::RelationshipDrop => "help relationship drop",
            HelpTopic::RelationshipInsert => "help relationship insert",
            HelpTopic::RelationshipDelete => "help relationship delete",
            HelpTopic::RelationshipPredicate => "help relationship predicate",
            HelpTopic::RelationshipProjection => "help relationship projection",
            // Jsonpath
            HelpTopic::Jsonpath => "help jsonpath",
        }
    }

    fn child_for_choice(self, choice: usize) -> Option<Self> {
        match self {
            HelpTopic::Root => match choice {
                1 => Some(HelpTopic::Dataset),
                2 => Some(HelpTopic::Select),
                3 => Some(HelpTopic::Insert),
                4 => Some(HelpTopic::Update),
                5 => Some(HelpTopic::Delete),
                6 => Some(HelpTopic::Relationships),
                7 => Some(HelpTopic::Jsonpath),
                _ => None,
            },
            HelpTopic::Dataset => match choice {
                1 => Some(HelpTopic::CreateDataset),
                2 => Some(HelpTopic::AlterDataset),
                3 => Some(HelpTopic::DropDataset),
                4 => Some(HelpTopic::DescribeDataset),
                _ => None,
            },
            HelpTopic::Select => match choice {
                1 => Some(HelpTopic::SelectList),
                2 => Some(HelpTopic::SelectPredicates),
                3 => Some(HelpTopic::SelectProjection),
                _ => None,
            },
            HelpTopic::Relationships => match choice {
                1 => Some(HelpTopic::RelationshipCreate),
                2 => Some(HelpTopic::RelationshipDrop),
                3 => Some(HelpTopic::RelationshipInsert),
                4 => Some(HelpTopic::RelationshipDelete),
                5 => Some(HelpTopic::RelationshipPredicate),
                6 => Some(HelpTopic::RelationshipProjection),
                _ => None,
            },
            HelpTopic::SelectList
            | HelpTopic::SelectPredicates
            | HelpTopic::SelectProjection
            | HelpTopic::Insert
            | HelpTopic::Update
            | HelpTopic::Delete
            | HelpTopic::Jsonpath
            | HelpTopic::CreateDataset
            | HelpTopic::AlterDataset
            | HelpTopic::DropDataset
            | HelpTopic::DescribeDataset
            | HelpTopic::RelationshipCreate
            | HelpTopic::RelationshipDrop
            | HelpTopic::RelationshipInsert
            | HelpTopic::RelationshipDelete
            | HelpTopic::RelationshipPredicate
            | HelpTopic::RelationshipProjection => None,
        }
    }
}

fn normalize_help_input(input: &str) -> &str {
    input.trim().trim_end_matches(';').trim()
}

fn known_help_topics() -> &'static [(&'static str, HelpTopic)] {
    &[
        ("dataset", HelpTopic::Dataset),
        ("create dataset", HelpTopic::CreateDataset),
        ("alter dataset", HelpTopic::AlterDataset),
        ("drop dataset", HelpTopic::DropDataset),
        ("describe dataset", HelpTopic::DescribeDataset),
        ("select", HelpTopic::Select),
        ("select list", HelpTopic::SelectList),
        ("select predicates", HelpTopic::SelectPredicates),
        ("select projection", HelpTopic::SelectProjection),
        ("insert", HelpTopic::Insert),
        ("update", HelpTopic::Update),
        ("delete", HelpTopic::Delete),
        ("jsonpath", HelpTopic::Jsonpath),
        ("relationships", HelpTopic::Relationships),
        ("relationship create", HelpTopic::RelationshipCreate),
        ("relationship drop", HelpTopic::RelationshipDrop),
        ("relationship insert", HelpTopic::RelationshipInsert),
        ("relationship delete", HelpTopic::RelationshipDelete),
        ("relationship predicate", HelpTopic::RelationshipPredicate),
        ("relationship projection", HelpTopic::RelationshipProjection),
    ]
}

fn resolve_direct_topic(input: &str) -> Option<HelpTopic> {
    if input.eq_ignore_ascii_case("help") {
        return Some(HelpTopic::Root);
    }

    let lowered = input.to_ascii_lowercase();
    let topic = lowered.strip_prefix("help ")?.trim();

    if topic.is_empty() {
        return Some(HelpTopic::Root);
    }

    if let Some((_, matched_topic)) = known_help_topics().iter().find(|(name, _)| *name == topic) {
        return Some(*matched_topic);
    }

    known_help_topics()
        .iter()
        .filter(|(name, _)| name.starts_with(topic))
        .min_by(|(a_name, _), (b_name, _)| a_name.len().cmp(&b_name.len()).then(a_name.cmp(b_name)))
        .map(|(_, matched_topic)| *matched_topic)
}

pub fn classify_help_input(input: &str, current_topic: Option<HelpTopic>) -> HelpAction {
    let normalized = normalize_help_input(input);

    if normalized.is_empty() {
        return HelpAction::NotHelp;
    }

    if let Some(topic) = resolve_direct_topic(normalized) {
        return HelpAction::Show(topic);
    }

    if let Some(topic) = current_topic {
        if let Ok(choice) = normalized.parse::<usize>() {
            if let Some(next_topic) = topic.child_for_choice(choice) {
                return HelpAction::Show(next_topic);
            }
            return HelpAction::InvalidSelection;
        }
    }

    if normalized.to_ascii_lowercase().starts_with("help ") {
        return HelpAction::InvalidSelection;
    }

    HelpAction::NotHelp
}

pub fn render_help(topic: HelpTopic) -> String {
    extract_help_section(topic.section_name())
        .unwrap_or_else(|| format!("Help content is missing for {}", topic.section_name()))
}

fn extract_help_section(section_name: &str) -> Option<String> {
    let mut found_section = false;
    let mut lines = Vec::new();

    for raw_line in HELP_BOOK.lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            if found_section {
                lines.push(String::new());
            }
            continue;
        }

        if line.starts_with('[') && line.ends_with(']') {
            if found_section {
                break;
            }
            found_section = line.trim_matches(['[', ']'].as_ref()) == section_name;
            continue;
        }

        if found_section {
            lines.push(line.to_string());
        }
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_help_input, render_help, HelpAction, HelpTopic};

    #[test]
    fn root_menu_uses_top_level_topics() {
        let help = render_help(HelpTopic::Root);
        assert!(help.contains("1 help dataset"));
        assert!(help.contains("2 help select"));
        assert!(help.contains("3 help insert"));
        assert!(help.contains("4 help update"));
        assert!(help.contains("5 help delete"));
    }

    #[test]
    fn dataset_help_has_second_level_topics() {
        let help = render_help(HelpTopic::Dataset);
        assert!(help.contains("1 help create dataset"));
        assert!(help.contains("2 help alter dataset"));
        assert!(help.contains("3 help drop dataset"));
        assert!(help.contains("4 help describe dataset"));
    }

    #[test]
    fn select_help_has_second_level_topics() {
        let help = render_help(HelpTopic::Select);
        assert!(help.contains("1 select list"));
        assert!(help.contains("2 select predicates"));
        assert!(help.contains("3 select projection"));
    }

    #[test]
    fn numeric_choice_follows_active_help_page() {
        assert_eq!(
            classify_help_input("1", Some(HelpTopic::Root)),
            HelpAction::Show(HelpTopic::Dataset)
        );
        assert_eq!(
            classify_help_input("2", Some(HelpTopic::Dataset)),
            HelpAction::Show(HelpTopic::AlterDataset)
        );
        assert_eq!(
            classify_help_input("1", Some(HelpTopic::Select)),
            HelpAction::Show(HelpTopic::SelectList)
        );
    }

    #[test]
    fn direct_help_command_resolves_topics() {
        assert_eq!(
            classify_help_input("help select", None),
            HelpAction::Show(HelpTopic::Select)
        );
        assert_eq!(
            classify_help_input("help dataset", None),
            HelpAction::Show(HelpTopic::Dataset)
        );
        assert_eq!(
            classify_help_input("help alter dataset", None),
            HelpAction::Show(HelpTopic::AlterDataset)
        );
        assert_eq!(
            classify_help_input("help create dataset", None),
            HelpAction::Show(HelpTopic::CreateDataset)
        );
        assert_eq!(
            classify_help_input("help delete", None),
            HelpAction::Show(HelpTopic::Delete)
        );
        assert_eq!(
            classify_help_input("help jsonpath", None),
            HelpAction::Show(HelpTopic::Jsonpath)
        );
        assert_eq!(
            classify_help_input("help relationship create", None),
            HelpAction::Show(HelpTopic::CreateRelationship)
        );
        assert_eq!(
            classify_help_input("help relationship alter", None),
            HelpAction::Show(HelpTopic::AlterRelationship)
        );
        assert_eq!(
            classify_help_input("help relationship drop", None),
            HelpAction::Show(HelpTopic::DropRelationship)
        );
    }

    #[test]
    fn partial_help_topic_matches_prefix() {
        assert_eq!(
            classify_help_input("help sel", None),
            HelpAction::Show(HelpTopic::Select)
        );
        assert_eq!(
            classify_help_input("help select pr", None),
            HelpAction::Show(HelpTopic::SelectPredicates)
        );
    }

    #[test]
    fn ambiguous_partial_help_topic_uses_shortest_match() {
        assert_eq!(
            classify_help_input("help cr", None),
            HelpAction::Show(HelpTopic::CreateDataset)
        );
        assert_eq!(
            classify_help_input("help d", None),
            HelpAction::Show(HelpTopic::Delete)
        );
    }
}
