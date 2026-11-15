//! Small view helpers and custom components built on gpui-kit primitives.

use gpui_kit::component::{button::Button, IconName, Sizable as _};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{div, Div, ElementId, ParentElement as _, Styled as _};

pub mod icons;
pub mod progress;
pub(crate) mod steps;

pub use icons::AppAssets;
pub use progress::upload_progress_bar;

pub(crate) fn wrapped_text(text: String, rtl: bool) -> Div {
    div()
        .w_full()
        .min_w_0()
        .whitespace_normal()
        .when(rtl, |view| {
            // shortcut: GPUI 0.9.1 clips soft-wrapped RTL glyphs; use flex words until its text renderer is fixed.
            view.flex().flex_col().children(text.lines().map(|line| {
                div()
                    .flex()
                    .flex_row_reverse()
                    .flex_wrap()
                    .gap_x_1()
                    .children(line.split_whitespace().map(|word| {
                        div()
                            .flex_shrink_0()
                            .whitespace_nowrap()
                            .child(word.to_owned())
                    }))
            }))
        })
        .when(!rtl, |view| view.child(text))
}

pub fn copy_button(id: impl Into<ElementId>, copied: bool, label: String) -> Button {
    Button::new(id)
        .outline()
        .xsmall()
        .icon(if copied {
            IconName::CircleCheck
        } else {
            IconName::Copy
        })
        .accessibility_label(label)
}

/// Chevron that points "forward" in the reading direction.
pub fn disclosure_chevron(open: bool, rtl: bool) -> IconName {
    if open {
        IconName::ChevronDown
    } else if rtl {
        IconName::ChevronLeft
    } else {
        IconName::ChevronRight
    }
}
