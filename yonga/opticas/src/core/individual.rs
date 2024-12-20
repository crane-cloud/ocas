use std::collections::HashMap;
use std::fmt::{Display, Formatter};
use std::ops::RangeBounds;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::core::{ODataValue, OOError, OProblem, OVariableValue};
use crate::utils::hasmap_eq_with_nans;

/// An individual in the population containing the problem solution, and the objective and
/// constraint values.
///
/// # Example
/// ```
/// use std::error::Error;
/// use optirustic::core::{BoundedNumber, Constraint, OIndividual, Problem, Objective,
/// ObjectiveDirection, ORelationalOperator, EvaluationResult, Evaluator, VariableType, VariableValue};
/// use std::sync::Arc;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let objectives = vec![Objective::new("obj1", ObjectiveDirection::Minimise)];
///
///     let var_types = vec![VariableType::Real(BoundedNumber::new("var1", 0.0, 2.0)?)];
///     let constraints = vec![Constraint::new("C1", ORelationalOperator::EqualTo, 5.0)];
///
///     // dummy evaluator function
///     #[derive(Debug)]
///     struct UserEvaluator;
///     impl Evaluator for UserEvaluator {
///         fn evaluate(&self, _: &OIndividual) -> Result<EvaluationResult, Box<dyn Error>> {
///             Ok(EvaluationResult {
///                 constraints: Default::default(),
///                 objectives: Default::default(),
///             })
///         }
///     }
///     // create a new one-variable problem
///     let problem = Arc::new(Problem::new(objectives, var_types, Some(constraints), Box::new(UserEvaluator))?);
///
///     // create an individual and set the calculated variable
///     let mut a = OIndividual::new(problem.clone());
///     a.update_variable("var1", OVariableValue::Real(0.2))?;
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone)]
pub struct OIndividual {
    /// The problem being solved
    problem: Arc<OProblem>,
    /// The value of the problem variables for the individual.
    variable_values: HashMap<String, OVariableValue>,
    /// The value of the constraints.
    constraint_values: HashMap<String, (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)>,
    /// The values of the objectives.
    objective_values: HashMap<String, f64>,
    /// Whether the individual has been evaluated and the problem constraint and objective values
    /// are available. When an individual is created with some variables after the population
    /// evolves, constraints and objectives need to be evaluated using a user-defined function.
    evaluated: bool,
    /// Additional numeric data to store for the individuals (such as crowding distance or rank)
    /// depending on the algorithm the individuals are derived from.
    data: HashMap<String, ODataValue>,
}

impl PartialEq for OIndividual {
    /// Compare two individual's constraints, variables, objectives and stored data.
    ///
    /// # Arguments
    ///
    /// * `other`: The other individual to compare.
    ///
    /// returns: `bool`
    fn eq(&self, other: &Self) -> bool {
        self.variable_values == other.variable_values
            // && hasmap_eq_with_nans(&self.constraint_values, &other.constraint_values)
            && hasmap_eq_with_nans(&self.objective_values, &other.objective_values)
            && self.data == other.data
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OIndividualExport {
    /// The value of the constraints.
    pub constraint_values: HashMap<String, (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)>,
    /// The values of the objectives.
    pub objective_values: HashMap<String, f64>,
    /// The overall amount of violation of the solution constraints.
    pub constraint_violation: u64,
    /// The value of the problem variables for the individual.
    pub variable_values: HashMap<String, OVariableValue>,
    /// Whether the solution meets all the problem constraints.
    pub is_feasible: bool,
    /// Whether the individual has been evaluated and the problem constraint and objective values
    /// are available.
    pub evaluated: bool,
    /// Additional numeric data to store for the individuals (such as crowding distance or rank)
    /// depending on the algorithm the individuals are derived from.
    pub data: HashMap<String, ODataValue>,
}

impl Display for OIndividual {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Individual(variables={:?}, objectives={:?},constraints={:?})",
            self.variable_values, self.objective_values, self.constraint_values,
        )
    }
}

