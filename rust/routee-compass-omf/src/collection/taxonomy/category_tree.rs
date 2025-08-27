use std::collections::{HashMap, HashSet};

#[derive(Default, Debug)]
pub struct CategoryTree(HashMap<String, Vec<String>>);

impl CategoryTree {
    /// Creates a new CategoryTree data structure from a vector of category -> parent relationships.
    /// The final datastructure is a HashMap where the keys are nodes and the values are vectors of
    /// Strings representing the children of each node.
    ///
    /// # Arguments
    ///
    /// * `nodes` - Vector of cateogry, parent node relationships. All nodes are
    ///             expected to be represented by strings. If no parent is given,
    ///             i.e. the second element is None, the entry is ignored
    pub fn new(nodes: Vec<(String, Option<String>)>) -> Self {
        let mut tree: HashMap<String, Vec<String>> = HashMap::new();

        for node in nodes {
            let (category, parent) = node;

            // If parent is None, we ignore this entry
            if let Some(parent_label) = parent {
                let parent_node = tree.entry(parent_label.clone()).or_default();
                parent_node.push(category);
            }
            // We only need to take care of the second to last one
            // because the every entry in this list is a node that eventually
            // will be processed
            // if parents.len() < 2 {continue;}

            // let parent_label = &parents[parents.len() - 2];
            // let parent_node = tree.entry(parent_label.clone()).or_insert(vec![]);
            // parent_node.push(category);
        }

        Self(tree)
    }

    /// Recursively get a linear representation of all the nodes below a given node in the tree. The
    /// output of this function includes the node itself. If the node is not found, it returns an empty list.
    ///
    /// # Arguments
    ///
    /// * `node` - Node at which to start the search.
    pub fn get_linearized_children(&self, node: String) -> Vec<String> {
        let mut node_children = self.0.get(&node).cloned().unwrap_or(Vec::<String>::new());
        let recursive_children: Vec<String> = node_children
            .iter()
            .flat_map(|e| self.get_linearized_children(e.to_owned()))
            .collect();
        node_children.extend(recursive_children);
        node_children
    }

    /// Compute a linear representation of all the possible values that would satisfy a query. In this case,
    /// a query is a vector of nodes in the tree and all the nodes below them. The ouput of this function
    /// is a HashSet that contains all the possible values in the original input query and their recursive
    /// children.
    /// # Arguments
    ///
    /// * `query` - Vector of all the categories to be considered
    pub fn get_linearized_query(&self, query: Vec<String>) -> HashSet<String> {
        // Linearize the query: get all the possible values that match
        // e.g If I put restaurant, any of restaurant, afghan_restaurant, moroccan_restaurant, ... work
        let linearized_children: Vec<String> = query
            .iter()
            .flat_map(|e| self.get_linearized_children(e.to_owned()))
            .collect();

        // Make a HashSet with the linearized query
        HashSet::from_iter(query.into_iter().chain(linearized_children))
    }
}
