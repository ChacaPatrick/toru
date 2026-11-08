//! Plain local date and time values.

const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// A date in local time. `month` is 1 to 12 and `day` is 1 to 31.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl LocalDate {
    /// "Nov", or "???" for a month outside 1 to 12, which a date read back from a damaged
    /// database may have.
    pub fn month_short(self) -> &'static str {
        usize::from(self.month)
            .checked_sub(1)
            .and_then(|index| MONTHS.get(index))
            .unwrap_or(&"???")
    }

    /// "Tue", by Sakamoto's method, or "???" for a month outside 1 to 12.
    pub fn weekday_short(self) -> &'static str {
        const OFFSETS: [i64; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let Some(offset) = usize::from(self.month)
            .checked_sub(1)
            .and_then(|index| OFFSETS.get(index))
        else {
            return "???";
        };
        // Floor divisions, so January and February of year 0 come out right too.
        let year = i64::from(self.year) - i64::from(self.month < 3);
        let days = year + year.div_euclid(4) - year.div_euclid(100)
            + year.div_euclid(400)
            + offset
            + i64::from(self.day);
        WEEKDAYS[days.rem_euclid(7) as usize]
    }

    /// The date `days` days earlier, or January 1 of year 0 if that would come before it.
    pub fn minus_days(self, days: u32) -> LocalDate {
        let LocalDate {
            mut year,
            mut month,
            mut day,
        } = self;
        let mut days = days;
        // Walks back a month at a time, stepping onto the last day of the month before.
        while days >= u32::from(day) {
            days -= u32::from(day);
            if month <= 1 {
                let Some(previous_year) = year.checked_sub(1) else {
                    return LocalDate {
                        year: 0,
                        month: 1,
                        day: 1,
                    };
                };
                year = previous_year;
                month = 12;
            } else {
                month -= 1;
            }
            day = days_in_month(year, month);
        }
        LocalDate {
            year,
            month,
            // `days` is below `day` here, so this always fits.
            day: day - days as u8,
        }
    }
}

/// A date and time in local time. `hour` is 0 to 23 and `minute` is 0 to 59.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalDateTime {
    pub date: LocalDate,
    pub hour: u8,
    pub minute: u8,
}

impl LocalDateTime {
    /// "2:31 PM".
    pub fn clock(self) -> String {
        let period = if self.hour < 12 { "AM" } else { "PM" };
        let hour = if self.hour.is_multiple_of(12) {
            12
        } else {
            self.hour % 12
        };
        format!("{hour}:{:02} {period}", self.minute)
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 31,
    }
}

fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn date(year: u16, month: u8, day: u8) -> LocalDate {
        LocalDate { year, month, day }
    }

    #[test]
    fn zero_days_is_the_same_date() {
        assert_eq!(date(2026, 11, 8).minus_days(0), date(2026, 11, 8));
    }

    #[test]
    fn days_within_the_month() {
        assert_eq!(date(2026, 11, 8).minus_days(7), date(2026, 11, 1));
    }

    #[test]
    fn crosses_into_the_previous_month() {
        assert_eq!(date(2026, 11, 1).minus_days(1), date(2026, 10, 31));
        assert_eq!(date(2026, 10, 1).minus_days(1), date(2026, 9, 30));
        assert_eq!(date(2026, 11, 8).minus_days(8), date(2026, 10, 31));
    }

    #[test]
    fn crosses_into_the_previous_year() {
        assert_eq!(date(2026, 1, 1).minus_days(1), date(2025, 12, 31));
        assert_eq!(date(2026, 1, 10).minus_days(30), date(2025, 12, 11));
    }

    #[test]
    fn february_follows_the_leap_year_rules() {
        assert_eq!(date(2024, 3, 1).minus_days(1), date(2024, 2, 29));
        assert_eq!(date(2026, 3, 1).minus_days(1), date(2026, 2, 28));
        assert_eq!(date(2000, 3, 1).minus_days(1), date(2000, 2, 29));
        assert_eq!(date(1900, 3, 1).minus_days(1), date(1900, 2, 28));
    }

    #[test]
    fn long_spans_land_on_the_right_day() {
        assert_eq!(date(2026, 11, 8).minus_days(1000), date(2024, 2, 12));
        assert_eq!(date(2026, 11, 8).minus_days(3653), date(2016, 11, 7));
    }

    #[test]
    fn stops_at_the_earliest_date() {
        assert_eq!(date(2026, 11, 8).minus_days(u32::MAX), date(0, 1, 1));
    }

    #[test]
    fn month_and_weekday_names() {
        assert_eq!(date(2026, 11, 8).weekday_short(), "Sun");
        assert_eq!(date(2026, 11, 6).weekday_short(), "Fri");
        assert_eq!(date(2024, 2, 29).weekday_short(), "Thu");
        assert_eq!(date(2000, 1, 1).weekday_short(), "Sat");
        assert_eq!(date(0, 1, 1).weekday_short(), "Sat");
        assert_eq!(date(2026, 11, 8).month_short(), "Nov");
        assert_eq!(date(2026, 13, 8).month_short(), "???");
        assert_eq!(date(2026, 0, 8).weekday_short(), "???");
    }
}