impl OIndividual {
    /// Create a new individual. An individual contains the solution after an evolution.
    ///
    /// # Arguments
    ///
    /// * `problem`: The problem being solved.
    ///
    /// returns: `Individual`
    pub fn new(problem: Arc<OProblem>) -> Self {
        let mut variable_values: HashMap<String, OVariableValue> = HashMap::new();
        for (variable_name, var_type) in problem.variables() {
            variable_values.insert(variable_name, var_type.generate_random_value());
        }

        let mut objective_values: HashMap<String, f64> = HashMap::new();
        for objective_name in problem.objective_names() {
            objective_values.insert(objective_name, f64::NAN);
        }

        let mut constraint_values: HashMap<String, (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)> = HashMap::new();
        for constraint_name in problem.constraint_names() {
            constraint_values.insert(constraint_name, (None, None, None));
        }

        Self {
            problem,
            variable_values,
            constraint_values,
            objective_values,
            evaluated: false,
            data: HashMap::new(),
        }
    }

    /// Get the problem being solved with the individual.
    ///
    /// return `Arc<Problem>`
    pub fn problem(&self) -> Arc<OProblem> {
        self.problem.clone()
    }

    /// Clone an individual by preserving only its solutions.
    ///
    /// return: `Individual`
    pub(crate) fn clone_variables(&self) -> Self {
        let mut i = Self::new(self.problem.clone());
        for (var_name, var_value) in self.variable_values.iter() {
            i.update_variable(var_name, var_value.clone()).unwrap()
        }
        i
    }

    /// Update the variable for a solution. This returns an error if the variable name does not
    /// exist or the variable value does not match the variable type set on the problem (for
    /// example [`VariableValue::Integer`] is provided but the type is [`crate::core::VariableType::Real`]).
    ///
    /// # Arguments
    ///
    /// * `name`: The variable to update.
    /// * `value`: The value to set.
    ///
    /// returns: `Result<(), OError>`
    pub fn update_variable(&mut self, name: &str, value: OVariableValue) -> Result<(), OOError> {
        if !self.variable_values.contains_key(name) {
            return Err(OOError::NonExistingName(
                "variable".to_string(),
                name.to_string(),
            ));
        }
        if !value.match_type(name, self.problem.clone())? {
            return Err(OOError::NonMatchingVariableType(name.to_string()));
        }
        if let Some(x) = self.variable_values.get_mut(name) {
            *x = value;
        }
        Ok(())
    }

    /// Update the objective for a solution. The value is saved as negative if the objective being
    /// updated is being maximised. This returns an error if the name does not exist in the problem.
    ///
    /// # Arguments
    ///
    /// * `name`: The objective to update.
    /// * `value`: The value to set.
    ///
    /// returns: `Result<(), OError>`
    pub fn update_objective(&mut self, name: &str, value: f64) -> Result<(), OOError> {
        if !self.objective_values.contains_key(name) {
            return Err(OOError::NonExistingName(
                "objective".to_string(),
                name.to_string(),
            ));
        }
        if value.is_nan() {
            return Err(OOError::NaN("objective".to_string(), name.to_string()));
        }

        // invert the sign for maximisation problems
        let sign = match self.problem.is_objective_minimised(name)? {
            true => 1.0,
            false => -1.0,
        };
        if let Some(x) = self.objective_values.get_mut(name) {
            *x = sign * value;
        }
        Ok(())
    }

    /// Update a constraint.
    ///
    /// # Arguments
    ///
    /// * `name`: The constraint to update.
    /// * `value`: The value to set.
    ///
    /// returns: `Result<(), OError>`
    pub(crate) fn update_constraint(&mut self, name: &str, value: (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)) -> Result<(), OOError> {
        if !self.constraint_values.contains_key(name) {
            return Err(OOError::NonExistingName(
                "constraint".to_string(),
                name.to_string(),
            ));
        }
        // if value.is_nan() {
        //     return Err(OError::NaN("constraint".to_string(), name.to_string()));
        // }
        if let Some(x) = self.constraint_values.get_mut(name) {
            *x = value;
        }
        Ok(())
    }

