/// The strings whose tone changes with the look. Dates go where the caller puts them.
pub struct Strings {
    /// Around the day being fetched: "Fetching " + date + " …".
    pub loading: (&'static str, &'static str),
    pub copied: &'static str,
    pub back_to_session: &'static str,
    /// Before the day in a history group header.
    pub earlier: &'static str,
    /// The header of today's captures from sessions that have ended.
    pub earlier_today: &'static str,
    pub session: &'static str,
    pub day_date: DayDate,
}

/// How a day group writes its date.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DayDate {
    /// "Tue, Nov 4"
    Long,
    /// "NOV 4"
    ShortUpper,
    /// "TUE NOV 4"
    LongUpper,
}
