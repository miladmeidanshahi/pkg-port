//! Step 4: console upload view and its telemetry helpers.

use crate::ui::{copy_button, upload_progress_bar};
use crate::{package_parts_from, App};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{button::Button, ActiveTheme as _, Icon, IconName};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::time::Duration;

fn format_size(bytes: u64) -> String {
    let (size, unit) = if bytes >= 1_000_000_000 {
        (bytes as f64 / 1_000_000_000.0, "GB")
    } else if bytes >= 1_000_000 {
        (bytes as f64 / 1_000_000.0, "MB")
    } else if bytes >= 1_000 {
        (bytes as f64 / 1_000.0, "KB")
    } else {
        return format!("{bytes} B");
    };
    let rounded = (size * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{rounded:.0} {unit}")
    } else {
        format!("{rounded:.1} {unit}")
    }
}

impl App {
    pub(crate) fn scroll_to_active_package(&self) {
        let Some(active) = self.transfers.active.last() else {
            return;
        };
        if let Some(index) = self
            .package_parts()
            .iter()
            .position(|(name, _)| *name == active.file)
        {
            self.package_scroll.scroll_to_item(index);
        }
    }

    /// Package parts the console will request, with their local file sizes.
    pub(crate) fn package_parts(&self) -> Vec<(String, u64)> {
        let Some(Ok(inspection)) = &self.folder_inspection else {
            return Vec::new();
        };
        let required = match &self.part_discovery_result {
            Some(Ok(required)) => Some(*required),
            _ => None,
        };
        package_parts_from(&inspection.files, &self.game_urls, required)
    }

    /// Bytes the console has received and the size of the whole package.
    pub(crate) fn upload_totals(&self) -> (u64, u64) {
        let parts = self.package_parts();
        let total = parts.iter().map(|(_, size)| *size).sum();
        let served = parts
            .iter()
            .map(|(name, size)| {
                self.transfers
                    .files
                    .get(name)
                    .map_or(0, |file| file.served.min(*size))
            })
            .sum();
        (served, total)
    }

    pub(crate) fn transfer_done(&self, name: &str, size: u64) -> bool {
        size > 0
            && self
                .transfers
                .files
                .get(name)
                .is_some_and(|file| file.served >= size)
    }