    /// Calculate the overall amount of violation of the solution constraints. This is a measure
    /// about how close (or far) the individual meets the constraints. If the solution is feasible,
    /// then the violation is 0.0. Otherwise, a positive number is returned.
    ///
    /// return: `f64`
    pub fn constraint_violation(&self) -> u64 {
        // return self
        //     .problem
        //     .constraints()
        //     .iter()
        //     .map(|(name, c)| c.constraint_violation(self.constraint_values[name]))
        //     .sum();
        // return self
        //     .problem
        //     .constraint_names()
        //     .iter()
        //     .map(|name| {
        //         let (violation, _) = self.constraint_values[name];
        //         violation.unwrap_or(0)
        //     })
        //     .sum();
        return self
            .problem
            .constraint_names()
            .iter()
            .map(|name| {
                let (violation, _, _) = self.constraint_values[name];
                violation.unwrap_or(0)
            })
            .sum();
    }

    /// Return whether the solution meets all the problem constraints.
    ///
    /// return: `bool`
    pub fn is_feasible(&self) -> bool {
        for (name, constraint_value) in self.constraint_values.iter() {
            if !self.problem.constraint_names().contains(name) {
                continue;
            }
            if !self
                .problem
                .get_constraint(name)
                .unwrap()
                .is_met(constraint_value.clone())
            {
                return false;
            }
        }
        true
    }

    /// Ge all the variables.
    ///
    /// returns: `HashMap<String, VariableValue>`
    pub fn variables(&self) -> HashMap<String, OVariableValue> {
        self.variable_values.clone()
    }

    /// Get all the constraints.
    ///
    /// returns: `HashMap<String, f64>`
    pub fn constraints(&self) -> HashMap<String, (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)> {
        self.constraint_values.clone()
    }

    /// Get all the objectives.
    ///
    /// returns: `HashMap<String, f64>`
    pub fn objectives(&self) -> HashMap<String, f64> {
        self.objective_values.clone()
    }

    /// Ge the variable value by name. This return an error if the variable name does not exist.
    ///
    /// # Arguments
    ///
    /// * `name`: The variable name.
    ///
    /// returns: `Result<&VariableValue, OError>`
    pub fn get_variable_value(&self, name: &str) -> Result<&OVariableValue, OOError> {
        if !self.variable_values.contains_key(name) {
            return Err(OOError::NonExistingName(
                "variable".to_string(),
                name.to_string(),
            ));
        }

        Ok(&self.variable_values[name])
    }

    /// Get the vector with the variable values for the individual.
    ///
    /// returns: `Result<Vec<&VariableValue>, OError>`
    pub fn get_variable_values(&self) -> Result<Vec<&OVariableValue>, OOError> {
        self.problem
            .variable_names()
            .iter()
            .map(|var_name| self.get_variable_value(var_name))
            .collect()
    }

    /// Get the constraint value by name. This return an error if the constraint name does not exist.
    ///
    /// # Arguments
    ///
    /// * `name`: The constraint name.
    ///
    /// returns: `Result<f64, OError>`
    pub fn get_constraint_value(&self, name: &str) -> Result<(Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>), OOError> {
        if !self.constraint_values.contains_key(name) {
            return Err(OOError::NonExistingName(
                "constraint".to_string(),
                name.to_string(),
            ));
        }

        Ok(self.constraint_values[name].clone())
    }



    /// Get the objective value by name. This returns an error if the objective does not exist.
    ///
    /// # Arguments
    ///
    /// * `name`: The objective name.
    ///
    /// returns: `Result<f64, OError>`
    pub fn get_objective_value(&self, name: &str) -> Result<f64, OOError> {
        if !self.objective_values.contains_key(name) {
            return Err(OOError::NonExistingName(
                "objective".to_string(),
                name.to_string(),
            ));
        }

        Ok(self.objective_values[name])
    }

