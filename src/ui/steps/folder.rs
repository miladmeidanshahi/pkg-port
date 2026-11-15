//! Step 3: PKG folder view.

use crate::ui::copy_button;
use crate::{folder, parts, App};
use gpui_kit::base::StyledExt as _;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    ActiveTheme as _, Icon, IconName,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl App {
    pub(crate) fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(self.language.text("app.select_pkg_folder").into()),
        });
        cx.spawn(async move |view, cx| match selection.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.into_iter().next() {
                    let _ = view.update(cx, |app, cx| {
                        if let Some(proxy) = &app.proxy {
                            if let Err(error) = proxy.set_package_dir(Some(path.clone())) {
                                app.message = error.to_string();
                                cx.notify();
                                return;
                            }
                        }
                        app.folder_check = Some(folder::inspect(path.clone()));
                        app.folder_inspection = None;
                        app.package_dir = Some(path);
                        app.message.clear();
                        app.copy_feedback = None;
                        app.persist_settings(cx);
                        cx.notify();
                    });
                }
            }
            Ok(Ok(None)) => {}
            result => {
                let _ = view.update(cx, |app, cx| {
                    app.message = format!(
                        "{}: {result:?}",
                        app.language.text("app.folder_picker_failed")
                    );
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub(crate) fn clear_folder(&mut self, cx: &mut Context<Self>) {
        if let Some(proxy) = &self.proxy {
            if let Err(error) = proxy.set_package_dir(None) {
                self.message = error.to_string();
                cx.notify();
                return;
            }
        }
        self.folder_check = None;
        self.folder_inspection = None;
        self.package_dir = None;
        self.message.clear();
        self.copy_feedback = None;
        self.persist_settings(cx);
        cx.notify();
    }

    pub(crate) fn folder_view(&self, cx: &mut Context<Self>, narrow: bool) -> AnyElement {
        let language = self.language;
        let rtl = language.is_rtl();
        let (status_key, detail, error) = if self.package_dir.is_none() {
            (
                "app.no_folder_selected",
                language.text("app.select_the_folder_containing_complete_pkg_parts"),
                false,
            )
        } else if self.folder_check.is_some() {
            ("app.checking_folder", String::new(), false)
        } else if let Some(Err(reason)) = &self.folder_inspection {
            ("app.unreadable_folder", reason.clone(), true)
        } else if let Some(Ok(inspection)) = &self.folder_inspection {
            let count = inspection.files.len();
            if count == 0 {
                ("app.no_pkg_files", "0 .pkg".into(), true)
            } else if let (Some(Ok(required)), Some(first)) = (
                self.part_discovery_result.as_ref(),
                self.game_urls
                    .iter()
                    .find(|url| parts::part_index(url) == Some(0)),
            ) {
                let found = folder::matching_parts(first, *required, &inspection.files);
                if *required > 0 && found == *required {
                    (
                        "app.package_ready",
                        format!("{count} .pkg · {found}/{required}"),
                        false,
                    )
                } else {
                    (
                        "app.missing_required_parts",
                        format!("{count} .pkg · {found}/{required}"),
                        true,
                    )
                }
            } else {
                (
                    "app.pkg_files_found",
                    language.text("app.readiness_unverified"),
                    false,
                )
            }
        } else {
            ("app.checking_folder", String::new(), false)
        };
        let folder_name = self
            .package_dir
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| language.text("app.choose_folder"));
        let path = self
            .package_dir
            .as_ref()
            .map(|path| path.display().to_string());
        let folder_selected = self.package_dir.is_some() && !error && self.folder_check.is_none();
        let folder_count = self
            .folder_inspection
            .as_ref()
            .and_then(|result| result.as_ref().ok())
            .map(|inspection| inspection.files.len());
        let folder_summary = if let Some(count) = folder_count {
            div()
                .flex()
                .items_center()
                .gap_1()
                .when(rtl, |row| row.flex_row_reverse())
                .child(
                    div()
                        .id("folder-pkg-count")
                        .test_support()
                        .flex_shrink_0()
                        .text_left()
                        .child(count.to_string()),
                )
                .child(
                    div()
                        .id("folder-pkg-count-label")
                        .test_support()
                        .child(language.text("app.pkg_files_count")),
                )
                .into_any_element()
        } else {
            crate::ui::wrapped_text(
                language.text("app.select_the_folder_containing_complete_pkg_parts"),
                rtl,
            )
            .into_any_element()
        };
        let card = div()
            .flex()
            .flex_col()
            .min_w_0()
            .overflow_hidden()
            .gap_4()
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
                            .child(language.text("app.local_package_folder")),
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
                            .border_color(if folder_selected {
                                rgb(0x35634A).into()
                            } else {
                                cx.theme().border
                            })
                            .when(folder_selected, |badge| badge.bg(rgb(0x202D25)))
                            .text_xs()
                            .text_color(if error {
                                cx.theme().danger
                            } else if folder_selected {
                                cx.theme().success
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(if error {
                                cx.theme().danger
                            } else if folder_selected {
                                cx.theme().success
                            } else {
                                cx.theme().muted_foreground
                            }))
                            .child(language.text(if folder_selected {
                                "app.folder_selected"
                            } else {
                                status_key
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .p_3()
                    .min_w_0()
                    .bg(cx.theme().background)
                    .border_1()
                    .border_dashed()
                    .border_color(rgb(0x424249))
                    .rounded_md()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .w(px(34.))
                            .h(px(34.))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(cx.theme().group_box)
                            .child(Icon::new(IconName::FolderOpen).text_color(cx.theme().primary)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap_1()
                            .min_w_0()
                            .when(rtl, |column| column.text_right())
                            .child(div().truncate().font_semibold().child(folder_name))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(folder_summary),
                            ),
                    )
                    .when(self.package_dir.is_some(), |row| {
                        row.child(
                            Button::new("clear-folder")
                                .outline()
                                .icon(IconName::CircleX)
                                .tooltip(language.text("app.clear_selected_folder"))
                                .accessibility_label(language.text("app.clear_selected_folder"))
                                .flex_shrink_0()
                                .on_click(cx.listener(|app, _, _, cx| app.clear_folder(cx))),
                        )
                    }),
            )
            .when_some(path, |view, path| {
                let copied_path = path.clone();
                view.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .id("folder-path-row")
                        .test_support()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(
                            div()
                                .id("folder-path-value")
                                .test_support()
                                .when(rtl, |value| value.text_right())
                                .when(!rtl, |value| value.text_left())
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(path),
                        )
                        .child(
                            copy_button(
                                "copy-folder-path",
                                self.copy_feedback == Some("app.path_copied"),
                                language.text("app.copy_folder_path"),
                            )
                            .flex_shrink_0()
                            .on_click(cx.listener(
                                move |app, _, window, cx| {
                                    app.copy_to_clipboard(
                                        copied_path.clone(),
                                        "app.path_copied",
                                        window,
                                        cx,
                                    );
                                },
                            )),
                        ),
                )
            })
            .when(!self.message.is_empty(), |view| {
                view.child(
                    div()
                        .text_color(cx.theme().danger)
                        .child(self.message.clone()),
                )
            })
            .when(!detail.is_empty(), |view| {
                view.child(
                    crate::ui::wrapped_text(detail, rtl && self.package_dir.is_none())
                        .text_color(if error {
                            cx.theme().danger
                        } else {
                            cx.theme().muted_foreground
                        })
                        .text_xs(),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_between()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        Button::new("back-to-game")
                            .outline()
                            .label(language.text("app.back_to_game"))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.current_step = 1;
                                cx.notify();
                            })),
                    )
                    .when(self.folder_ready(), |row| {
                        row.child(
                            Button::new("continue-to-upload")
                                .primary()
                                .child(
                                    div()
                                        .id("folder-next-content")
                                        .test_support()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .when(rtl, |row| row.flex_row_reverse())
                                        .child(div().id("folder-next-icon").test_support().child(
                                            Icon::new(if rtl {
                                                IconName::ArrowLeft
                                            } else {
                                                IconName::ArrowRight
                                            }),
                                        ))
                                        .child(
                                            div()
                                                .id("folder-next-label")
                                                .test_support()
                                                .child(language.text("app.continue_to_upload")),
                                        ),
                                )
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.current_step = 3;
                                    app.scroll_to_active_package();
                                    cx.notify();
                                })),
                        )
                    })
                    .child(
                        Button::new("choose-folder")
                            .primary()
                            .accessibility_label(language.text(if self.package_dir.is_some() {
                                "app.change_folder"
                            } else {
                                "app.choose_folder"
                            }))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(Icon::new(IconName::FolderOpen))
                                    .child(div().font_weight(FontWeight::BOLD).child(
                                        language.text(if self.package_dir.is_some() {
                                            "app.change_folder"
                                        } else {
                                            "app.choose_folder"
                                        }),
                                    )),
                            )
                            .on_click(cx.listener(|app, _, _, cx| app.choose_folder(cx))),
                    ),
            );
        let guide = div()
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
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .id("folder-guide-icon")
                            .test_support()
                            .child(Icon::new(assets::IconName::PackageCheck)),
                    )
                    .child(
                        div()
                            .id("folder-guide-title")
                            .test_support()
                            .font_semibold()
                            .child(language.text("app.folder_checklist")),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .min_w_0()
                    .whitespace_normal()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(language.text("app.folder_check_one")),
            )
            .child(
                crate::ui::wrapped_text(language.text("app.folder_check_two"), rtl)
                    .id("folder-guide-filenames")
                    .test_support()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            );
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
                            .child(language.text("app.pkg_folder")),
                    )
                    .child(
                        crate::ui::wrapped_text(
                            language.text("app.select_the_folder_containing_complete_pkg_parts"),
                            rtl,
                        )
                        .id("folder-intro")
                        .test_support()
                        .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .when(narrow, |row| row.flex_col())
                    .child(div().flex_1().min_w_0().child(card))
                    .child(
                        div()
                            .when(narrow, |view| view.w_full())
                            .when(!narrow, |view| view.w(px(280.)))
                            .child(guide),
                    ),
            )
            .into_any_element()
    }
}
