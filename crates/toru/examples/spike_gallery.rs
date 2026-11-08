//! Spike for `docs/spikes.md` section 1.3: a 3-across gallery of 200 cards, each with a thumbnail
//! made from a 1440x900 synthetic capture and an editable name field.
//!
//! Field events and scroll timings are printed to stdout.
//!
//! Set `TORU_SPIKE_AUTOSCROLL=1` to sweep the gallery twice once every thumbnail has loaded, print
//! the frame report and quit. `TORU_SPIKE_THUMB_WIDTH` sets the thumbnail width in pixels, and
//! `TORU_SPIKE_AUTOCLOSE_MS` quits after that long.

use std::{
    cell::RefCell,
    env,
    io::Cursor,
    mem,
    ops::Range,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, App, Bounds, Context, Entity, ObjectFit, Pixels, Render, RenderImage, Size,
    Subscription, Window, WindowBounds, WindowOptions, canvas, div, img, point, prelude::*, px,
    size,
};
use gpui_kit::component::{
    ActiveTheme, VirtualListScrollHandle,
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    v_virtual_list,
};
use gpui_platform::application;
use xcap::{
    Monitor,
    image::{Frame, ImageFormat, Rgba, RgbaImage, imageops, load_from_memory_with_format},
};

const CARD_COUNT: usize = 200;
const COLUMNS: usize = 3;
const CARDS_PER_DAY: usize = 30;
const CAPTURE_SIZE: (u32, u32) = (1440, 900);
/// About the card's display width at scale 1, so one thumbnail pixel maps to one screen pixel.
const DEFAULT_THUMB_WIDTH: u32 = 280;

const HEADER_HEIGHT: f32 = 48.;
const ROW_HEIGHT: f32 = 258.;
const THUMB_HEIGHT: f32 = 140.;
const SWEEP_STEP: f32 = 30.;
const SWEEP_PASSES: usize = 2;

enum Row {
    Header(usize),
    Cards(Range<usize>),
}

/// Timing for frames where the list moved. Everything is in milliseconds.
struct Frames {
    started: Instant,
    target_ms: f32,
    first_paint_printed: bool,
    frame_start: Instant,
    moved: bool,
    last_moved: Option<Instant>,
    intervals: Vec<f32>,
    work: Vec<f32>,
}

impl Frames {
    fn begin(&mut self, moved: bool) {
        self.frame_start = Instant::now();
        self.moved = moved;
    }

    fn end_of_paint(&mut self) {
        if !self.first_paint_printed {
            self.first_paint_printed = true;
            println!(
                "first frame painted {:?} after main",
                self.started.elapsed()
            );
        }
        if self.moved {
            self.work
                .push(self.frame_start.elapsed().as_secs_f32() * 1000.);
            if let Some(last) = self.last_moved {
                self.intervals
                    .push((self.frame_start - last).as_secs_f32() * 1000.);
            }
            self.last_moved = Some(self.frame_start);
        }
    }

    fn report(&mut self, label: &str) {
        print_stats(
            label,
            "interval",
            &mem::take(&mut self.intervals),
            self.target_ms,
        );
        print_stats(label, "work", &mem::take(&mut self.work), self.target_ms);
        self.last_moved = None;
    }
}

fn print_stats(label: &str, kind: &str, samples: &[f32], target_ms: f32) {
    if samples.is_empty() {
        return;
    }
    let over = |limit: f32| samples.iter().filter(|&&sample| sample > limit).count();
    let mut sorted = samples.to_vec();
    sorted.sort_by(f32::total_cmp);
    println!(
        "{label} {kind}: {} frames, avg {:.2} ms, p99 {:.2}, worst {:.2}, over {target_ms:.2} ms (target): {}, over 1.5x target: {}, over 16.7 ms: {}, over 33 ms: {}",
        samples.len(),
        samples.iter().sum::<f32>() / samples.len() as f32,
        sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)],
        sorted[sorted.len() - 1],
        over(target_ms),
        over(target_ms * 1.5),
        over(16.7),
        over(33.),
    );
}

struct Gallery {
    names: Vec<Entity<InputState>>,
    thumbnails: Vec<Option<Arc<RenderImage>>>,
    rows: Vec<Row>,
    row_sizes: Rc<Vec<Size<Pixels>>>,
    scroll: VirtualListScrollHandle,
    frames: Rc<RefCell<Frames>>,
    last_offset: Pixels,
    loaded: usize,
    decode_total: Duration,
    png_total: usize,
    autoscroll: bool,
    sweep_pass: Option<usize>,
    started: Instant,
    _subscriptions: Vec<Subscription>,
}