    /// Ge the vector with the objective values for the individual. The size of the vector will
    /// equal the number of problem objectives.
    ///
    /// returns: `Result<Vec<f64>, OError>`
    pub fn get_objective_values(&self) -> Result<Vec<f64>, OOError> {
        self.problem
            .objective_names()
            .iter()
            .map(|obj_name| self.get_objective_value(obj_name))
            .collect()
    }

    /// Ge the vector with the objective values for the individual and transform their value using
    /// a closure. The size of the vector will equal the number of problem objectives.
    ///
    /// # Arguments
    ///
    /// * `transform`: The function to apply to transform each objective value. This function
    ///    receives the objective value and its name.
    ///
    /// returns: `Result<Vec<f64>, OError>`
    pub fn transform_objective_values<F: Fn(f64, String) -> Result<f64, OOError>>(
        &self,
        transform: F,
    ) -> Result<Vec<f64>, OOError> {
        self.problem
            .objective_names()
            .iter()
            .map(|obj_name| {
                let val = self.get_objective_value(obj_name)?;
                transform(val, obj_name.clone())
            })
            .collect()
    }

    /// Check if the individual was evaluated.
    ///
    /// return: `bool`
    pub fn is_evaluated(&self) -> bool {
        self.evaluated
    }

    /// Set the individual as evaluated. This means that its constraints and objectives have been
    /// calculated for its solution.
    pub fn set_evaluated(&mut self) {
        self.evaluated = true;
    }

    /// Get all the individual's data.
    ///
    /// returns: `HashMap<String, DataValue>`
    pub fn data(&self) -> HashMap<String, ODataValue> {
        self.data.clone()
    }

    /// Store custom data on the individual.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the data.
    /// * `value`: The value.
    ///
    /// returns: `()`.
    pub fn set_data(&mut self, name: &str, value: ODataValue) {
        self.data.insert(name.to_string(), value);
    }

    /// Get a copy of the custom data set on the individual. This returns an error if no custom
    /// data with the provided `name` is set on the individual.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the data.
    ///
    /// returns: `Result<DataValue, OError>`
    pub fn get_data(&self, name: &str) -> Result<ODataValue, OOError> {
        self.data
            .get(name)
            .cloned()
            .ok_or(OOError::WrongDataName(name.to_string()))
    }

    /// Export all the solution data (constraint and objective values, constraint violation and
    /// feasibility).
    ///
    /// return: `IndividualExport`
    pub fn serialise(&self) -> OIndividualExport {
        // invert maximised objective for user
        let mut objective_values = self.objective_values.clone();
        for name in self.problem.objective_names() {
            match self.problem.is_objective_minimised(&name) {
                Ok(is_minimised) => {
                    if !is_minimised {
                        *objective_values.get_mut(&name).unwrap() *= -1.0;
                    }
                }
                Err(_) => continue,
            }
        }

        OIndividualExport {
            constraint_values: self.constraint_values.clone(),
            objective_values,
            constraint_violation: self.constraint_violation(),
            variable_values: self.variable_values.clone(),
            is_feasible: self.is_feasible(),
            evaluated: self.evaluated,
            data: self.data.clone(),
        }
    }

    /// Import the individual's objectives, variables and constraints.
    ///
    /// # Arguments
    ///
    /// * `data`: The data.
    /// * `problem`: The problem being solved.
    ///
    /// returns: `Result<Individual, OError>`
    pub fn deserialise(data: &OIndividualExport, problem: Arc<OProblem>) -> Result<Self, OOError> {
        let mut ind = OIndividual::new(problem.clone());

        for (var_name, var_value) in data.variable_values.iter() {
            ind.update_variable(var_name, var_value.clone())?;
        }
        for (obj_name, obj_value) in data.objective_values.iter() {
            ind.update_objective(obj_name, *obj_value)?;
        }
        for (const_name, const_value) in data.constraint_values.iter() {
            ind.update_constraint(const_name, const_value.clone())?;
        }
        ind.set_evaluated();
        Ok(ind)
    }
}

