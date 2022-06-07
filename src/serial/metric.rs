use std::fmt::Debug;

use self::{name::MetricName, value::MetricValue, timestamp::Timestamp};

pub mod name;
pub mod value;
pub mod timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RobotCommand {
    StartInfraredInitialization = 0x18
}

#[derive(Debug)]
pub struct Metric {
    pub timestamp: Timestamp,
    pub name: MetricName,
    pub value: MetricValue,
}
