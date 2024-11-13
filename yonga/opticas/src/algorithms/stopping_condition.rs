use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Trait to define a condition that causes an algorithm to terminate.
pub trait OStoppingCondition<T: PartialOrd> {
    /// The target value of the stopping condition.
    fn target(&self) -> T;

    /// Whether the stopping condition is met.
    fn is_met(&self, current: T) -> bool {
        self.target() <= current
    }

    /// A name describing the stopping condition.
    fn name() -> String;
}

/// Number of generations after which a genetic algorithm terminates.
#[derive(Serialize, Deserialize, Clone)]
pub struct OMaxGenerationValue(pub usize);

impl OStoppingCondition<usize> for OMaxGenerationValue {
    fn target(&self) -> usize {
        self.0
    }

    fn name() -> String {
        "maximum number of generations".to_string()
    }
}

/// Number of function evaluations after which a genetic algorithm terminates.
#[derive(Serialize, Deserialize, Clone)]
pub struct OMaxFunctionEvaluationValue(pub usize);

impl OStoppingCondition<usize> for OMaxFunctionEvaluationValue {
    fn target(&self) -> usize {
        self.0
    }

    fn name() -> String {
        "maximum number of function evaluations".to_string()
    }
}

/// Elapsed time after which a genetic algorithm terminates.
#[derive(Serialize, Deserialize, Clone)]
pub struct OMaxDurationValue(pub Duration);

impl OStoppingCondition<Duration> for OMaxDurationValue {
    fn target(&self) -> Duration {
        self.0
    }

    fn name() -> String {
        "maximum duration".to_string()
    }
}

/// The type of stopping condition. Pick one type to inform the algorithm how/when it should
/// terminate the population evolution.
#[derive(Serialize, Deserialize, Clone)]
pub enum OStoppingConditionType {
    /// Set a maximum duration
    MaxDuration(OMaxDurationValue),
    /// Set a maximum number of generations
    MaxGeneration(OMaxGenerationValue),
    /// Set a maximum number of function evaluations
    MaxFunctionEvaluations(OMaxFunctionEvaluationValue),
    /// Stop when at least on condition is met (this acts as an OR operator)
    Any(Vec<OStoppingConditionType>),
    /// Stop when all conditions are met (this acts as an AND operator)
    All(Vec<OStoppingConditionType>),
}

impl OStoppingConditionType {
    /// A name describing the stopping condition.
    ///
    /// returns: `String`
    pub fn name(&self) -> String {
        match self {
            OStoppingConditionType::MaxDuration(_) => OMaxDurationValue::name(),
            OStoppingConditionType::MaxGeneration(_) => OMaxGenerationValue::name(),
            OStoppingConditionType::MaxFunctionEvaluations(_) => OMaxFunctionEvaluationValue::name(),
            OStoppingConditionType::Any(s) => s
                .iter()
                .map(|cond| cond.name())
                .collect::<Vec<String>>()
                .join(" OR "),
            OStoppingConditionType::All(s) => s
                .iter()
                .map(|cond| cond.name())
                .collect::<Vec<String>>()
                .join(" AND "),
        }
    }

    /// Check whether the stopping condition is a vector and has nested vector in it.
    ///
    /// # Arguments
    ///
    /// * `conditions`: A vector of stopping conditions.
    ///
    /// returns: `bool`
    pub fn has_nested_vector(conditions: &[OStoppingConditionType]) -> bool {
        conditions.iter().any(|c| match c {
            OStoppingConditionType::Any(_) | OStoppingConditionType::All(_) => true,
            _ => false,
        })
    }
}
