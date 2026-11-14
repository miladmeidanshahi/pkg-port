//! Small view helpers and custom components built on gpui-kit primitives.

use gpui_kit::component::{button::Button, IconName, Sizable as _};
use gpui_kit::ElementId;

pub mod icons;
pub mod progress;
pub(crate) mod steps;

pub use icons::AppAssets;
pub use progress::upload_progress_bar;

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
