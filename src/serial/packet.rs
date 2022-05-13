
#[derive(Debug)]
pub enum Packet {
    Telemetry(Metric),
    System(SystemPacket),
}

#[derive(Debug)]
pub struct Metric {
    pub name: String,
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

#[derive(Debug)]
pub enum SystemPacket {
    SerialDisconnect,
    SerialConnect,
}