/// The population with the solutions.
#[derive(Clone, Default, Debug)]
pub struct OPopulation(Vec<OIndividual>);

impl OPopulation {
    /// Initialise a population with no individuals.
    ///
    /// returns: `Self`
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialise a population with some individuals.
    ///
    /// # Arguments
    ///
    /// * `individual`: The vector of individuals to add.
    ///
    /// returns: `Self`
    pub fn new_with(individuals: Vec<OIndividual>) -> Self {
        Self(individuals)
    }

    /// Get the population size.
    ///
    /// return: `usize`
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Return `true` if the population is empty.
    ///
    /// return: `bool`
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get the population individuals.
    ///
    /// return: `&[Individual]`
    pub fn individuals(&self) -> &[OIndividual] {
        self.0.as_ref()
    }

    /// Get a population individual by its index.
    ///
    /// return: `Option<&Individual>`
    pub fn individual(&self, index: usize) -> Option<&OIndividual> {
        self.0.get(index)
    }

    /// Borrow the population individuals as mutable reference.
    ///
    /// return: `&mut [Individual]`
    pub fn individuals_as_mut(&mut self) -> &mut [OIndividual] {
        self.0.as_mut()
    }

    /// Add new individuals to the population.
    ///
    /// # Arguments
    ///
    /// * `individuals`: The vector of individuals to add.
    ///
    /// returns: `()`
    pub fn add_new_individuals(&mut self, individuals: Vec<OIndividual>) {
        self.0.extend(individuals);
    }

    /// Add a new individual to the population.
    ///
    /// # Arguments
    ///
    /// * `individual`: The individual to add.
    ///
    /// returns: `()`
    pub fn add_individual(&mut self, individual: OIndividual) {
        self.0.push(individual);
    }

    /// Remove the specified range from the population in bulk and return all removed elements.
    ///
    /// # Arguments
    ///
    /// * `range_to_remove`: The range to remove.
    ///
    /// returns: `Vec<Individual>`
    pub fn drain<R>(&mut self, range_to_remove: R) -> Vec<OIndividual>
    where
        R: RangeBounds<usize>,
    {
        self.0.drain(range_to_remove).collect()
    }

    /// Generate a population with a number of individuals equal to `number_of_individuals`. All
    /// variable values for all individuals are initialised to an initial value depending on the
    /// variable type (for example min for a bounded real variable).  
    ///
    /// # Arguments
    ///
    /// * `problem`: The problem being solved.
    /// * `number_of_individuals`: The number of individuals to add to the population.
    ///
    /// returns: `Population`
    pub fn init(problem: Arc<OProblem>, number_of_individuals: usize) -> Self {
        let mut population: Vec<OIndividual> = vec![];
        for _ in 0..number_of_individuals {
            population.push(OIndividual::new(problem.clone()));
        }
        Self(population)
    }

    /// Serialise the individuals for export.
    ///
    /// return: `Vec<IndividualExport>`
    pub fn serialise(&self) -> Vec<OIndividualExport> {
        self.0.iter().map(|i| i.serialise()).collect()
    }

    /// Import the population exported to a JSON file.
    ///
    /// # Arguments
    ///
    /// * `data`: The vector of [`IndividualExport`].
    /// * `problem`: The problem.
    ///
    /// returns: `Result<Population, OError>`
    pub fn deserialise(
        data: &[OIndividualExport],

        problem: Arc<OProblem>,
    ) -> Result<OPopulation, OOError> {
        // Import data
        let individuals = data
            .iter()
            .map(|d| OIndividual::deserialise(d, problem.clone()))
            .collect::<Result<Vec<OIndividual>, OOError>>()?;
        Ok(OPopulation::new_with(individuals))
    }
}

pub trait OIndividuals {
    fn individual(&self, index: usize) -> Result<&OIndividual, OOError>;
    fn objective_values(&self, name: &str) -> Result<Vec<f64>, OOError>;
    //fn to_real_vec(&self, name: &str) -> Result<Vec<f64>, OOError>;
}

