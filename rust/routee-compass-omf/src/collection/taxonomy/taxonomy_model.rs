use super::category_tree::CategoryTree;
use std::collections::{HashMap, HashSet};

use crate::collection::error::OvertureMapsCollectionError;

/// Implements the logic of grouping OvertureMaps Labels
/// into semantically equivalent groups
/// e.g.
///   restaurant, bar -> food
///   sports_and_recreation_venue, arts_and_entertainment -> entertainment
///
/// It internally uses a `CategoryTree` to obtain a map from group label
/// to OvertureMaps label with constant time complexity
///
/// ```
/// use crate::bambam_omf::collection::TaxonomyModel;
/// use std::collections::HashMap;
///
/// let tree_nodes: Vec<(String, Option<String>)> = vec![(String::from("restaurant"), Some(String::from("eat_and_drink"))),
///                                                    (String::from("eat_and_drink"), None),
///                                                    (String::from("chilean_restaurant"), Some(String::from("restaurant"))),
///                                                    (String::from("bar"), Some(String::from("eat_and_drink"))),
///                                                    (String::from("bank"), Some(String::from("services"))),
///                                                    (String::from("services"), None)];
/// let group_mappings = HashMap::<String, Vec<String>>::from_iter(vec![(String::from("restaurants"), vec![String::from("restaurant")]),
///                                                                     (String::from("food"), vec![String::from("eat_and_drink")]),
///                                                                     (String::from("fun_places"), vec![String::from("chilean_restaurant"), String::from("bank")])]
///                                                                .into_iter());
/// let taxonomy_model = TaxonomyModel::from_tree_nodes(tree_nodes, group_mappings);
///
/// let groups = vec![String::from("restaurants"), String::from("food"), String::from("fun_places")];
/// assert_eq!(taxonomy_model.reverse_map(&[String::from("chilean_restaurant"),
///                                         String::from("restaurant"),
///                                         String::from("bank")
///                                        ], groups).unwrap(),
///            vec![vec![true, true, true],
///                 vec![true, true, false],
///                 vec![false, false, true]])
/// ```
#[derive(Debug, Clone)]
pub struct TaxonomyModel {
    group_mappings: HashMap<String, HashSet<String>>,
}

impl TaxonomyModel {
    /// Build [`TaxonomyModel`] for tree-based taxonomy mapping. This implementation assumes that
    /// the desired taxonomy can be represented as a forest data structure in which each node represents
    /// a category/concept and children nodes represent sub-categories (typically more specific). This
    /// function builds a taxonomy model that can parse the forest structure and use it to compute all
    /// the categories that belong to a group.
    ///
    /// Arguments
    ///
    /// * `tree_nodes` - Vector of cateogry, parent node relationships. All nodes are
    ///             expected to be represented by strings. If no parent is given,
    ///             i.e. the second element is None, the entry is treated as a root
    /// * `group_mappings` - HashMap specifying the categories (nodes) that are associated with
    ///             each group.
    pub fn from_tree_nodes(
        tree_nodes: Vec<(String, Option<String>)>,
        group_mappings: HashMap<String, Vec<String>>,
    ) -> Self {
        // Build category tree with nodes
        let category_tree = CategoryTree::new(tree_nodes);

        // Linearize each query
        let activity_mappings = group_mappings
            .into_iter()
            .map(|(activity, categories)| {
                (activity, category_tree.get_linearized_query(categories))
            })
            .collect::<HashMap<String, HashSet<String>>>();

        TaxonomyModel {
            // group_labels: activity_fields,
            group_mappings: activity_mappings,
        }
    }

    /// This constructor receives a complete mapping, it does not expand
    /// from a CategoryTree
    pub fn from_mapping(mapping: HashMap<String, HashSet<String>>) -> Self {
        Self {
            group_mappings: mapping,
        }
    }

    /// Compute the union of all mappings. Useful to filter points as those are retrieved from an external source.
    pub fn get_unique_categories(&self) -> HashSet<String> {
        let mut result = HashSet::<String>::new();
        for set in self
            .group_mappings
            .values()
            .cloned()
            .collect::<Vec<HashSet<String>>>()
        {
            result.extend(set);
        }
        result
    }

    pub fn reverse_map(
        &self,
        categories: &[String],
        group_labels: Vec<String>,
    ) -> Result<Vec<Vec<bool>>, OvertureMapsCollectionError> {
        categories
            .iter()
            .map(|category| {
                group_labels
                    .iter()
                    .map(|group| {
                        Ok::<bool, OvertureMapsCollectionError>(
                            self.group_mappings
                                .get(group)
                                .ok_or(OvertureMapsCollectionError::GroupMappingError(format!(
                                    "Group {group} was not found in mapping"
                                )))?
                                .contains(category),
                        )
                    })
                    .collect::<Result<Vec<bool>, OvertureMapsCollectionError>>()
            })
            .collect::<Result<Vec<Vec<bool>>, OvertureMapsCollectionError>>()
    }
}
