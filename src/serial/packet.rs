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
    One(OneValue),
    Many(ManyValues),
    Unknown(String, Box<[u8]>),
}

#[derive(Debug, Clone, Copy)]
pub enum OneValue {
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
}

#[derive(Debug, Clone)]
pub enum ManyValues {
    U8(Box<[u8]>),
    U16(Box<[u16]>),
    U32(Box<[u32]>),
    U64(Box<[u64]>),
    I8(Box<[i8]>),
    I16(Box<[i16]>),
    I32(Box<[i32]>),
    I64(Box<[i64]>),
    Bool(Box<[bool]>),
    F32(Box<[f32]>),
    F64(Box<[f64]>),
}

pub enum MetricValueError {
    BadLength { expected: usize, got: usize },
}

impl MetricValue {
    pub fn from_bytes(ty: String, bytes: &[u8]) -> Result<Self, MetricValueError> {
        macro_rules! metric {
            ($bytes:ident as [bool]) => {
                metric!(@internal window as [u8])
                    .map(|result| result.map(|byte| byte != 0))
                    .collect::<Result<Box<[bool]>, _>>()
            };
            ($bytes:ident as [$ty:ty]) => {
                metric!(@internal window as [$ty])
                    .collect::<Result<Box<[$ty]>, _>>()
            };
            (@internal $bytes:ident as [$ty:ty]) => {
                bytes
                    .chunks(core::mem::size_of::<$ty>())
                    .map(|window| metric!(window as $ty))
            };
            ($bytes:ident as bool) => {
                metric!($bytes as u8).map(|byte| byte != 0)
            };
            ($bytes:ident as $ty:ty) => {
                $bytes
                    .try_into()
                    .map_err(|_| MetricValueError::BadLength {
                        expected: std::mem::size_of::<$ty>(),
                        got: $bytes.len(),
                    })
                    .map(|arr| <$ty>::from_le_bytes(arr))
            };
        }

        Ok(match ty.as_str() {
            "u8" => MetricValue::One(OneValue::U8(metric!(bytes as u8)?)),
            "[u8]" => MetricValue::Many(ManyValues::U8(metric!(bytes as [u8])?)),
            "u16" => MetricValue::One(OneValue::U16(metric!(bytes as u16)?)),
            "[u16]" => MetricValue::Many(ManyValues::U16(metric!(bytes as [u16])?)),
            "u32" => MetricValue::One(OneValue::U32(metric!(bytes as u32)?)),
            "[u32]" => MetricValue::Many(ManyValues::U32(metric!(bytes as [u32])?)),
            "u64" => MetricValue::One(OneValue::U64(metric!(bytes as u64)?)),
            "[u64]" => MetricValue::Many(ManyValues::U64(metric!(bytes as [u64])?)),

            "i8" => MetricValue::One(OneValue::I8(metric!(bytes as i8)?)),
            "[i8]" => MetricValue::Many(ManyValues::I8(metric!(bytes as [i8])?)),
            "i16" => MetricValue::One(OneValue::I16(metric!(bytes as i16)?)),
            "[i16]" => MetricValue::Many(ManyValues::I16(metric!(bytes as [i16])?)),
            "i32" => MetricValue::One(OneValue::I32(metric!(bytes as i32)?)),
            "[i32]" => MetricValue::Many(ManyValues::I32(metric!(bytes as [i32])?)),
            "i64" => MetricValue::One(OneValue::I64(metric!(bytes as i64)?)),
            "[i64]" => MetricValue::Many(ManyValues::I64(metric!(bytes as [i64])?)),

            "bool" => MetricValue::One(OneValue::Bool(metric!(bytes as bool)?)),
            "[bool]" => MetricValue::Many(ManyValues::Bool(metric!(bytes as [bool])?)),

            "f32" => MetricValue::One(OneValue::F32(metric!(bytes as f32)?)),
            "[f32]" => MetricValue::Many(ManyValues::F32(metric!(bytes as [f32])?)),
            "f64" => MetricValue::One(OneValue::F64(metric!(bytes as f64)?)),
            "[f64]" => MetricValue::Many(ManyValues::F64(metric!(bytes as [f64])?)),

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
            MetricValue::One(value) => match value {
                OneValue::U8(value) => ("u8", value),
                OneValue::U16(value) => ("u16", value),
                OneValue::U32(value) => ("u32", value),
                OneValue::U64(value) => ("u64", value),
                OneValue::I8(value) => ("i8", value),
                OneValue::I16(value) => ("i16", value),
                OneValue::I32(value) => ("i32", value),
                OneValue::I64(value) => ("i64", value),
                OneValue::Bool(value) => ("bool", value),
                OneValue::F32(value) => ("f32", value),
                OneValue::F64(value) => ("f64", value),
            },
            MetricValue::Many(value) => match value {
                ManyValues::U8(value) => ("[u8]", value),
                ManyValues::U16(value) => ("[u16]", value),
                ManyValues::U32(value) => ("[u32]", value),
                ManyValues::U64(value) => ("[u64]", value),
                ManyValues::I8(value) => ("[i8]", value),
                ManyValues::I16(value) => ("[i16]", value),
                ManyValues::I32(value) => ("[i32]", value),
                ManyValues::I64(value) => ("[i64]", value),
                ManyValues::Bool(value) => ("[bool]", value),
                ManyValues::F32(value) => ("[f32]", value),
                ManyValues::F64(value) => ("[f64]", value),
            },
            MetricValue::Unknown(ty, value) => (ty, value),
        }
    }

    pub fn as_integer(&self) -> Option<i128> {
        match self {
            MetricValue::One(value) => match value {
                OneValue::U8(value) => Some(i128::from(*value)),
                OneValue::U16(value) => Some(i128::from(*value)),
                OneValue::U32(value) => Some(i128::from(*value)),
                OneValue::U64(value) => Some(i128::from(*value)),
                OneValue::I8(value) => Some(i128::from(*value)),
                OneValue::I16(value) => Some(i128::from(*value)),
                OneValue::I32(value) => Some(i128::from(*value)),
                OneValue::I64(value) => Some(i128::from(*value)),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn as_float(&self) -> Option<f64> {
        match self {
            MetricValue::One(value) => match value {
                OneValue::F32(value) => Some(f64::from(*value)),
                OneValue::F64(value) => Some(*value),
                _ => None,
            },
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