    pub(crate) fn upload_done(&self) -> bool {
        let (served, total) = self.upload_totals();
        total > 0 && served >= total
    }
    pub(crate) fn upload_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
        narrow: bool,
    ) -> AnyElement {
        let language = self.language;
        let rtl = language.is_rtl();
        let parts = self.package_parts();
        let (served, total) = self.upload_totals();
        let done = self.upload_done();
        let active = self.transfers.active.last();
        let uploading = !done && (active.is_some() || self.upload_status_until.is_some());
        let percent = if total > 0 {
            served as f32 / total as f32 * 100.0
        } else {
            0.0
        };
        let active_url = active.map(|transfer| transfer.url.clone());
        let completed = parts
            .iter()
            .filter(|(name, size)| self.transfer_done(name, *size))
            .count();
        let displayed_rate = if uploading { self.upload_rate } else { 0.0 };
        let (status_key, border, tint, color) = if done {
            (
                "app.files_sent",
                cx.theme().success,
                rgb(0x202D25).into(),
                cx.theme().success,
            )
        } else if uploading {
            (
                "app.uploading",
                rgb(0x6C3632).into(),
                rgb(0x362321).into(),
                cx.theme().primary_hover,
            )
        } else {
            (
                "app.waiting_for_console",
                cx.theme().border,
                cx.theme().group_box,
                cx.theme().muted_foreground,
            )
        };
        let route_border = if uploading {
            rgb(0x6C3632).into()
        } else if done {
            rgb(0x35634A).into()
        } else {
            cx.theme().border
        };
        let route_icon = if done {
            cx.theme().success
        } else if uploading {
            cx.theme().primary_hover
        } else {
            cx.theme().foreground
        };
        let route_bg = cx.theme().background;
        let caption_color = cx.theme().muted_foreground;
        let packet_color = cx.theme().primary_hover;
        let route_node = move |icon: assets::IconName, label: String| {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .flex_shrink_0()
                .child(
                    div()
                        .w(px(38.))
                        .h(px(38.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .border_1()
                        .border_color(route_border)
                        .bg(route_bg)
                        .child(Icon::new(icon).text_lg().text_color(route_icon)),
                )
                .child(div().text_xs().text_color(caption_color).child(label))
        };
        let route_link_border = if uploading {
            rgb(0x5A302D).into()
        } else {
            cx.theme().border
        };
        let route_link = move |id: usize| {
            let link = div()
                .flex_1()
                .min_w_0()
                .relative()
                .mt(px(18.))
                .h(px(2.))
                .rounded_full()
                .bg(route_link_border);
            if !uploading {
                return link.into_any_element();
            }
            // GPUI repaints the packets every frame; the 100 ms poll ticks
            // made them jump.
            link.with_animation(
                ("upload-route-link", id),
                Animation::new(Duration::from_millis(1800)).repeat(),
                move |link, progress| {
                    link.children((0..3).map(move |index| {
                        // In RTL the route runs right to left, so the packets
                        // travel the mirrored way.
                        let t = (progress + index as f32 / 3.0) % 1.0;
                        let phase = if rtl { 1.0 - t } else { t };
                        div()
                            .absolute()
                            .top(px(-2.))
                            .left(relative(phase * 0.94))
                            .w(px(6.))
                            .h(px(6.))
                            .rounded_full()
                            .bg(packet_color)
                    }))
                },
            )
            .into_any_element()
        };
        let transfer_card = div()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .gap_3()
            .p_4()
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .font_semibold()
                            .child(language.text("app.console_transfer")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_full()
                            .border_1()
                            .border_color(border)
                            .bg(tint)
                            .text_xs()
                            .text_color(color)
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(color))
                            .child(language.text(status_key)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(route_node(
                        assets::IconName::Monitor,
                        language.text("app.this_computer"),
                    ))
                    .child(route_link(0))
                    .child(route_node(
                        assets::IconName::Network,
                        language.text("app.local_proxy"),
                    ))
                    .child(route_link(1))
                    .child(route_node(
                        assets::IconName::Gamepad2,
                        language.text("app.playstation"),
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_3()
                    .pt_3()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text("app.overall_progress")),
                            )
                            .child(
                                div()
                                    .text_xl()
                                    .font_semibold()
                                    .child(format!("{}%", percent.floor() as u64)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text("app.upload_speed")),
                            )
                            .child(
                                // Separate runs: gpui drops the leading digits
                                // of an RTL-shaped mixed-direction string.
                                div()
                                    .flex()
                                    .items_baseline()
                                    .gap_1()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .text_xl()
                                    .font_semibold()
                                    .text_color(if uploading {
                                        cx.theme().primary_hover
                                    } else {
                                        cx.theme().foreground
                                    })
                                    .child(format!("{displayed_rate:.0}"))
                                    .child(language.text("app.mb_per_second")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text("app.data_sent")),
                            )
                            .child(div().text_base().font_semibold().child(format!(
                                "{} / {}",
                                format_size(served),
                                format_size(total)
                            ))),
                    ),
            )
            .child(upload_progress_bar(
                "upload-overall",
                percent,
                if done {
                    cx.theme().success
                } else {
                    cx.theme().primary
                },
                px(7.),
                total == 0 && self.transfers.bytes > 0,
                rtl,
                language.text("app.overall_upload_progress"),
                window,
                cx,
            ));
        let mut active_files: Vec<(String, u64, u64)> = Vec::new();
        for transfer in &self.transfers.active {
            let file = self.transfers.files.get(&transfer.file);
            let size = file.map_or(transfer.total, |file| file.size);
            let sent = file.map_or(transfer.sent, |file| file.served.min(file.size));
            match active_files
                .iter_mut()
                .find(|(name, _, _)| *name == transfer.file)
            {
                Some(row) => {
                    row.1 = row.1.max(size);
                    row.2 = row.2.max(sent);
                }
                None => active_files.push((transfer.file.clone(), size, sent)),
            }
        }
        let mut file_rows = Vec::new();
        if active_files.is_empty() {
            file_rows.push(
                div()
                    .text_sm()
                    .font_semibold()
                    .child(language.text("app.no_file_uploading"))
                    .into_any_element(),
            );
        }
        for (position, (name, size, sent)) in active_files.iter().enumerate() {
            file_rows.push(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .mt_1()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_baseline()
                            .justify_between()
                            .gap_2()
                            .when(rtl, |row| row.flex_row_reverse())
                            .child(
                                div()
                                    .id(("upload-file-name", position))
                                    .test_support()
                                    .text_sm()
                                    .font_semibold()
                                    .child(name.clone()),
                            )
                            .child(
                                div()
                                    .id(("upload-file-size", position))
                                    .test_support()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "{} / {}",
                                        format_size(*sent),
                                        format_size(*size)
                                    )),
                            ),
                    )
                    .child(upload_progress_bar(
                        ("upload-file", position),
                        if *size > 0 {
                            *sent as f32 / *size as f32 * 100.0
                        } else {
                            0.0
                        },
                        cx.theme().primary,
                        px(4.),
                        false,
                        rtl,
                        language.text("app.current_file_progress"),
                        window,
                        cx,
                    ))
                    .into_any_element(),
            );
        }
        let active_card = div()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .gap_2()
            .p_4()
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .font_semibold()
                    .child(language.text("app.current_request_url")),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .min_w_0()
                    .id("upload-url-row")
                    .test_support()
                    .when(rtl, |row| row.flex_row_reverse())
                    .p_2()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .child(
                        div()
                            .id("upload-url-value")
                            .test_support()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .text_left()
                            .text_xs()
                            .child(active_url.clone().unwrap_or_else(|| {
                                language.text(if done {
                                    "app.no_active_request"
                                } else {
                                    "app.waiting_for_package_request"
                                })
                            })),
                    )
                    .when_some(active_url, |row, url| {
                        row.child(
                            copy_button(
                                "copy-upload-url",
                                self.copy_feedback == Some("app.url_copied"),
                                language.text("app.copy_url"),
                            )
                            .flex_shrink_0()
                            .on_click(cx.listener(
                                move |app, _, window, cx| {
                                    app.copy_to_clipboard(
                                        url.clone(),
                                        "app.url_copied",
                                        window,
                                        cx,
                                    );
                                },
                            )),
                        )
                    }),
            )
            .child(div().flex().flex_col().gap_2().children(file_rows));
        let mut queue = div()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .gap_2()
            .p_4()
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_lg()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .font_semibold()
                            .child(language.text("app.package_files")),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(rtl, |row| row.flex_row_reverse())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{completed} / {}", parts.len()))
                            .child(language.text("app.sent")),
                    ),
            );
        let mut queue_files = div()
            .id("package-file-list")
            .test_support()
            .flex()
            .flex_col()
            .min_h_0()
            .max_h(rems(20.))
            .overflow_y_scroll()
            .track_scroll(&self.package_scroll);
        for (name, size) in &parts {
            let sent = self.transfer_done(name, *size);
            let interrupted = self
                .transfers
                .files
                .get(name)
                .is_some_and(|file| file.interrupted);
            let is_active = active.is_some_and(|transfer| transfer.file == *name);
            let (row_tint, row_color, state_key) = if sent {
                (cx.theme().group_box, cx.theme().success, "app.sent_state")
            } else if is_active {
                (
                    rgb(0x392623).into(),
                    cx.theme().primary_hover,
                    "app.uploading",
                )
            } else if interrupted {
                (cx.theme().group_box, cx.theme().danger, "app.interrupted")
            } else {
                (
                    cx.theme().group_box,
                    cx.theme().muted_foreground,
                    "app.waiting",
                )
            };
            queue_files = queue_files.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(row_tint)
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(Icon::new(if sent {
                        assets::IconName::CircleCheck
                    } else {
                        assets::IconName::Package
                    }))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .child(name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(row_color)
                            .child(language.text(state_key)),
                    ),
            );
        }
        queue = queue.child(queue_files);
        div()
            .flex()
            .flex_col()
            .min_w_0()
            .gap_4()
            .mt_2()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .min_w_0()
                    .overflow_hidden()
                    .gap_1()
                    .child(
                        div()
                            .text_3xl()
                            .font_bold()
                            .child(language.text("app.upload_to_console")),
                    )
                    .child(
                        div()
                            .w_full()
                            .whitespace_normal()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text("app.upload_intro")),
                    ),
            )
            .child(transfer_card)
            .child(
                div()
                    .flex()
                    .gap_3()
                    .when(narrow, |row| row.flex_col())
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .flex_grow(1.45)
                            .flex_basis(px(0.))
                            .min_w_0()
                            .child(active_card),
                    )
                    .child(
                        div()
                            .flex_grow(1.0)
                            .flex_basis(px(0.))
                            .min_w_0()
                            .child(queue),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        Button::new("back-to-folder")
                            .outline()
                            .child(
                                div()
                                    .id("upload-back-content")
                                    .test_support()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(div().id("upload-back-icon").test_support().child(
                                        Icon::new(if rtl {
                                            IconName::ArrowRight
                                        } else {
                                            IconName::ArrowLeft
                                        }),
                                    ))
                                    .child(
                                        div()
                                            .id("upload-back-label")
                                            .test_support()
                                            .child(language.text("app.back_to_folder")),
                                    ),
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.current_step = 2;
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::format_size;

    #[test]
    fn formats_transfer_sizes_with_readable_units() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(500_000_000), "500 MB");
        assert_eq!(format_size(2_472_000_000), "2.5 GB");
        assert_eq!(format_size(10_000_000_000), "10 GB");
        assert_eq!(format_size(222_929_000_000), "222.9 GB");
    }
}
