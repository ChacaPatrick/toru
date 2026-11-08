//! Paging through earlier days, newest first, for the gallery. Plain functions, so the rules can be
//! tested without a window.

use std::{fs, ops::Range};

use gpui::{Pixels, px};
use toru_core::{
    store::{Result as StoreResult, SessionId, Store},
    time::LocalDate,
};

use super::gallery::{CardFiles, NewCard};

/// What the gallery fetches when the user next nears the bottom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Nothing has been fetched yet, so which day comes first isn't known.
    Unknown,
    Day(LocalDate),
    /// Every day with captures has been fetched.
    End,
}

impl Next {
    /// Whether `day` has been fetched already, so that a capture from it must be listed by hand
    /// rather than waiting for its day to arrive from the store.
    pub fn has_fetched(self, day: LocalDate) -> bool {
        match self {
            Next::Unknown => false,
            Next::Day(next) => day > next,
            Next::End => true,
        }
    }
}

/// One day's captures from the store, and the day after them.
#[derive(Debug)]
pub struct Page {
    pub day: LocalDate,
    pub cards: Vec<NewCard>,
    pub next: Next,
}

/// Fetches the day `next` points at, or `None` if there isn't one. Today's page leaves out the
/// running session, which has its own group. Runs off the main thread.
pub fn fetch_page(
    store: &Store,
    session: SessionId,
    today: LocalDate,
    next: Next,
) -> StoreResult<Option<Page>> {
    let day = match next {
        // From the very latest, so that captures dated after today, by a clock since set back,
        // aren't lost from the list.
        Next::Unknown => store.latest_day_before(LocalDate::LATEST)?,
        Next::Day(day) => Some(day),
        Next::End => None,
    };
    let Some(day) = day else {
        return Ok(None);
    };
    let stored = if day == today {
        store.earlier_today(session, today)?
    } else {
        store.captures_on(day)?
    };
    let cards = stored
        .into_iter()
        .map(|stored| {
            let full = stored
                .capture
                .saved_path
                .clone()
                .unwrap_or_else(|| store.image_path(stored.id));
            NewCard {
                files: CardFiles {
                    id: stored.id,
                    thumbnail: store.thumbnail_path(stored.id),
                },
                capture: stored.capture,
                // A file that has moved away only leaves the size unknown.
                bytes: fs::metadata(full).map_or(0, |file| file.len()),
            }
        })
        .collect();
    let next = store.latest_day_before(day)?.map_or(Next::End, Next::Day);
    Ok(Some(Page { day, cards, next }))
}

/// Whether the rows run out within `ahead` below the bottom of the view, so more should be fetched.
pub fn near_bottom(content: Pixels, scrolled: Pixels, viewport: Pixels, ahead: Pixels) -> bool {
    content - (scrolled + viewport) < ahead
}

/// Which of `count` groups have a row of cards reaching into `band`, a range of distances from the
/// top of the list. `rows` is every row's group, if it is a row of cards, and height.
pub fn groups_near(
    rows: impl Iterator<Item = (Option<usize>, Pixels)>,
    band: Range<Pixels>,
    count: usize,
) -> Vec<bool> {
    let mut near = vec![false; count];
    let mut top = px(0.);
    for (group, height) in rows {
        let bottom = top + height;
        if let Some(group) = group
            && bottom > band.start
            && top < band.end
        {
            near[group] = true;
        }
        top = bottom;
    }
    near
}

/// Where `day`'s group goes among `days`, which run newest first, and whether it is already there.
pub fn day_slot(days: &[LocalDate], day: LocalDate) -> (usize, bool) {
    let slot = days.partition_point(|&other| other > day);
    (slot, days.get(slot) == Some(&day))
}

#[cfg(test)]
mod tests {
    use toru_core::{
        capture::{Capture, CaptureMode},
        export::save_capture,
        settings::SaveFormat,
        store::CaptureId,
        time::LocalDateTime,
    };
    use xcap::image::RgbaImage;

    use super::super::captures::tests::TestDir;
    use super::*;

    const fn date(year: u16, month: u8, day: u8) -> LocalDate {
        LocalDate { year, month, day }
    }

    const TODAY: LocalDate = date(2026, 11, 8);

    fn add(store: &Store, session: SessionId, name: &str, day: LocalDate) -> CaptureId {
        let capture = Capture {
            name: name.to_string(),
            mode: CaptureMode::Region,
            width: 4,
            height: 2,
            taken_at: LocalDateTime {
                date: day,
                hour: 12,
                minute: 0,
            },
            saved_path: None,
        };
        store
            .add_capture(session, &RgbaImage::new(4, 2), &capture, 2)
            .unwrap()
    }

    fn names(page: &Page) -> Vec<&str> {
        page.cards.iter().map(|c| c.capture.name.as_str()).collect()
    }

    #[test]
    fn days_come_a_page_at_a_time_newest_first_skipping_empty_days() {
        let dir = TestDir::new("paging");
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, "older", date(2026, 10, 30));
        add(&store, session, "yesterday", date(2026, 11, 7));
        let current = store.new_session().unwrap();

        let first = fetch_page(&store, current, TODAY, Next::Unknown)
            .unwrap()
            .unwrap();
        assert_eq!(first.day, date(2026, 11, 7));
        assert_eq!(names(&first), ["yesterday"]);
        assert_eq!(first.next, Next::Day(date(2026, 10, 30)));

        let second = fetch_page(&store, current, TODAY, first.next)
            .unwrap()
            .unwrap();
        assert_eq!(names(&second), ["older"]);
        assert_eq!(second.next, Next::End);

