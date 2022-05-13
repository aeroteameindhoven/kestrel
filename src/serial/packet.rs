use std::{convert::Infallible, str::FromStr};

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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum MetricName {
    Namespaced { namespace: String, name: String },
    Default(String),
}

impl FromStr for MetricName {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.split_once(':') {
            Some((namespace, name)) => Self::Namespaced {
                namespace: namespace.into(),
                name: name.into(),
            },
            None => Self::Default(s.into()),
        })
    }
}
