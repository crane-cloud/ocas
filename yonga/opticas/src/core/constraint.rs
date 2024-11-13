use std::fmt::{Display, Formatter};
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

/// Operator used to check a bounded constraint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ORelationalOperator {
    /// Value must equal the constraint value
    EqualTo,
    /// Value must not equal the constraint value
    NotEqualTo,
    /// Value must be less or equal to the constraint value
    LessOrEqualTo,
    /// Value must be less than the constraint value
    LessThan,
    /// Value must be greater or equal to the constraint value
    GreaterOrEqualTo,
    /// Value must be greater than the constraint value
    GreaterThan,
}


/// Operator used to check a bounded constraint on a service group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServiceGroupORelationalOperator {
    /// Value must equal the constraint value
    EqualTo,
}




/// Define a constraint where a value is compared with a relational operator as follows:
///  - Equality operator ([`ORelationalOperator::EqualTo`]): value == target
///  - Inequality operator ([`ORelationalOperator::NotEqualTo`]): value != target
///  - Greater than operator ([`ORelationalOperator::GreaterThan`]): value > target
///  - Greater or equal to operator ([`ORelationalOperator::GreaterOrEqualTo`]): value >= target
///  - less than operator ([`ORelationalOperator::LessThan`]): value < target
///  - less or equal to operator ([`ORelationalOperator::LessOrEqualTo`]): value <= target
///
/// # Example
///
/// ```
///   use optirustic::core::{Constraint, ORelationalOperator};
///   let c = Constraint::new("Z>=5.2",ORelationalOperator::GreaterOrEqualTo, 5.2);
///   assert_eq!(c.is_met(10.1), true);
///   assert_eq!(c.is_met(3.11), false);
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OConstraint {
    /// The constraint name.
    name: String,
    /// The relational operator to use to compare a value against the constraint target value.
    operator: ORelationalOperator,
    /// The constraint target.
    target: Option<u64>,
    /// Option Group
    service_group: Option<Vec<String>>,
    /// Option Resource
    resource: Option<HashMap<u64, (f64, f64, f64, f64)>> // target (cpu, mem, disk, net) - u64 is the target node id
    
}

impl OConstraint {
    /// Create a new relational constraint.
    ///
    /// # Arguments
    ///
    /// * `name`: The constraint name.
    /// * `operator`: The relational operator to use to compare a value against the constraint
    ///    target value.
    /// * `target`: The constraint target.
    ///
    /// returns: `Constraint`
    pub fn new(name: &str, operator: ORelationalOperator, target: Option<u64>, service_group: Option<Vec<String>>, resource: Option<HashMap<u64, (f64, f64, f64, f64)>>) -> Self {
        Self {
            name: name.to_owned(),
            operator,
            target,
            service_group,
            resource
        }
    }

    /// Create a new relational constraint with a scale and offset. The target is first scaled,
    /// then offset.
    ///
    /// # Arguments
    ///
    /// * `name`: The constraint name.
    /// * `operator`: The relational operator to use to compare a value against the constraint
    ///    target value.
    /// * `target`: The constraint target.
    /// * `scale`: Apply a scaling factor to the `target`.
    /// * `offset`: Apply an offset to the `target`.
    ///
    /// returns: `Constraint`
    pub fn new_with_modifiers(
        name: &str,
        operator: ORelationalOperator,
        target: Option<u64>,
        service_group: Option<Vec<String>>,
        resource: Option<HashMap<u64, (f64, f64, f64, f64)>>,
        scale: u64,
        offset: u64,
    ) -> Self {
        let target = match target {
            Some(t) => t,
            None => 0,
        };
        Self {
            name: name.to_owned(),
            operator,
            target: Some(target * scale + offset),
            service_group,
            resource
        }
    }

