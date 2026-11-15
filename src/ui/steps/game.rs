//! Step 2: detected game view.

use crate::ui::{copy_button, disclosure_chevron};
use crate::MAX_GAME_URLS;
use crate::{parts, App};
use gpui_kit::base::{Disableable as _, StyledExt as _};
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    collapsible::Collapsible,
    group_box::{GroupBox, GroupBoxVariants as _},
    list::ListItem,
    scroll::ScrollableElement as _,
    skeleton::Skeleton,
    tooltip::Tooltip,
    ActiveTheme as _, Icon, IconName, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

impl App {
    pub(crate) fn game_step_view(&self, cx: &mut Context<Self>, narrow: bool) -> AnyElement {
        let language = self.language;
        let rtl = language.is_rtl();
        let main = if self.game.is_some() {
            self.game_view(cx)
        } else {
            div()
                .flex()
                .flex_col()
                .gap_4()
                .p_4()
                .bg(cx.theme().group_box)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .child(
                    div()
                        .font_semibold()
                        .child(language.text("app.detected_package")),
                )
                .child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(if self.proxy.is_some() {
                            "app.waiting_for_package"
                        } else {
                            "app.start_proxy_first"
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(
                            Button::new("game-back-proxy")
                                .outline()
                                .label(language.text("app.back_to_proxy"))
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.current_step = 0;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("game-next-folder")
                                .primary()
                                .accessibility_label(language.text("app.continue_to_folder"))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .child(language.text("app.continue_to_folder")),
                                )
                                .on_click(cx.listener(|app, _, _, cx| {
                                    app.current_step = 2;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element()
        };
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
                            .id("game-guide-icon")
                            .test_support()
                            .child(Icon::new(IconName::FolderOpen)),
                    )
                    .child(
                        div()
                            .id("game-guide-title")
                            .test_support()
                            .font_semibold()
                            .child(language.text("app.use_local_files")),
                    ),
            )
            .child(
                crate::ui::wrapped_text(language.text("app.use_local_files_help"), rtl)
                    .id("game-guide-help")
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
                            .child(language.text("app.game_content")),
                    )
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_color(cx.theme().muted_foreground)
                            .child(
                                language
                                    .text("app.game_content_detected_from_playstation_package_r"),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .when(narrow, |row| row.flex_col())
                    .child(div().flex_1().min_w_0().child(main))
                    .child(
                        div()
                            .when(narrow, |view| view.w_full())
                            .when(!narrow, |view| view.w(px(280.)))
                            .child(guide),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn game_view(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(game) = &self.game else {
            return GroupBox::new()
                .fill()
                .title(
                    div()
                        .text_color(cx.theme().foreground)
                        .font_bold()
                        .child(self.language.text("app.game_content")),
                )
                .child(
                    self.language
                        .text("app.no_game_package_detected_yet_start_the_proxy_and"),
                )
                .child(
                    Button::new("game-open-proxy")
                        .label(self.language.text("app.open_proxy_setup"))
                        .on_click(cx.listener(|app, _, _, cx| {
                            app.current_step = 0;
                            cx.notify();
                        })),
                )
                .into_any_element();
        };
        let loading = self.metadata.is_some();
        let rtl = self.language.is_rtl();
        let cover: AnyElement = if let Some(cover) = &self.cover {
            img(cover.clone())
                .id("game-cover")
                .w(px(100.))
                .h(px(100.))
                .rounded_md()
                .object_fit(ObjectFit::Cover)
                .into_any_element()
        } else if loading {
            Skeleton::new()
                .w(px(100.))
                .h(px(100.))
                .rounded_md()
                .into_any_element()
        } else {
            div()
                .w(px(100.))
                .h(px(100.))
                .rounded_md()
                .bg(cx.theme().muted)
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child(self.language.text("app.no_cover"))
                .into_any_element()
        };
        let title: AnyElement = if let Some(name) = &game.name {
            div()
                .w_full()
                .min_w_0()
                .whitespace_normal()
                .text_lg()
                .font_bold()
                .child(name.clone())
                .into_any_element()
        } else if loading {
            Skeleton::new().w_48().h_6().rounded_md().into_any_element()
        } else {
            div()
                .text_lg()
                .font_bold()
                .child(self.language.text("app.unknown_game"))
                .into_any_element()
        };
        let status = if loading {
            self.language.text("app.loading_game_details")
        } else if self.metadata_error.is_some() && game.name.is_none() {
            self.language.text("app.details_unavailable")
        } else {
            self.language.text("app.game_details_loaded")
        };
        let mut body = div().flex().flex_col().gap_4().child(
            div()
                .flex()
                .flex_wrap()
                .items_start()
                .gap_4()
                .when(rtl, |row| row.flex_row_reverse())
                .child(cover)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .flex_1()
                        .min_w_0()
                        .when(rtl, |col| col.items_end().text_right())
                        .child(title)
                        .child(
                            div()
                                .id("game-title-id-row")
                                .test_support()
                                .flex()
                                .w_full()
                                .gap_2()
                                .when(rtl, |row| row.flex_row_reverse())
                                .child(
                                    div()
                                        .id("game-title-id-label")
                                        .test_support()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.language.text("app.title_id")),
                                )
                                .child(
                                    div()
                                        .id("game-title-id-value")
                                        .test_support()
                                        .text_xs()
                                        .flex_1()
                                        .min_w_0()
                                        .child(game.title_id.clone()),
                                ),
                        )
                        .when_some(game.region, |view, region| {
                            view.child(
                                div()
                                    .id("game-region-row")
                                    .test_support()
                                    .flex()
                                    .w_full()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(
                                        div()
                                            .id("game-region-label")
                                            .test_support()
                                            .flex_shrink_0()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(self.language.text("app.package_region")),
                                    )
                                    .child(
                                        div()
                                            .id("game-region-value")
                                            .test_support()
                                            .text_xs()
                                            .flex_1()
                                            .min_w_0()
                                            .child(self.language.text(region)),
                                    ),
                            )
                        })
                        .child(
                            div()
                                .id("game-device-row")
                                .test_support()
                                .flex()
                                .w_full()
                                .gap_2()
                                .when(rtl, |row| row.flex_row_reverse())
                                .child(
                                    div()
                                        .id("game-device-label")
                                        .test_support()
                                        .flex_shrink_0()
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.language.text("app.device")),
                                )
                                .child(
                                    div()
                                        .id("game-device-value")
                                        .test_support()
                                        .text_xs()
                                        .flex_1()
                                        .min_w_0()
                                        .child(game.device.clone()),
                                ),
                        )
                        .when_some(self.metadata_error.as_ref(), |view, error| {
                            view.child(div().text_color(cx.theme().danger).child(error.clone()))
                                .child(
                                    Button::new("retry-game-metadata")
                                        .label(self.language.text("app.retry"))
                                        .on_click(
                                            cx.listener(|app, _, _, cx| app.retry_metadata(cx)),
                                        ),
                                )
                        }),
                ),
        );
        let mut url_panel = Collapsible::new()
            .open(self.show_game_urls)
            .min_w_0()
            .overflow_hidden()
            .bg(cx.theme().background)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .child(
                div()
                    .flex()
                    .justify_start()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        Button::new("toggle-game-urls")
                            .ghost()
                            .w_full()
                            .accessibility_label(self.language.text("app.package_urls"))
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(div().child(self.language.text("app.package_urls")))
                                    .child(Icon::new(disclosure_chevron(self.show_game_urls, rtl))),
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.show_game_urls = !app.show_game_urls;
                                cx.notify();
                            })),
                    ),
            );
        if self.show_game_urls {
            let mut listed_urls = self.game_urls.clone();
            listed_urls.sort_by_key(|url| parts::part_index(url).unwrap_or(usize::MAX));
            let all_urls = listed_urls.join("\n");
            let mut url_list = div()
                .flex()
                .flex_col()
                .min_w_0()
                .max_h(px(208.))
                .overflow_y_scrollbar();
            for (index, url) in listed_urls.iter().enumerate() {
                let copied_url = url.clone();
                let tooltip_url = url.clone();
                let copy_label = self.language.text("app.copy_url");
                url_list = url_list.child(
                    ListItem::new(("package-url", index))
                        .accessibility_label(url.clone())
                        .w_full()
                        .min_w_0()
                        .text_xs()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .w_full()
                                .min_w_0()
                                .when(rtl, |row| row.flex_row_reverse())
                                .child(
                                    div()
                                        .id(("package-url-text", index))
                                        .test_support()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .text_left()
                                        .text_color(cx.theme().muted_foreground)
                                        .tooltip(move |window, cx| {
                                            Tooltip::new(tooltip_url.clone())
                                                .bg(cx.theme().background)
                                                .build(window, cx)
                                        })
                                        .child(url.clone()),
                                )
                                .child(
                                    copy_button(
                                        ("copy-game-url", index),
                                        self.copy_feedback == Some("app.game_url_copied")
                                            && self.copied_game_url.as_deref()
                                                == Some(url.as_str()),
                                        copy_label,
                                    )
                                    .flex_shrink_0()
                                    .on_click(cx.listener(
                                        move |app, _, window, cx| {
                                            app.copied_game_url = Some(copied_url.clone());
                                            app.copy_to_clipboard(
                                                copied_url.clone(),
                                                "app.game_url_copied",
                                                window,
                                                cx,
                                            );
                                        },
                                    )),
                                ),
                        ),
                );
            }
            let mut urls = div()
                .flex()
                .flex_col()
                .min_w_0()
                .gap_2()
                .px_3()
                .pb_3()
                .text_xs()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .justify_between()
                        .gap_1()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(div().id("package-part-count").test_support().child(format!(
                            "{} ({})",
                            self.language.text("app.package_part_urls"),
                            listed_urls.len()
                        )))
                        .child(
                            copy_button(
                                "copy-all-game-urls",
                                self.copy_feedback == Some("app.all_urls_copied"),
                                self.language.text("app.copy_all_urls"),
                            )
                            .disabled(listed_urls.is_empty())
                            .on_click(cx.listener(
                                move |app, _, window, cx| {
                                    app.copy_to_clipboard(
                                        all_urls.clone(),
                                        "app.all_urls_copied",
                                        window,
                                        cx,
                                    );
                                },
                            )),
                        ),
                )
                .child(if self.part_discovery.is_some() {
                    self.language
                        .text("app.finding_package_parts")
                        .replace("%{count}", &listed_urls.len().to_string())
                } else if let Some(Ok(count)) = self.part_discovery_result.as_ref() {
                    format!("{}: {count}", self.language.text("app.total_parts"))
                } else if let Some(Err(error)) = self.part_discovery_result.as_ref() {
                    format!(
                        "{}: {error}",
                        self.language.text("app.part_search_incomplete")
                    )
                } else {
                    self.language.text("app.only_observed_parts")
                })
                .when(
                    self.part_discovery_result
                        .as_ref()
                        .is_some_and(Result::is_err),
                    |view| {
                        view.child(
                            Button::new("retry-part-search")
                                .outline()
                                .xsmall()
                                .label(self.language.text("app.retry"))
                                .on_click(
                                    cx.listener(|app, _, _, cx| app.retry_part_discovery(cx)),
                                ),
                        )
                    },
                )
                .child(url_list);
            if self.game_urls.len() == MAX_GAME_URLS {
                urls = urls.child(self.language.text("app.url_list_limit"));
            }
            url_panel = url_panel.content(urls);
        }
        body = body.child(url_panel);
        div()
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
                            .child(self.language.text("app.detected_package")),
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
                            .border_color(if !loading && self.metadata_error.is_none() {
                                rgb(0x35634A).into()
                            } else {
                                cx.theme().border
                            })
                            .when(!loading && self.metadata_error.is_none(), |badge| {
                                badge.bg(rgb(0x202D25))
                            })
                            .text_xs()
                            .text_color(if self.metadata_error.is_some() {
                                cx.theme().danger
                            } else if !loading {
                                cx.theme().success
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(
                                if self.metadata_error.is_some() {
                                    cx.theme().danger
                                } else if !loading {
                                    cx.theme().success
                                } else {
                                    cx.theme().muted_foreground
                                },
                            ))
                            .child(status),
                    ),
            )
            .child(body)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        Button::new("game-back-proxy")
                            .outline()
                            .label(self.language.text("app.back_to_proxy"))
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.current_step = 0;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("game-next-folder")
                            .primary()
                            .accessibility_label(self.language.text("app.continue_to_folder"))
                            .child(
                                div()
                                    .id("game-next-content")
                                    .test_support()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(
                                        div().id("game-next-icon").test_support().child(
                                            Icon::new(if rtl {
                                                IconName::ArrowLeft
                                            } else {
                                                IconName::ArrowRight
                                            })
                                            .with_size(px(14.)),
                                        ),
                                    )
                                    .child(
                                        div()
                                            .id("game-next-label")
                                            .test_support()
                                            .font_weight(FontWeight::BOLD)
                                            .child(self.language.text("app.continue_to_folder")),
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
