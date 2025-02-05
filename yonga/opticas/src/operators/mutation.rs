use rand::{Rng, RngCore};
use serde::{Deserialize, Serialize};

use crate::core::{OIndividual, OOError, OProblem, OVariableType, OVariableValue};

/// The trait to implement a mutation operator to modify the genetic material of an individual.
pub trait OMutation {
    /// Mutate a population individual.
    ///
    /// # Arguments
    ///
    /// * `individual`: The individual to mutate.
    /// * `rng`: The random number generator.
    ///
    /// returns: `Result<Individual, OError>`. The mutated individual.
    fn mutate_offspring(
        &self,
        individual: &OIndividual,
        rng: &mut dyn RngCore,
    ) -> Result<OIndividual, OOError>;
}

/// Input arguments for [`OPolynomialMutation`].
#[derive(Serialize, Deserialize, Clone)]
pub struct OPolynomialMutationArgs {
    /// A user-defined parameter to control the mutation. This is eta_m in the paper, and it is
    /// suggested its value to be in the [20, 100] range.
    pub index_parameter: f64,
    /// The probability of mutating a parent variable.
    pub variable_probability: f64,
}

impl OPolynomialMutationArgs {
    /// Initialise the Polynomial mutation (PM) operator with the default parameters. With a
    /// distribution index or index parameter of `20` and variable probability equal `1` divided by
    /// the number of real variables in the problem (i.e. each variable will have the same
    /// probability of being mutated).
    ///
    /// # Arguments
    ///
    /// * `problem`: THe problem being solved.
    ///
    /// returns: `Self`
    pub fn default(problem: &OProblem) -> Self {
        let num_real_vars = problem
            .variables()
            .iter()
            .filter(|(_, v)| v.is_choice())
            .count() as f64;
        let variable_probability = 1.0 / num_real_vars;
        Self {
            index_parameter: 20.0,
            variable_probability,
        }
    }
}

/// The Polynomial mutation (PM) operator.
///
/// Adapted from [Deb & Deb (2014)](https://dl.acm.org/doi/10.1504/IJAISC.2014.059280), full
/// text available at <https://www.egr.msu.edu/~kdeb/papers/k2012016.pdf>.
///
/// # Integer support
/// Since the original method does not provide support for integer variables, this has been added by
/// using the approach proposed in:
/// > Deep, Kusum & Singh, Krishna & Kansal, M. & Mohan, Chander. (2009). A real coded genetic
/// > algorithm for solving integer and mixed integer optimization problems. Applied Mathematics
/// > and Computation. 212. 505-518. 10.1016/j.amc.2009.02.044.
///
/// See the truncation procedure in section 2.4 in the [full text](https://www.researchgate.net/publication/220557819_A_real_coded_genetic_algorithm_for_solving_integer_and_mixed_integer_optimization_problems),
/// where a probability of `0.5` is applied to ensure randomness in the integer crossover.
///
/// # Example
///
/// ```
/// use std::error::Error;
/// use optirustic::core::{BoundedNumber, Individual, Problem, OVariableType, OVariableValue,
/// Objective, Constraint, ObjectiveDirection, RelationalOperator, EvaluationResult, Evaluator};
/// use optirustic::operators::{Crossover, PolynomialMutationArgs, PolynomialMutation, Mutation};
/// use std::sync::Arc;
/// use rand_chacha::ChaCha8Rng;
/// use rand::SeedableRng;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     // create a new one-variable
///     let objectives = vec![Objective::new("obj1", ObjectiveDirection::Minimise)];
///     let variables = vec![
///         OVariableType::Real(BoundedNumber::new("var1", 0.0, 1000.0)?),
///         OVariableType::Integer(BoundedNumber::new("var2", -1, 1)?)
///     ];
///     
///     // dummy evaluator function
///     #[derive(Debug)]
///     struct UserEvaluator;
///     impl Evaluator for UserEvaluator {
///         fn evaluate(&self, _: &Individual) -> Result<EvaluationResult, Box<dyn Error>> {
///             Ok(EvaluationResult {
///                 constraints: Default::default(),
///                 objectives: Default::default(),
///             })
///         }
///     }
///     let problem = Arc::new(Problem::new(objectives, variables, None, Box::new(UserEvaluator))?);
///
///     // add new individuals
///     let mut a = Individual::new(problem.clone());
///     a.update_variable("var1", OVariableValue::Real(0.2))?;
///     a.update_variable("var2", OVariableValue::Integer(1))?;
///
///     // crossover
///     let parameters = PolynomialMutationArgs {
///         index_parameter: 1.0 ,
///         variable_probability: 1.0,
///     };
///     let pm = PolynomialMutation::new(parameters)?;
///     let mut rng = ChaCha8Rng::from_seed(Default::default());
///     let out = pm.mutate_offspring(&a, &mut rng)?;
///     println!("{}", out);
///     Ok(())
/// }
/// ```
pub struct OPolynomialMutation {
    /// The user-defined parameter to control the mutation.
    index_parameter: f64,
    /// The probability of mutating a parent variable.
    variable_probability: f64,
}

