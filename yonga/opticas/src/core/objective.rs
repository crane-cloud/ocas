use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

/// Whether the objective should be minimised or maximised. Default is minimise.
#[derive(Default, Clone, Copy, Debug, PartialOrd, PartialEq, Serialize, Deserialize)]
pub enum OObjectiveDirection {
    #[default]
    /// Minimise an objective.
    OMinimise,
    /// Maximise an objective.
    OMaximise,
}

impl Display for OObjectiveDirection {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OObjectiveDirection::OMinimise => f.write_str("minimised"),
            OObjectiveDirection::OMaximise => f.write_str("maximised"),
        }
    }
}

/// Define a problem objective to minimise or maximise.
///
/// # Example
/// ```
///  use optirustic::core::{OObjective, OObjectiveDirection};
///
///  let o = Objective::new("Reduce cost", OObjectiveDirection::OMinimise);
///  println!("{}", o);
/// ```
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OObjective {
    /// The objective name.
    name: String,
    /// Whether the objective should be minimised or maximised.
    direction: OObjectiveDirection,
}

impl OObjective {
    /// Create a new objective.
    ///
    /// # Arguments
    ///
    /// * `name`: The objective name.
    /// * `direction`:  Whether the objective should be minimised or maximised.
    ///
    /// returns: `Objective`
    pub fn new(name: &str, direction: OObjectiveDirection) -> Self {
        Self {
            name: name.to_string(),
            direction,
        }
    }

    /// Get the objective name.
    ///
    /// return: `String`
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Get the objective direction.
    ///
    /// return: `ObjectiveDirection`
    pub fn direction(&self) -> OObjectiveDirection {
        self.direction
    }
}

impl Display for OObjective {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Objective '{}' is {}", self.name, self.direction)
    }
}
