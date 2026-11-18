use toru_core::time::{LocalDate, LocalDateTime};

#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetLocalTime(time: *mut SystemTime);
}

pub fn local_now() -> LocalDateTime {
    let mut now = SystemTime::default();
    // SAFETY: `now` is a writable SYSTEMTIME.
    unsafe { GetLocalTime(&mut now) };
    LocalDateTime {
        date: LocalDate {
            year: now.year,
            month: now.month as u8,
            day: now.day as u8,
        },
        hour: now.hour as u8,
        minute: now.minute as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_local_time_is_a_real_date() {
        let now = local_now();
        assert!(now.date.year >= 2026);
        assert!((1..=12).contains(&now.date.month));
        assert!((1..=31).contains(&now.date.day));
        assert!(now.hour < 24 && now.minute < 60);
    }
}