impl OPolynomialMutation {
    /// Initialise the Polynomial mutation (PM) operator. This returns an error if the probability
    /// is outside the [0, 1] range.
    ///
    /// # Arguments
    ///
    /// * `index_parameter`:
    /// * `variable_probability`: The probability of mutating a parent variable.
    ///
    /// returns: `Result<PolynomialMutation, OError>`
    pub fn new(args: OPolynomialMutationArgs) -> Result<Self, OOError> {
        if !(0.0..=1.0).contains(&args.variable_probability) {
            return Err(OOError::MutationOperator(
                "PolynomialMutation".to_string(),
                format!(
                    "The variable probability {} must be a number between 0 and 1",
                    args.variable_probability
                ),
            ));
        }
        Ok(Self {
            index_parameter: args.index_parameter,
            variable_probability: args.variable_probability,
        })
    }

    /// Perform the mutation of a real variable for an offspring.
    ///
    /// # Arguments
    ///
    /// * `y`: The real variable value to mutate.
    /// * `y_lower`: The variable lower bound.
    /// * `y_upper`: The variable lower bound.
    /// * `rng`: The random number generator reference.
    ///
    /// returns: `f64`
    fn mutate_variable(&self, y: f64, y_lower: f64, y_upper: f64, rng: &mut dyn RngCore) -> f64 {
        let delta_y = y_upper - y_lower;
        let prob = rng.gen_range(0.0..=1.0);

        // this is delta_l or delta_r
        let delta = if prob <= 0.5 {
            let bl = (y - y_lower) / delta_y;
            let b =
                2.0 * prob + (1.0 - 2.0 * prob) * f64::powf(1.0 - bl, self.index_parameter + 1.0);
            f64::powf(b, 1.0 / (self.index_parameter + 1.0)) - 1.0
        } else {
            let bu = (y_upper - y) / delta_y;
            let b = 2.0 * (1.0 - prob)
                + 2.0 * (prob - 0.5) * f64::powf(1.0 - bu, self.index_parameter + 1.0);
            1.0 - f64::powf(b, 1.0 / (self.index_parameter + 1.0))
        };

        // adjust the variable
        let new_y = y + delta * delta_y;
        f64::min(f64::max(new_y, y_lower), y_upper)
    }
}

impl OMutation for OPolynomialMutation {
    fn mutate_offspring(
        &self,
        individual: &OIndividual,
        rng: &mut dyn RngCore,
    ) -> Result<OIndividual, OOError> {
        let mut mutated_individual = individual.clone_variables();
        let problem = individual.problem();

        // Check that all variables are either real, integer, or Choice
        if !problem
            .variables()
            .iter()
            .all(|(_, v)| v.is_choice()) // Ensure Choice support
        {
            return Err(OOError::CrossoverOperator(
                "PolynomialMutation".to_string(),
                "The PM operator only works with OChoice variables".to_string(),
            ));
        }

        for (var_name, var_type) in problem.variables() {
            if rng.gen_range(0.0..=1.0) <= self.variable_probability {
                let value = individual.get_variable_value(&var_name)?;
                match (value, var_type) {

                    (OVariableValue::OChoice(_z), OVariableType::OChoice(choice)) => {
                        let choices = choice.choices();
                        let idx = rng.gen_range(0..choices.len());

                        mutated_individual.update_variable(&var_name, OVariableValue::OChoice(choices[idx].clone()))?;

                    }

                    _ => {} // Handle unsupported types if necessary
                }
            }
        }

        Ok(mutated_individual)
    }
}

#[cfg(test)]
mod test {
    use std::sync::Arc;

    use crate::core::utils::{dummy_evaluator, get_rng};
    use crate::core::{
        BoundedNumber, Individual, Objective, ObjectiveDirection, Problem, OVariableType,
        OVariableValue,
    };
    use crate::operators::{Mutation, PolynomialMutation, PolynomialMutationArgs};

    #[test]
    /// Test that the PM operator mutates variables
    fn test_sbx_crossover() {
        let objectives = vec![Objective::new("obj1", ObjectiveDirection::Minimise)];

        let variables = vec![
            OVariableType::Real(BoundedNumber::new("var1", 0.0, 1000.0).unwrap()),
            OVariableType::Integer(BoundedNumber::new("var2", -10, 20).unwrap()),
        ];

        let problem =
            Arc::new(Problem::new(objectives, variables, None, dummy_evaluator()).unwrap());

        // add new individuals
        let mut a = Individual::new(problem.clone());
        a.update_variable("var1", OVariableValue::Real(0.2)).unwrap();
        a.update_variable("var2", OVariableValue::Integer(0))
            .unwrap();
        let mut b = Individual::new(problem.clone());
        b.update_variable("var1", OVariableValue::Real(0.8)).unwrap();
        b.update_variable("var2", OVariableValue::Integer(3))
            .unwrap();

        let args = PolynomialMutationArgs {
            // ensure different variable value (with integers)
            index_parameter: 1.0,
            // always force mutation
            variable_probability: 1.0,
        };
        let pm = PolynomialMutation::new(args).unwrap();
        let mut rng = get_rng(Some(1));
        let mutated_offspring = pm.mutate_offspring(&a, &mut rng).unwrap();

        // Mutation always performed because variable_probability is 1
        assert_ne!(
            *mutated_offspring.get_variable_value("var1").unwrap(),
            OVariableValue::Real(0.2)
        );
        assert_ne!(
            *mutated_offspring.get_variable_value("var2").unwrap(),
            OVariableValue::Integer(0)
        );
    }
}
