use super::{
    vehicle_restriction_builder_config::VehicleRestrictionBuilderConfig, RestrictionRow,
    VehicleParameterType, VehicleRestriction, VehicleRestrictionFrontierService,
};
use crate::{
    model::{
        constraint::{ConstraintModelBuilder, ConstraintModelError, ConstraintModelService},
        network::EdgeId,
    },
    util::fs::read_utils,
};
use indexmap::IndexMap;
use kdam::Bar;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

pub struct VehicleRestrictionBuilder {}

impl ConstraintModelBuilder for VehicleRestrictionBuilder {
    fn build(
        &self,
        parameters: &serde_json::Value,
    ) -> Result<Arc<dyn ConstraintModelService>, ConstraintModelError> {
        let config: VehicleRestrictionBuilderConfig = serde_json::from_value(parameters.clone())
            .map_err(|e| {
                ConstraintModelError::BuildError(format!(
                    "failed to read vehicle restriction configuration: {e}"
                ))
            })?;

        let vehicle_restriction_input_file = PathBuf::from(&config.vehicle_restriction_input_file);

        let vehicle_restriction_lookup =
            vehicle_restriction_lookup_from_file(&vehicle_restriction_input_file)?;

        let m = VehicleRestrictionFrontierService {
            vehicle_restriction_lookup: Arc::new(vehicle_restriction_lookup),
        };

        Ok(Arc::new(m))
    }
}

pub fn vehicle_restriction_lookup_from_file(
    vehicle_restriction_input_file: &PathBuf,
) -> Result<HashMap<EdgeId, IndexMap<VehicleParameterType, VehicleRestriction>>, ConstraintModelError>
{
    let rows: Vec<RestrictionRow> = read_utils::from_csv(
        &vehicle_restriction_input_file,
        true,
        Some(Bar::builder().desc("vehicle restrictions")),
        None,
    )
    .map_err(|e| {
        ConstraintModelError::BuildError(format!(
            "Could not load vehicle restriction file {vehicle_restriction_input_file:?}: {e}"
        ))
    })?
    .to_vec();

    let mut vehicle_restriction_lookup: HashMap<
        EdgeId,
        IndexMap<VehicleParameterType, VehicleRestriction>,
    > = HashMap::new();
    for row in rows {
        let restriction = VehicleRestriction::try_from(&row)?;
        match vehicle_restriction_lookup.get_mut(&row.edge_id) {
            None => {
                let mut restrictions = IndexMap::new();
                restrictions.insert(restriction.vehicle_parameter_type().clone(), restriction);
                vehicle_restriction_lookup.insert(row.edge_id, restrictions);
            }
            Some(restrictions) => {
                restrictions.insert(restriction.vehicle_parameter_type().clone(), restriction);
            }
        }
    }
    Ok(vehicle_restriction_lookup)
}
