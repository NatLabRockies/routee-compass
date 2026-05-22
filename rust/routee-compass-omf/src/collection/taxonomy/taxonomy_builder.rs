use super::TaxonomyModel;
use crate::collection::constants::OVERTURE_TAXONOMY_URL;
use crate::collection::OvertureMapsCollectionError;
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TaxonomyModelBuilder {
    activity_mappings: HashMap<String, Vec<String>>,
    source_url: Option<String>,
}

impl From<HashMap<String, Vec<String>>> for TaxonomyModelBuilder {
    fn from(value: HashMap<String, Vec<String>>) -> Self {
        Self {
            activity_mappings: value,
            source_url: None,
        }
    }
}

impl TaxonomyModelBuilder {
    pub fn new(
        activity_mappings: HashMap<String, Vec<String>>,
        source_url: Option<String>,
    ) -> Self {
        Self {
            activity_mappings,
            source_url,
        }
    }

    pub fn build(&self) -> Result<TaxonomyModel, OvertureMapsCollectionError> {
        // Collect taxonomy records from CSV
        let taxonomy_tree = download_taxonomy_csv(self.source_url.clone())?;

        // Process records to identify (child, parent) pairs
        let processed_tree_nodes = taxonomy_tree
            .into_iter()
            .map(|(category, parents)| {
                if parents.len() < 2 {
                    return (category, None);
                };
                (category, Some(parents[parents.len() - 2].to_owned()))
            })
            .collect::<Vec<(String, Option<String>)>>();

        // Cloning here too expensive?
        Ok(TaxonomyModel::from_tree_nodes(
            processed_tree_nodes,
            self.activity_mappings.clone(),
        ))
    }

    pub fn get_mappings(&self) -> HashMap<String, Vec<String>> {
        self.activity_mappings.clone()
    }
}

#[derive(Deserialize)]
struct TaxonomyCSVRecord {
    #[serde(rename = "Category code")]
    category: String,
    #[serde(rename = "Overture Taxonomy")]
    taxonomy: String,
}

fn download_taxonomy_csv(
    source_url: Option<String>,
) -> Result<Vec<(String, Vec<String>)>, OvertureMapsCollectionError> {
    // Create a new thread to handle async operations
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| {
            OvertureMapsCollectionError::TaxonomyLoadingError(format!(
                "Failed to safely create thread to consume taxonomy csv: {e}"
            ))
        })?;

    // Set default for the url
    let source_url = source_url.unwrap_or(OVERTURE_TAXONOMY_URL.to_owned());

    // Execute GET request and parse response as text
    let response = runtime.block_on(reqwest::get(source_url)).map_err(|e| {
        OvertureMapsCollectionError::TaxonomyLoadingError(format!("GET request failed: {e}"))
    })?;
    let csv_content = runtime.block_on(response.text()).map_err(|e| {
        OvertureMapsCollectionError::TaxonomyLoadingError(format!("Parsing response failed: {e}"))
    })?;

    // Parse text from response as CSV
    let mut rdr = ReaderBuilder::new()
        .has_headers(true)
        .delimiter(b';')
        .trim(csv::Trim::All)
        .from_reader(csv_content.as_bytes());

    // Deserialize each row into Taxonomy record and then into (String, Vec<String>)
    let mut results = Vec::new();
    for result in rdr.deserialize() {
        let record: TaxonomyCSVRecord = result
            .map_err(|e| OvertureMapsCollectionError::TaxonomyDeserializingError(format!("{e}")))?;
        let taxonomy: Vec<String> = record
            .taxonomy
            .replace("[", "")
            .replace("]", "")
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        results.push((record.category, taxonomy));
    }

    Ok(results)
}