impl Gallery {
    fn new(
        thumb_width: u32,
        autoscroll: bool,
        target_ms: f32,
        started: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        print_memory("before creating inputs");
        let inputs_started = Instant::now();
        let names: Vec<_> = (0..CARD_COUNT)
            .map(|index| {
                let name = format!("screenshot_11_08_2026_{}", index + 1);
                cx.new(|cx| InputState::new(window, cx).default_value(name))
            })
            .collect();
        println!(
            "created {CARD_COUNT} inputs in {:?}",
            inputs_started.elapsed()
        );
        print_memory("after creating inputs");

        let subscriptions = names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                cx.subscribe(name, move |_, name, event: &InputEvent, cx| match event {
                    InputEvent::Change => {
                        println!("card {index} value: {:?}", name.read(cx).value())
                    }
                    InputEvent::Focus => println!("card {index} focused"),
                    InputEvent::Blur => println!("card {index} blurred"),
                    InputEvent::PressEnter { .. } => println!("card {index} enter"),
                })
            })
            .collect();

        for index in 0..CARD_COUNT {
            let thumbnail = cx.background_spawn(async move { make_thumbnail(index, thumb_width) });
            cx.spawn(async move |this, cx| {
                let thumbnail = thumbnail.await;
                this.update(cx, |this, cx| this.thumbnail_ready(index, thumbnail, cx))
                    .ok();
            })
            .detach();
        }

        // Scrolling by hand has no clear end, so each burst is reported once the list goes quiet.
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(400))
                    .await;
                if this
                    .update(cx, |this, _| this.report_finished_scroll())
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        let rows = gallery_rows();
        let row_sizes = rows
            .iter()
            .map(|row| {
                let height = match row {
                    Row::Header(_) => HEADER_HEIGHT,
                    Row::Cards(_) => ROW_HEIGHT,
                };
                size(px(1.), px(height))
            })
            .collect();

        Self {
            names,
            thumbnails: vec![None; CARD_COUNT],
            rows,
            row_sizes: Rc::new(row_sizes),
            scroll: VirtualListScrollHandle::new(),
            frames: Rc::new(RefCell::new(Frames {
                started,
                target_ms,
                first_paint_printed: false,
                frame_start: started,
                moved: false,
                last_moved: None,
                intervals: Vec::new(),
                work: Vec::new(),
            })),
            last_offset: px(0.),
            loaded: 0,
            decode_total: Duration::ZERO,
            png_total: 0,
            autoscroll,
            sweep_pass: None,
            started,
            _subscriptions: subscriptions,
        }
    }

    fn thumbnail_ready(&mut self, index: usize, thumbnail: Thumbnail, cx: &mut Context<Self>) {
        self.thumbnails[index] = Some(thumbnail.image);
        self.decode_total += thumbnail.decode_time;
        self.png_total += thumbnail.png_size;
        self.loaded += 1;
        if self.loaded == CARD_COUNT {
            println!(
                "all {CARD_COUNT} thumbnails ready {:?} after main; decode and downscale averaged \
                 {:?} per image; PNGs averaged {} KB",
                self.started.elapsed(),
                self.decode_total / CARD_COUNT as u32,
                self.png_total / CARD_COUNT / 1024,
            );
            print_memory("all thumbnails decoded, few uploaded to the GPU");
            if self.autoscroll {
                self.sweep_pass = Some(1);
            }
        }
        cx.notify();
    }

    fn report_finished_scroll(&mut self) {
        let mut frames = self.frames.borrow_mut();
        let quiet = frames
            .last_moved
            .is_some_and(|last| last.elapsed() > Duration::from_millis(300));
        if quiet && self.sweep_pass.is_none() {
            let offset = f32::from(self.scroll.offset().y);
            frames.report(&format!("manual scroll, now at {offset:.0}"));
        }
    }

    /// One step of the automatic sweep. The first pass uploads every thumbnail to the GPU; later
    /// passes draw thumbnails that are already there.
    fn advance_sweep(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(pass) = self.sweep_pass else {
            return;
        };
        window.request_animation_frame();

        let y = -self.scroll.offset().y;
        let max = self.scroll.max_offset().y;
        if y < max {
            self.scroll
                .set_offset(point(px(0.), -(y + px(SWEEP_STEP)).min(max)));
            return;
        }

        self.frames.borrow_mut().report(&format!("sweep {pass}"));
        print_memory(&format!("after sweep {pass}"));
        self.scroll.set_offset(point(px(0.), px(0.)));
        self.sweep_pass = (pass < SWEEP_PASSES).then_some(pass + 1);
        if self.sweep_pass.is_none() {
            cx.spawn(async move |_, cx| {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                print_memory("one second after the last sweep");
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    }

    fn render_row(&self, row: usize, cx: &App) -> AnyElement {
        match &self.rows[row] {
            Row::Header(day) => div()
                .w_full()
                .h(px(HEADER_HEIGHT))
                .px_4()
                .flex()
                .items_center()
                .gap_2()
                .text_color(cx.theme().foreground)
                .child(format!("Earlier: day {}", day + 1))
                .child(div().text_sm().child(format!(
                    "{CARDS_PER_DAY} captures · shown because “Show previous” is on"
                )))
                .into_any_element(),
            Row::Cards(cards) => div()
                .w_full()
                .h(px(ROW_HEIGHT))
                .px_4()
                .pb_4()
                .flex()
                .gap_4()
                .children(cards.clone().map(|index| self.render_card(index, cx)))
                .children((cards.len()..COLUMNS).map(|_| div().flex_1().p_2().border_1()))
                .into_any_element(),
        }
    }

    fn render_card(&self, index: usize, cx: &App) -> AnyElement {
        let theme = cx.theme();
        let thumbnail = match &self.thumbnails[index] {
            Some(image) => img(image.clone())
                .size_full()
                .object_fit(ObjectFit::Cover)
                .into_any_element(),
            None => div().size_full().bg(theme.muted).into_any_element(),
        };
        let caption = format!("1440×900 · Region · card {}", index + 1);

        div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_2()
            .border_1()
            .border_color(theme.border)
            .rounded_md()
            .text_color(theme.foreground)
            .child(
                div()
                    .relative()
                    .h(px(THUMB_HEIGHT))
                    .overflow_hidden()
                    .rounded_sm()
                    .child(thumbnail)
                    .child(
                        div()
                            .absolute()
                            .left_1()
                            .bottom_1()
                            .px_1()
                            .text_xs()
                            .bg(theme.background)
                            .child(caption),
                    ),
            )
            .child(
                Input::new(&self.names[index]).aria_label(format!("Name of capture {}", index + 1)),
            )
            .child(div().text_sm().child("Unsaved"))
            .into_any_element()
    }
}

impl Render for Gallery {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.advance_sweep(window, cx);
        let offset = self.scroll.offset().y;
        let moved = offset != self.last_offset;
        self.last_offset = offset;
        self.frames.borrow_mut().begin(moved);

        let list = v_virtual_list(
            cx.entity(),
            "gallery",
            self.row_sizes.clone(),
            |this, range, _, cx| {
                range
                    .map(|row| this.render_row(row, cx))
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&self.scroll);

        // Painted last, so this marks the end of render-to-paint work.
        let frames = self.frames.clone();
        let end_marker = canvas(
            |_, _, _| (),
            move |_, _, _, _| frames.borrow_mut().end_of_paint(),
        )
        .absolute()
        .size_0();

        div()
            .relative()
            .size_full()
            .bg(cx.theme().background)
            .child(list)
            .vertical_scrollbar(&self.scroll)
            .child(end_marker)
    }
}

fn gallery_rows() -> Vec<Row> {
    let mut rows = Vec::new();
    let mut card = 0;
    while card < CARD_COUNT {
        let day_end = (card + CARDS_PER_DAY).min(CARD_COUNT);
        rows.push(Row::Header(card / CARDS_PER_DAY));
        while card < day_end {
            let row_end = (card + COLUMNS).min(day_end);
            rows.push(Row::Cards(card..row_end));
            card = row_end;
        }
    }
    rows
}

fn mix(value: u32) -> u32 {
    let value = value.wrapping_mul(0x9E37_79B1);
    value ^ (value >> 15)
}

/// A fake screenshot: a colour wash, panels and rows of text-like blocks, different for each index.
fn synthetic_capture(index: usize) -> RgbaImage {
    let seed = mix(index as u32 + 1);
    let (red, green, blue) = (seed as u8, (seed >> 8) as u8, (seed >> 16) as u8);
    let (width, height) = CAPTURE_SIZE;
    RgbaImage::from_fn(width, height, |x, y| {
        let panel = mix(seed ^ (x / 180) ^ ((y / 150) << 12));
        let row = y % 18;
        let glyph = mix(panel ^ (x / 7) ^ ((y / 18) << 20));
        let is_text = row > 3 && row < 15 && !panel.is_multiple_of(3) && !glyph.is_multiple_of(4);
        let shade = (x * 90 / width + y * 90 / height) as u8;
        let channels = if is_text {
            [30, 32, 38]
        } else {
            [
                red.wrapping_add(shade).wrapping_add(panel as u8 / 4),
                green
                    .wrapping_add(shade)
                    .wrapping_add((panel >> 8) as u8 / 4),
                blue.wrapping_add(shade)
                    .wrapping_add((panel >> 16) as u8 / 4),
            ]
        };
        Rgba([channels[0], channels[1], channels[2], 255])
    })
}

struct Thumbnail {
    image: Arc<RenderImage>,
    decode_time: Duration,
    png_size: usize,
}

/// The path a real card takes: read a PNG capture, decode it, shrink it. The first two lines stand
/// in for the file on disk, and only the decode and downscale are timed.
fn make_thumbnail(index: usize, thumb_width: u32) -> Thumbnail {
    let mut png = Vec::new();
    synthetic_capture(index)
        .write_to(&mut Cursor::new(&mut png), ImageFormat::Png)
        .expect("failed to encode a capture");

    let decode_started = Instant::now();
    let capture = load_from_memory_with_format(&png, ImageFormat::Png)
        .expect("failed to decode a capture")
        .into_rgba8();
    let thumb_height = thumb_width * CAPTURE_SIZE.1 / CAPTURE_SIZE.0;
    let mut thumbnail = imageops::thumbnail(&capture, thumb_width, thumb_height);
    let decode_time = decode_started.elapsed();

    // GPUI's renderer takes BGRA.
    thumbnail.pixels_mut().for_each(|pixel| pixel.0.swap(0, 2));
    Thumbnail {
        image: Arc::new(RenderImage::new([Frame::new(thumbnail)])),
        decode_time,
        png_size: png.len(),
    }
}

#[repr(C)]
#[derive(Default)]
struct ProcessMemoryCounters {
    size: u32,
    page_fault_count: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged_pool: usize,
    quota_paged_pool: usize,
    quota_peak_non_paged_pool: usize,
    quota_non_paged_pool: usize,
    private_commit: usize,
    peak_private_commit: usize,
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn K32GetProcessMemoryInfo(
        process: isize,
        counters: *mut ProcessMemoryCounters,
        size: u32,
    ) -> i32;
}

fn print_memory(label: &str) {
    const CURRENT_PROCESS: isize = -1;
    const MB: f64 = 1024. * 1024.;
    let mut counters = ProcessMemoryCounters {
        size: size_of::<ProcessMemoryCounters>() as u32,
        ..Default::default()
    };
    let size = counters.size;
    // SAFETY: `counters` is a writable PROCESS_MEMORY_COUNTERS of the size passed.
    let ok = unsafe { K32GetProcessMemoryInfo(CURRENT_PROCESS, &mut counters, size) } != 0;
    if !ok {
        println!("memory {label}: unavailable");
        return;
    }
    println!(
        "memory {label}: working set {:.1} MB, private {:.1} MB",
        counters.working_set as f64 / MB,
        counters.private_commit as f64 / MB,
    );
}

fn primary_refresh_ms() -> f32 {
    let primary = Monitor::all()
        .expect("failed to list monitors")
        .into_iter()
        .find(|monitor| monitor.is_primary().unwrap_or(false));
    let hz = primary
        .and_then(|monitor| monitor.frequency().ok())
        .filter(|hz| *hz > 0.)
        .unwrap_or(60.);
    println!("primary monitor refresh {hz:.0} Hz");
    1000. / hz
}

fn main() {
    let started = Instant::now();
    let target_ms = primary_refresh_ms();
    let autoscroll = env::var_os("TORU_SPIKE_AUTOSCROLL").is_some();
    let autoclose = env::var("TORU_SPIKE_AUTOCLOSE_MS")
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_millis);
    let thumb_width = env::var("TORU_SPIKE_THUMB_WIDTH")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_THUMB_WIDTH);
    println!(
        "thumbnails {thumb_width} px wide, {} KB each when decoded",
        thumb_width * thumb_width * CAPTURE_SIZE.1 / CAPTURE_SIZE.0 * 4 / 1024
    );

    application().run(move |cx: &mut App| {
        gpui_kit::init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1000.), px(900.)),
                cx,
            ))),
            ..Default::default()
        };
        gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| Gallery::new(thumb_width, autoscroll, target_ms, started, window, cx))
        })
        .expect("failed to open the window");

        if let Some(delay) = autoclose {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(delay).await;
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}
