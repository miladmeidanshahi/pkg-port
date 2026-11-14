//! A linear progress bar that fills from the start side of the reading
//! direction.

use gpui_kit::base::{
    transition, Progress as BaseProgress, ProgressIndicator, ProgressTrack, Transition,
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

/// Linear progress bar that fills from the start side (right in RTL).
///
/// gpui-component's `Progress` anchors its indicator to the left with no RTL
/// support, so this builds the same pill from gpui-kit's base primitives.
#[allow(clippy::too_many_arguments)]
pub fn upload_progress_bar(
    id: impl Into<ElementId>,
    percent: f32,
    color: Hsla,
    height: Pixels,
    loading: bool,
    rtl: bool,
    label: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let id = id.into();
    let radius = height / 2.;
    let animated = transition(
        (id.clone(), "indicator"),
        percent,
        Transition::new(cx.theme().motion_tokens().duration_normal)
            .easing(cx.theme().motion_tokens().easing_move.clone()),
        window,
        cx,
    );
    let reduce_motion = cx.reduce_motion();
    let bg: Background = color.into();
    BaseProgress::new(id)
        .value(percent)
        .accessibility_label(label)
        .w_full()
        .relative()
        .h(height)
        .rounded(radius)
        .child(
            ProgressTrack::new()
                .absolute()
                .size_full()
                .rounded(radius)
                .bg(bg.opacity(0.2)),
        )
        .child(
            ProgressIndicator::new()
                .absolute()
                .top_0()
                .h_full()
                .rounded(radius)
                .bg(color)
                .map(|bar| {
                    if loading && !reduce_motion {
                        let slide = move |bar: ProgressIndicator, delta: f32| {
                            // Mirrored slide of the loading sweep.
                            let (start, end) = if rtl {
                                (
                                    relative(ease_in_out(1.0 - delta)),
                                    relative(ease_in_out(((delta - 0.5) / 0.5).clamp(0., 1.))),
                                )
                            } else {
                                (
                                    relative(ease_in_out(((delta - 0.5) / 0.5).clamp(0., 1.))),
                                    relative(ease_in_out(1.0 - delta)),
                                )
                            };
                            bar.left(start).right(end)
                        };
                        bar.with_animation(
                            "loading",
                            Animation::new(Duration::from_secs(1)).repeat(),
                            slide,
                        )
                        .into_any_element()
                    } else if loading {
                        bar.left(relative(0.325))
                            .right(relative(0.325))
                            .into_any_element()
                    } else {
                        let width = relative((animated / 100.).clamp(0., 1.));
                        if rtl {
                            bar.right_0().w(width)
                        } else {
                            bar.left_0().w(width)
                        }
                        .into_any_element()
                    }
                }),
        )
        .into_any_element()
}
