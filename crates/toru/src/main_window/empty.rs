//! The gallery's empty state: what a new user sees under "This session" before the first
//! capture, in place of a blank area. It names the Region button and, when one is working, the
//! Region shortcut, so the first screenshot is one press away. The mockups show no empty
//! gallery, so each look draws it from its own parts (`theme::EmptyState`): Japandi a serif
//! line under a round seal with air around it, Pixel art its loading row's dashed frame around
//! the Region icon in the accent colour on a hard-edged tile with the look's block shadow,
//! Neo-brutalism a blunt statement beside a block of thumbnail colour.
//! It is muted text on the window or the placeholder fill, never a warning colour, so it reads
//! as an invitation and not as an error.

use std::ops::Range;

use gpui::{Div, HighlightStyle, Stateful, StyledText, div, prelude::*, px};

use super::HINT_LEADING;
use crate::theme::{Ornament, Theme, ThemedText};
use crate::ui::{Icon, Readable, fill, shadowed};

/// Above an unframed empty state, so it sits in the space rather than under the header's chin.
const UNFRAMED_TOP: f32 = 56.;
/// Inside the frame.
const FRAME_PADDING_X: f32 = 24.;
const FRAME_PADDING_Y: f32 = 28.;
/// Between the ornament and the words, and between the title and the instruction.
const ORNAMENT_GAP: f32 = 16.;
const TITLE_GAP: f32 = 8.;
/// The instruction wraps at a comfortable line, which holds it whole in every look while no
/// shortcut is named.
const TEXT_WIDTH: f32 = 440.;
/// The seal, the tile and the block, and the icon in them.
const ORNAMENT_SIZE: f32 = 56.;
const ORNAMENT_ICON: f32 = 24.;

/// The instruction, and where in it the names to pick out are: the Region button and the
/// shortcut, which is `None` when no Region shortcut is set or working.
fn instruction(shortcut: Option<&str>) -> (String, Vec<Range<usize>>) {
    const BUTTON: &str = "Region";
    let mut text = String::from("Press ");
    let mut names = Vec::new();
    let mut name = |text: &mut String, name: &str| {
        names.push(text.len()..text.len() + name.len());
        text.push_str(name);
    };
    name(&mut text, BUTTON);
    text.push_str(" in the toolbar");
    if let Some(shortcut) = shortcut {
        text.push_str(", or ");
        name(&mut text, shortcut);
        text.push_str(" from anywhere,");
    }
    text.push_str(" to take your first screenshot.");
    (text, names)
}

/// The headline and the instruction as one sentence each, whatever the headline's own stop.
fn spoken_text(title: &str, instruction: &str) -> String {
    format!("{}. {instruction}", title.trim_end_matches('.'))
}

pub fn empty_state(shortcut: Option<&str>, theme: &Theme) -> Stateful<Div> {
    let c = &theme.colors;
    let look = &theme.empty;
    let title_style = theme.type_scale.section;
    let body_style = theme.type_scale.body;
    let title = title_style.case.apply(look.title);
    let (text, names) = instruction(shortcut);
    let named = HighlightStyle {
        color: Some(c.text),
        ..Default::default()
    };
    let words = div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(TITLE_GAP))
        .when(look.centered, |words| words.items_center().text_center())
        .child(
            div()
                .type_style(theme, title_style)
                .text_color(c.text)
                .child(title),
        )
        .child(
            div()
                .max_w(px(TEXT_WIDTH))
                .type_style(theme, body_style)
                .line_height(body_style.size * HINT_LEADING)
                .text_color(theme.window_muted())
                .child(
                    StyledText::new(text.clone())
                        .with_highlights(names.into_iter().map(|name| (name, named))),
                ),
        );
    let content = div()
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(ORNAMENT_GAP))
        .when(look.centered, |content| content.flex_col())
        .child(ornament(look.ornament, theme))
        .child(words);
    let frame = theme.borders.load;
    div()
        .id("empty")
        .read_as_text(spoken_text(look.title, &text))
        .w_full()
        .flex()
        .when(look.centered, |this| this.justify_center())
        .when(look.framed, |this| {
            this.px(px(FRAME_PADDING_X))
                .py(px(FRAME_PADDING_Y))
                .bg(c.load_bg)
                .border(frame.width)
                .border_color(frame.color)
                .when(frame.dashed, |this| this.border_dashed())
                .rounded(theme.radii.md)
        })
        .when(!look.framed, |this| this.pt(px(UNFRAMED_TOP)))
        .child(content)
}

fn ornament(ornament: Ornament, theme: &Theme) -> Div {
    let c = &theme.colors;
    let icon = |color| Icon::Region.render(theme, color).size(px(ORNAMENT_ICON));
    let tile = div()
        .relative()
        .flex_none()
        .size(px(ORNAMENT_SIZE))
        .flex()
        .items_center()
        .justify_center();
    match ornament {
        Ornament::Seal => {
            let border = theme.borders.card;
            tile.bg(c.surface)
                .border(border.width)
                .border_color(border.color)
                .rounded(theme.radii.pill)
                .child(icon(c.muted))
        }
        Ornament::Tile => {
            let border = theme.borders.chip;
            let radius = theme.radii.md;
            shadowed(
                tile.bg(c.surface)
                    .border(border.width)
                    .border_color(border.color)
                    .rounded(radius),
                &theme.shadows.accent,
                radius,
            )
            .child(icon(c.accent))
        }
        Ornament::Block => {
            let border = theme.borders.thumb;
            let radius = theme.radii.md;
            tile.border(border.width)
                .border_color(border.color)
                .rounded(radius)
                .child(div().absolute().inset_0().child(fill(&c.thumb, radius)))
                // The thumbnail's own outline is its ink, whatever the block's colour.
                .child(icon(border.color))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_instruction_names_the_button_and_a_working_shortcut() {
        let (text, names) = instruction(None);
        assert_eq!(
            text,
            "Press Region in the toolbar to take your first screenshot."
        );
        let named: Vec<&str> = names.iter().map(|name| &text[name.clone()]).collect();
        assert_eq!(named, ["Region"]);

        let (text, names) = instruction(Some("Ctrl + Shift + S"));
        assert_eq!(
            text,
            "Press Region in the toolbar, or Ctrl + Shift + S from anywhere, to take your first \
             screenshot."
        );
        let named: Vec<&str> = names.iter().map(|name| &text[name.clone()]).collect();
        assert_eq!(named, ["Region", "Ctrl + Shift + S"]);
    }

    #[test]
    fn the_headline_and_instruction_are_read_as_two_sentences() {
        let (instruction, _) = instruction(None);
        let expected = "Nothing yet. Press Region in the toolbar to take your first screenshot.";
        assert_eq!(spoken_text("Nothing yet.", &instruction), expected);
        assert_eq!(spoken_text("Nothing yet", &instruction), expected);
    }
}
