use std::mem;

use log::{debug, warn};

use crate::core::{OIndividual, OIndividuals, OOError};
use crate::metrics::hv_wfg::wfg::{Optimisation, Wfg};
use crate::metrics::hypervolume::{check_args, check_ref_point_coordinate};
use crate::utils::fast_non_dominated_sort;

mod wfg;

/// Calculate the hyper-volume using the WFG algorithm proposed by [While et al. (2012)](http://dx.doi.org/10.1109/TEVC.2010.2077298)
/// for a problem with `d` objectives and `n` individuals.
///
/// **IMPLEMENTATION NOTES**:
/// 1) Dominated and unfeasible solutions are excluded using the NSGA2 [`crate::utils::fast_non_dominated_sort()`]
///    algorithm in order to get the Pareto front. As assumed in the paper, non-dominated points do
///    not contribute do the metric.
/// 2) The coordinates of maximised objectives of the reference point are multiplied by -1 as the
///    algorithm assumes all objectives are maximised.
#[derive(Debug)]
pub struct HyperVolumeWhile2012 {
    /// The individuals to use. The size of this vector corresponds to the individual size and the
    /// size of the nested vector corresponds to the number of problem objectives.
    individuals: Vec<Vec<f64>>,
    /// The reference point.
    reference_point: Vec<f64>,
    /// The name of this metric
    metric_name: String,
}

impl HyperVolumeWhile2012 {
    /// Calculate the hyper-volume using the WFG algorithm proposed by [While et al. (2012)](http://dx.doi.org/10.1109/TEVC.2010.2077298)
    /// for a problem with `d` objectives and `n` individuals.
    ///
    /// # Arguments
    ///
    /// * `individuals`: The list of individuals.
    /// * `reference_point`: The reference point.
    ///
    /// returns: `Result<HyperVolumeWhile2012, OError>`
    pub fn new(individuals: &mut [OIndividual], reference_point: &[f64]) -> Result<Self, OOError> {
        let metric_name = "Hyper-volume While et al. (2012)".to_string();
        // check sizes
        check_args(individuals, reference_point)
            .map_err(|e| OOError::Metric(metric_name.clone(), e))?;

        // the reference point must dominate all objectives
        let problem = individuals[0].problem();
        for (obj_idx, (obj_name, obj)) in problem.objectives().iter().enumerate() {
            check_ref_point_coordinate(
                &individuals.objective_values(obj_name)?,
                obj,
                reference_point[obj_idx],
                obj_idx + 1,
            )
            .map_err(|e| OOError::Metric(metric_name.clone(), e))?;
        }

        // get non-dominated front with feasible solutions only
        let num_individuals = individuals.len();
        let mut front_data = fast_non_dominated_sort(individuals, true)?;
        let individuals = mem::take(&mut front_data.fronts[0]);

        if num_individuals != individuals.len() {
            warn!("{} individuals were removed from the given data because they are dominated by all the other points", num_individuals - individuals.len());
        }

        // Collect objective values - invert the sign of minimised objectives
        let objective_values = individuals
            .iter()
            .map(|ind| ind.get_objective_values())
            .collect::<Result<Vec<Vec<f64>>, _>>()?;

        // flip sign of maximised coordinates for the reference point
        let mut ref_point = reference_point.to_vec();
        for (obj_idx, obj_name) in problem.objective_names().iter().enumerate() {
            if !problem.is_objective_minimised(obj_name)? {
                ref_point[obj_idx] *= -1.0;
            }
        }

        debug!("Using non-dominated front {:?}", objective_values);
        debug!("Reference point is {:?}", ref_point);

        Ok(Self {
            individuals: objective_values,
            reference_point: ref_point,
            metric_name,
        })
    }

    /// Calculate the hyper-volume.
    ///
    /// return: `Result<f64, OError>`
    pub fn compute(&mut self) -> Result<f64, OOError> {
        let wfg = Wfg::new(&self.individuals, &self.reference_point, Optimisation::O2);
        wfg.calculate()
            .map_err(|e| OOError::Metric(self.metric_name.clone(), e))
    }
}

#[cfg(test)]
mod test {
    use float_cmp::approx_eq;

    use crate::core::test_utils::individuals_from_obj_values_dummy;
    use crate::core::ObjectiveDirection;
    use crate::metrics::test_utils::parse_pagmo_test_data_file;
    use crate::metrics::HyperVolumeWhile2012;

    /// Run a test using a Pagmo file.
    ///
    /// # Arguments
    ///
    /// * `file`: The file name in the `test_data` folder.
    ///
    /// returns: ()
    fn assert_test_file(file: &str) {
        let all_test_data = parse_pagmo_test_data_file(file).unwrap();
        let obj_count = all_test_data.first().unwrap().reference_point.len();
        let objective_direction = vec![ObjectiveDirection::Minimise; obj_count];

        for (ti, test_data) in all_test_data.iter().enumerate() {
            let mut individuals = individuals_from_obj_values_dummy(
                &test_data.objective_values,
                &objective_direction,
                None,
            );
            let mut hv =
                HyperVolumeWhile2012::new(&mut individuals, &test_data.reference_point).unwrap();

            let calculated = hv.compute().unwrap();
            let expected = test_data.hyper_volume;
            if !approx_eq!(f64, calculated, expected, epsilon = 0.001) {
                panic!(
                    r#"assertion failed for test #{}: `(left approx_eq right)` left: `{:?}`, right: `{:?}`"#,
                    ti + 1,
                    calculated,
                    expected,
                )
            }
        }
    }

    #[test]
    // /// Test the `HyperVolumeWhile2012` struct using Pagmo c_max_t1_d5_n1024 test data.
    // /// See https://github.com/esa/pagmo2/tree/master/tests/hypervolume_test_data
    fn test_c_max_t1_d5_n1024() {
        assert_test_file("c_max_t1_d5_n1024");
    }

    #[test]
    /// Test the `HyperVolumeWhile2012` struct using Pagmo c_max_t100_d3_n128 test data.
    /// See https://github.com/esa/pagmo2/tree/master/tests/hypervolume_test_data
    fn test_c_max_t100_d3_n128() {
        assert_test_file("c_max_t100_d3_n128");
    }

    #[test]
    /// Test the `HyperVolumeWhile2012` struct using Pagmo c_max_t1_d3_n2048 test data.
    /// See https://github.com/esa/pagmo2/tree/master/tests/hypervolume_test_data
    fn test_c_max_t1_d3_n2048() {
        assert_test_file("c_max_t1_d3_n2048");
    }
}