        assert!(
            fetch_page(&store, current, TODAY, Next::End)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn todays_page_leaves_out_the_running_session() {
        let dir = TestDir::new("today");
        let store = dir.store();
        let ended = store.new_session().unwrap();
        add(&store, ended, "earlier", TODAY);
        let current = store.new_session().unwrap();
        add(&store, current, "running", TODAY);

        let page = fetch_page(&store, current, TODAY, Next::Unknown)
            .unwrap()
            .unwrap();

        assert_eq!(page.day, TODAY);
        assert_eq!(names(&page), ["earlier"]);
        assert_eq!(page.next, Next::End);
    }

    #[test]
    fn a_new_day_finds_yesterdays_session_under_its_day() {
        let dir = TestDir::new("midnight");
        let store = dir.store();
        let ended = store.new_session().unwrap();
        add(&store, ended, "before-midnight", TODAY);
        let new = store.new_session().unwrap();

        let tomorrow = date(2026, 11, 9);
        let page = fetch_page(&store, new, tomorrow, Next::Unknown)
            .unwrap()
            .unwrap();

        assert_eq!(page.day, TODAY);
        assert_eq!(names(&page), ["before-midnight"]);
    }

    #[test]
    fn a_page_holds_saved_and_unsaved_captures_with_their_sizes() {
        let dir = TestDir::new("saved");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let day = date(2026, 11, 7);
        let unsaved = add(&store, session, "unsaved", day);
        let saved = add(&store, session, "saved", day);
        let folder = dir.0.join("Screenshots");
        let file = save_capture(&store, saved, &folder, SaveFormat::Png)
            .unwrap()
            .unwrap();

        let page = fetch_page(&store, session, TODAY, Next::Day(day))
            .unwrap()
            .unwrap();

        let card = |name: &str| page.cards.iter().find(|c| c.capture.name == name).unwrap();
        assert_eq!(card("saved").bytes, file.bytes);
        assert_eq!(card("saved").capture.saved_path, Some(file.path));
        assert_eq!(
            card("unsaved").bytes,
            fs::metadata(store.image_path(unsaved)).unwrap().len()
        );
        assert_eq!(card("unsaved").capture.saved_path, None);
        assert_eq!(
            card("unsaved").files.thumbnail,
            store.thumbnail_path(unsaved)
        );
    }

    #[test]
    fn a_missing_file_only_leaves_the_size_unknown() {
        let dir = TestDir::new("missing");
        let store = dir.store();
        let session = store.new_session().unwrap();
        let day = date(2026, 11, 7);
        let id = add(&store, session, "gone", day);
        fs::remove_file(store.image_path(id)).unwrap();

        let page = fetch_page(&store, session, TODAY, Next::Day(day))
            .unwrap()
            .unwrap();

        assert_eq!(page.cards[0].bytes, 0);
    }

    #[test]
    fn an_empty_history_ends_at_once() {
        let dir = TestDir::new("empty-history");
        let store = dir.store();
        let session = store.new_session().unwrap();
        assert!(
            fetch_page(&store, session, TODAY, Next::Unknown)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn captures_dated_after_today_are_still_listed() {
        let dir = TestDir::new("future");
        let store = dir.store();
        let session = store.new_session().unwrap();
        add(&store, session, "from-the-future", date(2026, 12, 25));
        let current = store.new_session().unwrap();

        let page = fetch_page(&store, current, TODAY, Next::Unknown)
            .unwrap()
            .unwrap();

        assert_eq!(page.day, date(2026, 12, 25));
    }

    #[test]
    fn only_days_above_the_next_one_have_been_fetched() {
        let day = date(2026, 11, 7);
        assert!(!Next::Unknown.has_fetched(day));
        assert!(!Next::Day(day).has_fetched(day));
        assert!(!Next::Day(date(2026, 11, 8)).has_fetched(day));
        assert!(Next::Day(date(2026, 11, 6)).has_fetched(day));
        assert!(Next::End.has_fetched(day));
    }

    #[test]
    fn fetching_starts_within_the_distance_ahead() {
        let at = |content, scrolled| near_bottom(px(content), px(scrolled), px(600.), px(1200.));
        assert!(at(100., 0.), "a short list starts fetching at once");
        assert!(at(3000., 1300.));
        assert!(!at(3000., 1100.));
        assert!(!at(3000., 0.));
    }

    #[test]
    fn groups_near_are_those_with_card_rows_in_the_band() {
        // Heights of 100: a header, group 1's two rows, a header, group 2's row.
        let rows = [None, Some(1), Some(1), None, Some(2)].map(|group| (group, px(100.)));
        let near =
            |band: Range<f32>| groups_near(rows.into_iter(), px(band.start)..px(band.end), 3);
        assert_eq!(near(0.0..100.0), [false, false, false]);
        assert_eq!(near(100.0..101.0), [false, true, false]);
        assert_eq!(near(250.0..450.0), [false, true, true]);
        assert_eq!(near(450.0..1000.0), [false, false, true]);
        assert_eq!(near(500.0..1000.0), [false, false, false]);
    }

    #[test]
    fn a_days_group_goes_in_date_order() {
        let days = [date(2026, 11, 8), date(2026, 11, 5), date(2026, 10, 30)];
        assert_eq!(day_slot(&days, date(2026, 11, 8)), (0, true));
        assert_eq!(day_slot(&days, date(2026, 11, 9)), (0, false));
        assert_eq!(day_slot(&days, date(2026, 11, 6)), (1, false));
        assert_eq!(day_slot(&days, date(2026, 11, 5)), (1, true));
        assert_eq!(day_slot(&days, date(2026, 1, 1)), (3, false));
        assert_eq!(day_slot(&[], date(2026, 1, 1)), (0, false));
    }
}
