use std::collections::HashMap;

use serde::{Deserialize, Serialize, Serializer};

use crate::core::OOError;

/// The data type and value that can be stored in an individual or algorithm..
#[derive(Clone, Deserialize, Debug)]
#[serde(untagged)]
pub enum ODataValue {
    /// The value for a floating-point number. This is a f64.
    Real(f64),
    /// The value for an integer number. This is an i64.
    Integer(i64),
    /// The value for an usize.
    USize(usize),
    /// The value for a vector of floating-point numbers.
    Vector(Vec<f64>),
    /// The value for a vector of nested data.
    DataVector(Vec<ODataValue>),
    /// The value for a Hashmap
    Map(HashMap<String, ODataValue>),
}

impl Serialize for ODataValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            ODataValue::Real(v) => serializer.serialize_f64(*v),
            ODataValue::Integer(v) => serializer.serialize_i64(*v),
            ODataValue::USize(v) => serializer.serialize_u64(*v as u64),
            ODataValue::Vector(v) => serializer.collect_seq(v),
            ODataValue::DataVector(v) => serializer.collect_seq(v),
            ODataValue::Map(v) => serializer.collect_map(v),
        }
    }
}

impl PartialEq for ODataValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (ODataValue::Real(s), ODataValue::Real(o)) => (s.is_nan() && o.is_nan()) || (*s == *o),
            (ODataValue::Integer(s), ODataValue::Integer(o)) => *s == *o,
            (ODataValue::USize(s), ODataValue::USize(o)) => s == o,
            (ODataValue::Vector(s), ODataValue::Vector(o)) => s == o,
            _ => false,
        }
    }
}

impl ODataValue {
    /// Get the value if the data is of real type. This returns an error if the data is not real.
    ///
    /// returns: `Result<f64, OError>`
    pub fn as_real(&self) -> Result<f64, OOError> {
        if let ODataValue::Real(v) = self {
            Ok(*v)
        } else {
            Err(OOError::WrongDataType("real".to_string()))
        }
    }

    /// Get the value if the data is of integer type. This returns an error if the data is not an
    /// integer.
    ///
    /// returns: `Result<f64, OError>`
    pub fn as_integer(&self) -> Result<i64, OOError> {
        if let ODataValue::Integer(v) = self {
            Ok(*v)
        } else {
            Err(OOError::WrongDataType("integer".to_string()))
        }
    }

    /// Get the value if the data is of vector of f64. This returns an error if the data is not a
    /// vector.
    ///
    /// returns: `Result<&Vec<f64, OError>`
    pub fn as_f64_vec(&self) -> Result<&Vec<f64>, OOError> {
        if let ODataValue::Vector(v) = self {
            Ok(v)
        } else {
            Err(OOError::WrongDataType("vector of f64".to_string()))
        }
    }

    /// Get the value if the data is of vector of data. This returns an error if the data is not a
    /// data vector.
    ///
    /// returns: `Result<&Vec<DataValue>, OError>`
    pub fn as_data_vec(&self) -> Result<&Vec<ODataValue>, OOError> {
        if let ODataValue::DataVector(v) = self {
            Ok(v)
        } else {
            Err(OOError::WrongDataType("vector of data".to_string()))
        }
    }
    /// Get the value if the data is a mao. This returns an error if the data is not a map.
    ///
    /// returns: `Result<HashMap<String, DataValue>, OError>`
    pub fn as_map(&self) -> Result<&HashMap<String, ODataValue>, OOError> {
        if let ODataValue::Map(v) = self {
            Ok(v)
        } else {
            Err(OOError::WrongDataType("map".to_string()))
        }
    }

    /// Get the value if the data is of usize type. This returns an error if the data is not an
    /// usize.
    ///
    /// returns: `Result<f64, OError>`
    pub fn as_usize(&self) -> Result<usize, OOError> {
        if let ODataValue::USize(v) = self {
            Ok(*v)
        } else {
            Err(OOError::WrongDataType("usize".to_string()))
        }
    }
}
