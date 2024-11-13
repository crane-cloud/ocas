use std::fmt;
use std::fmt::{Debug, Display, Formatter};
use std::sync::Arc;
use rand::prelude::IteratorRandom;
use serde::{Deserialize, Serialize};

use crate::core::{OOError, OProblem};

/// A trait to define a decision variable.
pub trait OVariable<T>: Display {
    /// Generate a new random value for the variable.
    fn generate(&self) -> T;
    /// Get the variable name
    fn name(&self) -> String;
}

/// A variable choice.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OChoice {
    /// The variable name.
    name: String,
    /// The list of choices.
    choices: Vec<u64>,
}

impl OChoice {
    /// Create a new list of choices.
    ///
    /// # Arguments
    ///
    /// * `name`: The variable name.
    /// * `choices`: The list of choices.
    ///
    /// returns: `Choice`
    pub fn new(name: &str, choices: Vec<u64>) -> Self {
        Self {
            name: name.to_string(),
            choices,
        }
    }

    pub fn choices(&self) -> Vec<u64> {
        self.choices.clone()
    }
}

impl Display for OChoice {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Choice '{}': {:?}", self.name, self.choices)
    }
}

impl OVariable<u64> for OChoice {
    /// Randomly pick a choice.
    fn generate(&self) -> u64 {
        let mut rng = rand::thread_rng();
        let choice_index = (0..self.choices.len()).choose(&mut rng).unwrap();
        self.choices[choice_index]
    }

    fn name(&self) -> String {
        self.name.clone()
    }
}

/// The types of variables to set on a problem.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OVariableType {

    OChoice(OChoice),
}

impl OVariableType {
    /// Generate a new random variable value based on its type.
    ///
    /// returns: `VariableValue`
    pub fn generate_random_value(&self) -> OVariableValue {
        match &self {
            OVariableType::OChoice(v) => OVariableValue::OChoice(v.generate()),
        }
    }

    /// Get the variable name.
    ///
    /// return: `String`
    pub fn name(&self) -> String {
        match self {
            OVariableType::OChoice(t) => t.name.clone(),
        }
    }

    pub fn label(&self) -> String {
        let label = match &self {
            OVariableType::OChoice(_) => "choice",
        };
        label.into()
    }

    /// Check if the variable is Choice.
    ///
    /// return: `bool`
    pub(crate) fn is_choice(&self) -> bool {
        matches!(self, OVariableType::OChoice(_))
    }

}

impl Display for OVariableType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OVariableType::OChoice(v) => write!(f, "{v}").unwrap(),
        };
        Ok(())
    }
}

/// The value of a variable to set on an individual.
#[derive(Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum OVariableValue {
    /// The value for a choice variable.
    OChoice(u64),
}

impl PartialEq for OVariableValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {

            (OVariableValue::OChoice(s), OVariableValue::OChoice(o)) => s == o,
            _ => false,
        }
    }
}

impl OVariableValue {
    /// Check if the variable value matches the variable type set on the problem. This return an
    /// error if the variable name does not exist in the problem.
    ///
    /// # Arguments
    ///
    /// * `name`: The name of the variable in the problem.
    /// * `problem`: The problem being solved.
    ///
    /// returns: `Result<bool, OError>`
    pub fn match_type(&self, name: &str, problem: Arc<OProblem>) -> Result<bool, OOError> {
        let value = match problem.get_variable(name)? {
            OVariableType::OChoice(_) => matches!(self, OVariableValue::OChoice(_)),
        };
        Ok(value)
    }

}

impl Debug for OVariableValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            OVariableValue::OChoice(v) => write!(f, "{v}").unwrap(),
        };
        Ok(())
    }
}
