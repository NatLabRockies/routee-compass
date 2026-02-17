use crate::model::unit::{AsF64, Cost};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::cost_model_error::CostModelError;

#[derive(Deserialize, Serialize, Clone, Copy, Debug, Default, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CostAggregation {
    /// sums all costs together
    #[default]
    Sum,
    /// multiplies all costs together
    Mul,
}

impl CostAggregation {
    /// aggregates the costs found in a collection of pairs (state_feature_name, cost_value)
    pub fn aggregate(&self, costs: &[(&str, Cost)]) -> Result<Cost, CostModelError> {
        match self {
            CostAggregation::Sum => {
                let mut sum = Cost::ZERO;
                for (_, cost) in costs.iter() {
                    sum += *cost;
                }
                Ok(sum)
            }
            CostAggregation::Mul => {
                // exit early if the iterator is empty
                if costs.is_empty() {
                    return Ok(Cost::ZERO);
                }

                let mut product = Cost::ONE;
                for (_, cost) in costs.iter() {
                    product = Cost::new(product.as_f64() * cost.as_f64());
                }
                Ok(product)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::model::{cost::CostAggregation, unit::Cost};

    #[test]
    fn test_agg_sum_empty() {
        let result = CostAggregation::Sum
            .aggregate(&[])
            .expect("sum should not fail over any real numbers");
        assert_eq!(result, Cost::ZERO)
    }

    #[test]
    fn test_agg_sum_singleton() {
        let result = CostAggregation::Sum
            .aggregate(&[("a", Cost::new(1.0))])
            .expect("sum should not fail over any real numbers");
        assert_eq!(result, Cost::ONE)
    }

    #[test]
    fn test_agg_sum_pair() {
        let result = CostAggregation::Sum
            .aggregate(&[("a", Cost::new(0.5)), ("b", Cost::new(0.5))])
            .expect("sum should not fail over any real numbers");
        assert_eq!(result, Cost::ONE)
    }

    #[test]
    fn test_agg_mul_empty() {
        let result = CostAggregation::Mul
            .aggregate(&[])
            .expect("mul should not fail over any real numbers");
        assert_eq!(result, Cost::ZERO)
    }

    #[test]
    fn test_agg_mul_singleton() {
        let result = CostAggregation::Mul
            .aggregate(&[("a", Cost::new(1.0))])
            .expect("mul should not fail over any real numbers");
        assert_eq!(result, Cost::ONE)
    }

    #[test]
    fn test_agg_mul_pair() {
        let result = CostAggregation::Mul
            .aggregate(&[("a", Cost::new(0.5)), ("b", Cost::new(0.5))])
            .expect("mul should not fail over any real numbers");
        assert_eq!(result, Cost::new(0.25))
    }
}
