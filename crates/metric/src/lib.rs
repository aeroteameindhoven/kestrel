use std::fmt::Debug;

use self::{name::MetricName, timestamp::Timestamp, value::MetricValue};

pub mod name;
pub mod timestamp;
pub mod value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
#[non_exhaustive]
pub enum RobotCommand {
    CalibrateAmbientInfrared = 0x00,
    CalibrateReferenceInfrared = 0x01,
}

#[derive(Debug)]
pub struct Metric {
    pub timestamp: Timestamp,
    pub name: MetricName,
    pub value: MetricValue,
}
