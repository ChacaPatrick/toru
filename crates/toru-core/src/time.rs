//! Plain local date and time values.

/// A date in local time. `month` is 1 to 12 and `day` is 1 to 31.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

/// A date and time in local time. `hour` is 0 to 23 and `minute` is 0 to 59.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub date: LocalDate,
    pub hour: u8,
    pub minute: u8,
}
