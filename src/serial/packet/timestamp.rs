use std::{
    fmt::{self, Display},
    ops::Sub,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default, Hash)]
pub struct Timestamp {
    timestamp: u32,
}

impl Timestamp {
    pub const MAX: Self = Self::from_millis(u32::MAX);
    pub const MIN: Self = Self::from_millis(u32::MIN);

    pub const fn from_millis(millis: u32) -> Self {
        Self { timestamp: millis }
    }
}

impl Timestamp {
    pub const fn timestamp(&self) -> u32 {
        self.timestamp
    }

    pub const fn millis(&self) -> u32 {
        self.timestamp % 1_000
    }

    pub const fn seconds(&self) -> u32 {
        (self.timestamp / 1_000) % 60
    }

    pub const fn minutes(&self) -> u32 {
        self.timestamp / 60_000
    }
}

impl Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let millis = self.millis();
        let seconds = self.seconds();
        let minutes = self.minutes();

        write!(f, "{minutes:02}:{seconds:02}.{millis:03}")
    }
}

impl Sub for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: Self) -> Self::Output {
        Timestamp {
            timestamp: self.timestamp.saturating_sub(rhs.timestamp),
        }
    }
}