    /// Get the constraint name.
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// Check whether the constraint is met. This is assessed as follows:
    ///  - Equality operator ([`ORelationalOperator::EqualTo`]): value == target
    ///  - Inequality operator ([`ORelationalOperator::NotEqualTo`]): value != target
    ///  - Greater than operator ([`ORelationalOperator::GreaterThan`]): value > target
    ///  - Greater or equal to operator ([`ORelationalOperator::GreaterOrEqualTo`]): value >= target
    ///  - less than operator ([`ORelationalOperator::LessThan`]): value < target
    ///  - less or equal to operator ([`ORelationalOperator::LessOrEqualTo`]): value <= target
    ///
    /// # Arguments
    ///
    /// * `value`: The value to check against the constraint target.
    ///
    /// returns: `bool`
    pub fn is_met(&self, value: (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)) -> bool {

        if self.target.is_none() && self.service_group.is_none() && self.resource.is_none() {
            return true; // no constraint
        }

        if let Some(service_group) = &self.service_group {
            // all services in value.1 must have the same node id (u64)
            let mut node = None;
            for service in service_group {
                if let Some(node_id) = value.1.clone().unwrap().iter().find(|&x| x.contains_key(service)) {
                    if let Some(n) = node {
                        if n != node_id[service] {
                            return false;
                        }
                    } else {
                        node = Some(node_id[service]);
                    }
                } else {
                    return false;
                }
            }
            return true;            
        }

        else if let Some(resource) = &self.resource {
            // all resources in value.2 must be less than or equal to the target resource
            for (node_id, ava) in resource {
                if let Some(req) = value.2.clone().unwrap().get(node_id) {
                    if req.0 > ava.0 || req.1 > ava.1 || req.2 > ava.2 || req.3 > ava.3 {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            return true;
        }

        else {
            //println!("Checking constraint - the u64 (target) was provided");
            match self.operator {
                ORelationalOperator::EqualTo => {
                    if value.0.unwrap() == self.target.unwrap() {
                        //println!("Constraint met - value {} equal to target {}", value.0.unwrap(), self.target.unwrap());
                        return true;
                    }
                    return false;
                }
                ORelationalOperator::NotEqualTo => value.0.unwrap() != self.target.unwrap(),
                ORelationalOperator::LessOrEqualTo => value.0.unwrap() <= self.target.unwrap(),
                ORelationalOperator::LessThan => value.0.unwrap() < self.target.unwrap(),
                ORelationalOperator::GreaterOrEqualTo => value.0.unwrap() >= self.target.unwrap(),
                ORelationalOperator::GreaterThan => value.0.unwrap() > self.target.unwrap(),
            }
        }
    }

    /// Calculate the amount of violation of the constraint for a solution value. This is a measure
    /// about how close (or far) the constraint value is from the constraint target. If the
    /// constraint is met (i.e. the solution associated to the constraint is feasible), then the
    /// violation is 0.0. Otherwise, the absolute difference between `target` and `value`
    /// is returned.
    ///
    /// See:
    ///  - Kalyanmoy Deb & Samir Agrawal. (2002). <https://doi.org/10.1007/978-3-7091-6384-9_40>.
    ///  - Shuang Li, Ke Li, Wei Li. (2022). <https://doi.org/10.48550/arXiv.2205.14349>.
    ///
    /// # Arguments
    ///
    /// * `value`: The value to check against the constraint target.
    ///
    /// return: `f64`
    pub fn constraint_violation(&self, value: (Option<u64>, Option<Vec<HashMap<String, u64>>>, Option<HashMap<u64, (f64, f64, f64, f64)>>)) -> u64 {
        let v = value.clone();
        if self.is_met(v) {
            0
        } else {
            match self.operator {
                ORelationalOperator::EqualTo => self.target.unwrap_or(0).saturating_sub(value.0.clone().unwrap()),
                ORelationalOperator::NotEqualTo => 1,
                ORelationalOperator::LessOrEqualTo | ORelationalOperator::GreaterOrEqualTo => {
                    self.target.unwrap_or(0) - value.0.unwrap()
                }
                ORelationalOperator::LessThan | ORelationalOperator::GreaterThan => {
                    // add the tolerance
                    (self.target.unwrap_or(0) - value.0.unwrap()) + 1000
                }
            }
        }
    }

    /// Get the set constraint target.
    ///
    /// returns: `f64`.
    pub fn target(&self) -> Option<u64> {
        self.target
    }

    /// Get the set constraint operator.
    ///
    /// returns: `f64`.
    pub fn operator(&self) -> ORelationalOperator {
        self.operator.clone()
    }

    /// Get the set constraint service group.
    /// returns: `Option<Vec<String>>`.
    pub fn services(&self) -> Option<Vec<String>> {
        self.service_group.clone()
    }

    /// Get the set constraint resource.
    /// returns: `Option<(f64, f64, f64, f64)>`.
    /// (cpu, mem, disk, net)
    pub fn resource(&self) -> Option<HashMap<u64, (f64, f64, f64, f64)>> {
        self.resource.clone()
    }
}

impl Display for OConstraint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let sign = match self.operator {
            ORelationalOperator::EqualTo => "==",
            ORelationalOperator::NotEqualTo => "!=",
            ORelationalOperator::LessOrEqualTo => "<=",
            ORelationalOperator::LessThan => "<",
            ORelationalOperator::GreaterOrEqualTo => ">=",
            ORelationalOperator::GreaterThan => ">",
        };
        f.write_fmt(format_args!("{} {} {}", self.name, sign, self.target.unwrap_or(0)))
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceGroupOConstraint {
    /// The constraint name of the service group
    name: String,
    /// The relational operator to use to compare a value against the constraint target value.
    operator: ServiceGroupORelationalOperator,
    /// The constraint target.
    service_group: Vec<String>
}

impl ServiceGroupOConstraint {

    pub fn new(
        name: &str,
        operator: ServiceGroupORelationalOperator,
        service_group: Vec<String>,
    ) -> Self {
        Self {
            name: name.to_owned(),
            operator,
            service_group,
        }
    }

    pub fn is_met(&self, service_placements: &std::collections::HashMap<String, u64>) -> bool {
        // Check if all services in a service group are placed on the same node

        let mut node = None;

        for group in &self.service_group {
            if let Some(node_id) = service_placements.get(group) {
                if let Some(n) = node {
                    if n != *node_id {
                        return false;
                    }
                } else {
                    node = Some(*node_id);
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn constraint_violation(&self, service_placements: &std::collections::HashMap<String, u64>) -> u64 {
        if self.is_met(service_placements) {
            0
        } else {
            1
        }
    }
}

impl Display for ServiceGroupOConstraint {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let sign = match self.operator {
            ServiceGroupORelationalOperator::EqualTo => "==",
        };
        f.write_fmt(format_args!("{} {} for groups: {:?}", self.name, sign, self.service_group))
    }
}

// #[cfg(test)]
// mod test {
//     use float_cmp::assert_approx_eq;

//     use crate::core::{Constraint, ORelationalOperator};

//     #[test]
//     fn test_is_met() {
//         let c = Constraint::new("test", ORelationalOperator::EqualTo, 5.2);
//         assert!(c.is_met(5.2));
//         assert!(!c.is_met(15.0));

//         let c = Constraint::new("test", ORelationalOperator::NotEqualTo, 5.2);
//         assert!(!c.is_met(5.2));
//         assert!(c.is_met(15.0));

//         let c = Constraint::new("test", ORelationalOperator::GreaterThan, 5.2);
//         assert!(!c.is_met(5.2));
//         assert!(c.is_met(15.0));
//         assert!(!c.is_met(1.0));

//         let c = Constraint::new("test", ORelationalOperator::GreaterOrEqualTo, 5.2);
//         assert!(c.is_met(5.2));
//         assert!(c.is_met(15.0));
//         assert!(!c.is_met(1.0));

//         let c = Constraint::new("test", ORelationalOperator::LessThan, 5.2);
//         assert!(!c.is_met(5.2));
//         assert!(c.is_met(1.0));
//         assert!(!c.is_met(15.0));

//         let c = Constraint::new("test", ORelationalOperator::LessOrEqualTo, 5.2);
//         assert!(c.is_met(5.2));
//         assert!(!c.is_met(15.0));
//         assert!(c.is_met(1.0));

//         let c = Constraint::new_with_modifiers("test", ORelationalOperator::EqualTo, 5.2, 1.0, -1.0);
//         assert!(c.is_met(4.2));

//         let c = Constraint::new_with_modifiers("test", ORelationalOperator::EqualTo, 5.0, 0.5, 1.0);
//         assert!(c.is_met(3.5));
//     }

//     #[test]
//     fn test_constraint_violation() {
//         let c = Constraint::new("test", ORelationalOperator::EqualTo, 5.2);
//         assert_eq!(c.constraint_violation(5.2), 0.0);
//         assert_eq!(c.constraint_violation(1.2), 4.0);
//         assert_eq!(c.constraint_violation(-1.2), 6.4);

//         let c = Constraint::new("test", ORelationalOperator::NotEqualTo, 5.2);
//         assert_eq!(c.constraint_violation(5.2), 1.0);
//         assert_eq!(c.constraint_violation(1.0), 0.0);

//         let c = Constraint::new("test", ORelationalOperator::LessThan, 5.2);
//         assert_eq!(c.constraint_violation(0.0), 0.0);
//         assert_approx_eq!(f64, c.constraint_violation(9.2), 4.0, epsilon = 0.001);

//         let c = Constraint::new("test", ORelationalOperator::GreaterThan, 5.2);
//         assert_eq!(c.constraint_violation(10.0), 0.0);
//         assert_approx_eq!(f64, c.constraint_violation(2.2), 3.0, epsilon = 0.001);

//         let c = Constraint::new("test", ORelationalOperator::LessOrEqualTo, 5.2);
//         assert_eq!(c.constraint_violation(0.0), 0.0);
//         assert_eq!(c.constraint_violation(5.2), 0.0);
//         assert_approx_eq!(f64, c.constraint_violation(9.2), 4.0, epsilon = 0.001);

//         let c = Constraint::new("test", ORelationalOperator::GreaterOrEqualTo, 5.2);
//         assert_eq!(c.constraint_violation(10.0), 0.0);
//         assert_eq!(c.constraint_violation(5.2), 0.0);
//         assert_approx_eq!(f64, c.constraint_violation(2.2), 3.0, epsilon = 0.001);
//     }
// }
