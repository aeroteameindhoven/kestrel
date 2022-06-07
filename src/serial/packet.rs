use std::fmt::Debug;

use self::{metric_name::MetricName, metric_value::MetricValue, timestamp::Timestamp};

pub mod metric_name;
pub mod metric_value;
pub mod timestamp;

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
    pub timestamp: Timestamp,
    pub name: MetricName,
    pub value: MetricValue,
}