pub trait OIndividualsMut {
    fn individual_as_mut(&mut self, index: usize) -> Result<&mut OIndividual, OOError>;
}

macro_rules! impl_individuals {
    ( $($type:ty),* $(,)? ) => {
        $(
            impl OIndividuals for $type {
                /// Get an individual from a vector.
                ///
                /// # Arguments
                ///
                /// * `index`: The index of the individual.
                ///
                /// return: `Result<&Individual, OError>`
                fn individual(&self, index: usize) -> Result<&OIndividual, OOError> {
                    self.get(index)
                        .ok_or(OOError::NonExistingIndex("individual".to_string(), index))
                }

                /// Get the objective values for all individuals. This returns an error if the objective name
                /// does not exist.
                ///
                /// # Arguments
                ///
                /// * `name`: The objective name.
                ///
                /// returns: `Result<f64, OError>`
                fn objective_values(&self, name: &str) -> Result<Vec<f64>, OOError> {
                    self.iter().map(|i| i.get_objective_value(name)).collect()
                }

            }
        )*
    };
}

impl OIndividualsMut for &mut [OIndividual] {
    /// Get a population individual as mutable.
    ///
    /// # Arguments
    ///
    /// * `index`: The index of the individual.
    ///
    /// return: `Result<&mut Individual, OError>`
    fn individual_as_mut(&mut self, index: usize) -> Result<&mut OIndividual, OOError> {
        self.get_mut(index)
            .ok_or(OOError::NonExistingIndex("individual".to_string(), index))
    }
}

// Implement methods for individuals for different types
impl_individuals!(&[OIndividual]);
impl_individuals!(&mut [OIndividual]);
impl_individuals!(Vec<OIndividual>);

// #[cfg(test)]
// mod test {
//     use std::sync::Arc;

//     use crate::core::utils::dummy_evaluator;
//     use crate::core::{
//         BoundedNumber, Constraint, Individual, Objective, ObjectiveDirection, Problem,
//         ORelationalOperator, VariableType,
//     };

//     #[test]
//     /// Test when an objective does not exist
//     fn test_non_existing_data() {
//         let objectives = vec![Objective::new("objX", ObjectiveDirection::Minimise)];
//         let var_types = vec![VariableType::Real(
//             BoundedNumber::new("X1", 0.0, 2.0).unwrap(),
//         )];
//         let e = dummy_evaluator();

//         let problem = Arc::new(Problem::new(objectives, var_types, None, e).unwrap());
//         let mut solution1 = Individual::new(problem);

//         assert!(solution1.update_objective("obj1", 5.0).is_err());
//         assert!(solution1.get_objective_value("obj1").is_err());
//     }

//     #[test]
//     /// The is_feasible and constraint violation
//     fn test_feasibility() {
//         let objectives = vec![Objective::new("obj1", ObjectiveDirection::Minimise)];
//         let variables = vec![VariableType::Real(
//             BoundedNumber::new("X1", 0.0, 2.0).unwrap(),
//         )];
//         let constraints = vec![
//             Constraint::new("c1", ORelationalOperator::EqualTo, 1.0),
//             Constraint::new("c2", ORelationalOperator::EqualTo, 599.0),
//         ];
//         let e = dummy_evaluator();
//         let problem = Arc::new(Problem::new(objectives, variables, Some(constraints), e).unwrap());

//         let mut solution1 = Individual::new(problem);
//         solution1.update_objective("obj1", 5.0).unwrap();

//         // Unfeasible solution
//         solution1.update_constraint("c1", 5.0).unwrap();
//         assert!(!solution1.is_feasible());

//         // Feasible solution
//         solution1.update_constraint("c1", 1.0).unwrap();
//         solution1.update_constraint("c2", 599.0).unwrap();
//         assert!(solution1.is_feasible());

//         // Total violation
//         solution1.update_constraint("c1", 2.0).unwrap();
//         solution1.update_constraint("c2", 600.0).unwrap();
//         assert_eq!(solution1.constraint_violation(), 2.0);
//     }
// }
