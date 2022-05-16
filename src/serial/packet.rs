#![forbid(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_lossless
)]

use std::{convert::Infallible, fmt::Debug, str::FromStr};

#[derive(Debug)]
pub enum Packet {
    Telemetry(Metric),
    System(SystemPacket),
}

#[derive(Debug)]
pub enum SystemPacket {
    SerialDisconnect,
    SerialConnect,
}

#[derive(Debug)]
pub struct Metric {
    pub timestamp: u32,
    pub name: MetricName,
    pub value: MetricValue,
}

#[derive(Debug, Clone)]
pub enum MetricValue {
    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),
    Bool(bool),
    F32(f32),
    F64(f64),
    Unknown(String, Box<[u8]>),
}

pub enum MetricValueError {
    BadLength { expected: usize, got: usize },
}

impl MetricValue {
    pub fn from_bytes(ty: String, bytes: &[u8]) -> Result<Self, MetricValueError> {
        macro_rules! metric {
            (as $ty:ty) => {
                <$ty>::from_le_bytes(bytes.try_into().map_err(|_| MetricValueError::BadLength {
                    expected: std::mem::size_of::<$ty>(),
                    got: bytes.len(),
                })?)
            };
        }

        Ok(match ty.as_str() {
            "u8" => MetricValue::U8(metric!(as u8)),
            "u16" => MetricValue::U16(metric!(as u16)),
            "u32" => MetricValue::U32(metric!(as u32)),
            "u64" => MetricValue::U64(metric!(as u64)),

            "i8" => MetricValue::I8(metric!(as i8)),
            "i16" => MetricValue::I16(metric!(as i16)),
            "i32" => MetricValue::I32(metric!(as i32)),
            "i64" => MetricValue::I64(metric!(as i64)),

            "bool" => MetricValue::Bool(metric!(as u8) != 0),

            "f32" => MetricValue::F32(metric!(as f32)),
            "f64" => MetricValue::F64(metric!(as f64)),

            _ => MetricValue::Unknown(ty, Box::from(bytes)),
        })
    }
}

impl MetricValue {
    #[inline]
    pub fn ty(&self) -> &str {
        self.ty_value().0
    }

    #[inline]
    pub fn value(&self) -> String {
        format!("{:?}", self.ty_value().1)
    }

    #[inline]
    fn ty_value(&self) -> (&str, &dyn Debug) {
        match self {
            MetricValue::U8(value) => ("u8", value),
            MetricValue::U16(value) => ("u16", value),
            MetricValue::U32(value) => ("u32", value),
            MetricValue::U64(value) => ("u64", value),
            MetricValue::I8(value) => ("i8", value),
            MetricValue::I16(value) => ("i16", value),
            MetricValue::I32(value) => ("i32", value),
            MetricValue::I64(value) => ("i64", value),
            MetricValue::Bool(value) => ("bool", value),
            MetricValue::F32(value) => ("f32", value),
            MetricValue::F64(value) => ("f64", value),
            MetricValue::Unknown(ty, value) => (ty, value),
        }
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            MetricValue::U8(value) => Some(i128::from(*value)),
            MetricValue::U16(value) => Some(i128::from(*value)),
            MetricValue::U32(value) => Some(i128::from(*value)),
            MetricValue::U64(value) => Some(i128::from(*value)),
            MetricValue::I8(value) => Some(i128::from(*value)),
            MetricValue::I16(value) => Some(i128::from(*value)),
            MetricValue::I32(value) => Some(i128::from(*value)),
            MetricValue::I64(value) => Some(i128::from(*value)),
            MetricValue::Bool(_) => None,
            MetricValue::F32(_) => None,
            MetricValue::F64(_) => None,
            MetricValue::Unknown(_, _) => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            MetricValue::U8(value) => Some(f64::from(*value)),
            MetricValue::U16(value) => Some(f64::from(*value)),
            MetricValue::U32(value) => Some(f64::from(*value)),
            MetricValue::U64(value) => None,
            MetricValue::I8(value) => Some(f64::from(*value)),
            MetricValue::I16(value) => Some(f64::from(*value)),
            MetricValue::I32(value) => Some(f64::from(*value)),
            MetricValue::I64(value) => None,
            MetricValue::Bool(_) => None,
            MetricValue::F32(value) => Some(f64::from(*value)),
            MetricValue::F64(value) => Some(*value),
            MetricValue::Unknown(_, _) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetricName {
    Namespaced { namespace: String, name: String },
    Global(String),
}

impl MetricName {
    pub fn namespaced(namespace: impl Into<String>, name: impl Into<String>) -> Self {
        Self::Namespaced {
            namespace: namespace.into(),
            name: name.into(),
        }
    }

    pub fn global(name: impl Into<String>) -> Self {
        Self::Global(name.into())
    }
}

impl FromStr for MetricName {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.split_once(':') {
            Some((namespace, name)) => Self::Namespaced {
                namespace: namespace.into(),
                name: name.into(),
            },
            None => Self::Global(s.into()),
        })
    }
}
