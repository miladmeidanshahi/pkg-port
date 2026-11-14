//! Step 1: proxy settings view.

use crate::ui::{copy_button, disclosure_chevron};
use crate::{valid_proxy_fields, App, GuideConsole, ProxyTransition};
use gpui_kit::base::{Disableable as _, StyledExt as _};
use gpui_kit::component::WindowExt;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    collapsible::Collapsible,
    dialog::{DialogDescription, DialogFooter, DialogHeader, DialogTitle},
    input::Input,
    select::Select,
    ActiveTheme as _, Icon, Sizable as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use std::net::Ipv4Addr;

const PS5_IMAGES: [&str; 9] = [
    "guides/ps5/1.webp",
    "guides/ps5/2.webp",
    "guides/ps5/3.webp",
    "guides/ps5/4.webp",
    "guides/ps5/5.webp",
    "guides/ps5/6.webp",
    "guides/ps5/7.webp",
    "guides/ps5/8.webp",
    "guides/ps5/9.webp",
];
const PS5_CAPTIONS: [&str; 9] = [
    "app.ps5_guide_one",
    "app.ps5_guide_two",
    "app.ps5_guide_three",
    "app.ps5_guide_four",
    "app.ps5_guide_five",
    "app.ps5_guide_six",
    "app.ps5_guide_seven",
    "app.ps5_guide_eight",
    "app.ps5_guide_nine",
];
const PS4_IMAGES: [&str; 12] = [
    "guides/ps4/6.jpg",
    "guides/ps4/7.jpg",
    "guides/ps4/8.jpg",
    "guides/ps4/9.jpg",
    "guides/ps4/11.jpg",
    "guides/ps4/10.jpg",
    "guides/ps4/12.jpg",
    "guides/ps4/13.jpg",
    "guides/ps4/14.jpg",
    "guides/ps4/15.jpg",
    "guides/ps4/16.jpg",
    "guides/ps4/17.jpg",
];
const PS4_CAPTIONS: [&str; 12] = [
    "app.ps4_guide_one",
    "app.ps4_guide_two",
    "app.ps4_guide_three",
    "app.ps4_guide_four",
    "app.ps4_guide_five",
    "app.ps4_guide_six",
    "app.ps4_guide_seven",
    "app.ps4_guide_eight",
    "app.ps4_guide_nine",
    "app.ps4_guide_ten",
    "app.ps4_guide_eleven",
    "app.ps4_guide_twelve",
];

impl App {
    pub(crate) fn proxy_view(&self, cx: &mut Context<Self>, narrow: bool) -> AnyElement {
        let language = self.language;
        let rtl = language.is_rtl();
        let running = self.proxy.is_some();
        let transitioning = self.proxy_transition.is_some();
        let bind_value = self.bind_ip.read(cx).value().to_string();
        let port_value = self.bind_port.read(cx).value().to_string();
        let valid_port = port_value.parse::<u16>().is_ok_and(|port| port > 0);
        let address = self.device_address(cx);
        let address_text = address
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "—".into());
        let shown_port = self
            .proxy
            .as_ref()
            .map(|proxy| proxy.address.port().to_string())
            .unwrap_or_else(|| port_value.clone());
        let fixed_bind = bind_value
            .parse::<Ipv4Addr>()
            .ok()
            .filter(|ip| !ip.is_unspecified());
        let interface_menu = Select::new(&self.interface_select)
            .id("network-interface")
            .w_full()
            .h(px(36.))
            .placeholder(language.text("app.select_interface"))
            .accessibility_label(language.text("app.network_interface"))
            .disabled(self.interfaces.is_empty() || fixed_bind.is_some());
        let copied_address = address_text.clone();
        let copied_port = shown_port.clone();
        let action = if let Some(transition) = &self.proxy_transition {
            let label = language.text(match transition {
                ProxyTransition::Starting(_) => "app.starting",
                ProxyTransition::Stopping(_) => "app.stopping",
            });
            Button::new("proxy-transition")
                .primary()
                .disabled(true)
                .accessibility_label(label.clone())
                .child(div().font_weight(FontWeight::BOLD).child(label))
                .into_any_element()
        } else if running {
            let label = language.text("app.stop_proxy");
            Button::new("stop")
                .outline()
                .bg(rgb(0x362321))
                .border_color(rgb(0x6C3632))
                .text_color(rgb(0xFF6B5F))
                .accessibility_label(label.clone())
                .child(div().font_weight(FontWeight::BOLD).child(label))
                .on_click(cx.listener(|app, _, _, cx| app.stop(cx)))
                .into_any_element()
        } else {
            let label = language.text("app.start_proxy");
            Button::new("start")
                .primary()
                .accessibility_label(label.clone())
                .child(div().font_weight(FontWeight::BOLD).child(label))
                .disabled(!valid_proxy_fields(&bind_value, &port_value) || address.is_none())
                .on_click(cx.listener(|app, _, _, cx| app.start(cx)))
                .into_any_element()
        };
        let status_key = if let Some(transition) = &self.proxy_transition {
            match transition {
                ProxyTransition::Starting(_) => "app.starting",
                ProxyTransition::Stopping(_) => "app.stopping",
            }
        } else if running {
            "app.running"
        } else if self.proxy_error.is_some() {
            "app.failed"
        } else {
            "app.stopped"
        };
        let state_color = if running {
            cx.theme().success
        } else if transitioning {
            rgb(0xF2BD83).into()
        } else if self.proxy_error.is_some() {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        };
        let device_card = div()
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
                            .child(language.text("app.device_setup")),
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
                            .border_color(if running {
                                cx.theme().success
                            } else {
                                cx.theme().border
                            })
                            .bg(if running {
                                rgb(0x202D25).into()
                            } else {
                                cx.theme().group_box
                            })
                            .text_color(state_color)
                            .text_xs()
                            .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(state_color))
                            .child(language.text(status_key)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text("app.network_interface")),
                    )
                    .child(interface_menu)
                    .when(self.interfaces.is_empty(), |view| {
                        view.child(
                            div()
                                .text_color(cx.theme().danger)
                                .child(language.text("app.no_interfaces")),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_4()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .min_w_0()
                            .flex_1()
                            .p_2()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_md()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text("app.proxy_address")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .min_w_0()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(
                                        div()
                                            .text_left()
                                            .min_w_0()
                                            .font_semibold()
                                            .child(address_text.clone()),
                                    )
                                    .child(
                                        copy_button(
                                            "copy-device-address",
                                            self.copy_feedback == Some("app.address_copied"),
                                            language.text("app.copy_address"),
                                        )
                                        .disabled(address.is_none())
                                        .on_click(
                                            cx.listener(move |app, _, window, cx| {
                                                app.copy_to_clipboard(
                                                    copied_address.clone(),
                                                    "app.address_copied",
                                                    window,
                                                    cx,
                                                );
                                            }),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_2()
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .rounded_md()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(language.text("app.port")),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(
                                        div().text_left().font_semibold().child(shown_port.clone()),
                                    )
                                    .child(
                                        copy_button(
                                            "copy-device-port",
                                            self.copy_feedback == Some("app.port_copied"),
                                            language.text("app.copy_port"),
                                        )
                                        .disabled(!valid_port)
                                        .on_click(
                                            cx.listener(move |app, _, window, cx| {
                                                app.copy_to_clipboard(
                                                    copied_port.clone(),
                                                    "app.port_copied",
                                                    window,
                                                    cx,
                                                );
                                            }),
                                        ),
                                    ),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(action)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .whitespace_normal()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text(if running {
                                "app.proxy_running_caption"
                            } else {
                                "app.proxy_stopped_caption"
                            })),
                    ),
            )
            .when_some(self.proxy_error.as_ref(), |view, error| {
                view.child(div().text_color(cx.theme().danger).child(error.clone()))
            })
            .when(address.is_none(), |view| {
                view.child(
                    div()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text("app.no_device_address")),
                )
            });
        let guide_step = |number: usize, key| {
            div()
                .flex()
                .items_start()
                .gap_2()
                .when(rtl, |row| row.flex_row_reverse())
                .child(
                    div()
                        .w(px(20.))
                        .h(px(20.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .rounded_md()
                        .bg(cx.theme().border)
                        .text_xs()
                        .child(if rtl {
                            ["۱", "۲", "۳"][number - 1]
                        } else {
                            ["1", "2", "3"][number - 1]
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .whitespace_normal()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(language.text(key)),
                )
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
                            .id("proxy-guide-icon")
                            .test_support()
                            .child(Icon::new(assets::IconName::Monitor)),
                    )
                    .child(
                        div()
                            .id("proxy-guide-title")
                            .test_support()
                            .font_semibold()
                            .child(language.text("app.on_your_device")),
                    ),
            )
            .child(guide_step(1, "app.device_guide_one"))
            .child(guide_step(2, "app.device_guide_two"))
            .child(guide_step(3, "app.device_guide_three"))
            .child(
                Button::new("ps4-guide-toggle")
                    .outline()
                    .accessibility_label(language.text("app.ps4_guide"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(rtl, |row| row.flex_row_reverse())
                            .child(Icon::new(assets::IconName::MessageCircleQuestionMark))
                            .child(language.text("app.ps4_guide")),
                    )
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.open_console_guide(window, cx, GuideConsole::Ps4);
                    })),
            )
            .child(
                Button::new("ps5-guide-toggle")
                    .outline()
                    .accessibility_label(language.text("app.ps5_guide"))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(rtl, |row| row.flex_row_reverse())
                            .child(Icon::new(assets::IconName::MessageCircleQuestionMark))
                            .child(language.text("app.ps5_guide")),
                    )
                    .on_click(cx.listener(|app, _, window, cx| {
                        app.open_console_guide(window, cx, GuideConsole::Ps5);
                    })),
            )
            .child(
                div()
                    .id("proxy-guide-note-row")
                    .test_support()
                    .flex()
                    .items_start()
                    .gap_2()
                    .pt_3()
                    .w_full()
                    .min_w_0()
                    .when(rtl, |row| row.flex_row_reverse())
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        div()
                            .id("proxy-guide-note-icon")
                            .test_support()
                            .flex_shrink_0()
                            .child(
                                Icon::new(assets::IconName::Shield)
                                    .with_size(px(14.))
                                    .mt(px(2.))
                                    .flex_shrink_0(),
                            ),
                    )
                    .child(
                        div()
                            .id("proxy-guide-note-text")
                            .test_support()
                            .min_w_0()
                            .flex_1()
                            .whitespace_normal()
                            .child(
                                language
                                    .text("app.any_device_on_those_subnets_may_use_the_proxy_wh"),
                            ),
                    ),
            );
        let bind_select = Select::new(&self.bind_select)
            .id("bind-address")
            .w_full()
            .h(px(32.))
            .accessibility_label(language.text("app.bind_ipv4"))
            .disabled(running || transitioning);
        let advanced = Collapsible::new()
            .open(self.advanced_open)
            .child(
                div()
                    .flex()
                    .justify_start()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        Button::new("toggle-advanced")
                            .ghost()
                            .w_full()
                            .px_3()
                            .text_color(cx.theme().foreground)
                            .accessibility_label(language.text("app.advanced_settings"))
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap_2()
                                    .when(rtl, |row| row.flex_row_reverse())
                                    .child(div().child(language.text("app.advanced_settings")))
                                    .child(Icon::new(disclosure_chevron(self.advanced_open, rtl))),
                            )
                            .on_click(cx.listener(|app, _, _, cx| {
                                app.advanced_open = !app.advanced_open;
                                cx.notify();
                            })),
                    ),
            )
            .content(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .px_3()
                    .pb_3()
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_3()
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
                                            .child(language.text("app.bind_ipv4")),
                                    )
                                    .child(bind_select),
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
                                            .child(language.text("app.listening_port")),
                                    )
                                    .child(
                                        Input::new(&self.bind_port)
                                            .w_full()
                                            .h(px(32.))
                                            .disabled(running || transitioning)
                                            .text_left(),
                                    ),
                            ),
                    )
                    .when(!valid_port, |view| {
                        view.child(
                            div()
                                .text_color(cx.theme().danger)
                                .child(language.text("app.invalid_port_inline")),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text("app.all_interfaces_help")),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} · {}",
                                language.text("app.detected_allowed_ipv4_subnets"),
                                if self.subnets.is_empty() {
                                    language.text("app.none")
                                } else {
                                    self.subnets.join(", ")
                                }
                            )),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text("app.https_remains_encrypted")),
                    ),
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
                            .child(language.text("app.proxy_setup")),
                    )
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .whitespace_normal()
                            .text_color(cx.theme().muted_foreground)
                            .child(language.text("app.proxy_intro")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_4()
                    .when(narrow, |row| row.flex_col())
                    .child(div().flex_1().min_w_0().child(device_card))
                    .child(
                        div()
                            .when(narrow, |view| view.w_full())
                            .when(!narrow, |view| view.w(px(280.)))
                            .child(guide),
                    ),
            )
            .child(
                div()
                    .bg(cx.theme().group_box)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .child(advanced),
            )
            .into_any_element()
    }

    fn open_console_guide(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        console: GuideConsole,
    ) {
        self.console_guide = Some((console, 0));
        cx.notify();
        let address = self
            .device_address(cx)
            .map_or_else(|| "—".to_owned(), |ip| ip.to_string());
        let port = self
            .proxy
            .as_ref()
            .map(|proxy| proxy.address.port().to_string())
            .unwrap_or_else(|| self.bind_port.read(cx).value().to_string());
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            let state = app.read(cx);
            let (console, step) = state.console_guide.unwrap_or((console, 0));
            let (images, captions, title, warning_step) = match console {
                GuideConsole::Ps4 => (
                    &PS4_IMAGES[..],
                    &PS4_CAPTIONS[..],
                    "app.ps4_setup_guide",
                    10,
                ),
                GuideConsole::Ps5 => (&PS5_IMAGES[..], &PS5_CAPTIONS[..], "app.ps5_setup_guide", 6),
            };
            let language = state.language;
            let narrow = window.viewport_size().width < px(640.);
            let previous_app = app.clone();
            let next_app = app.clone();
            let rtl = language.is_rtl();
            dialog
                .overlay(true)
                .when(rtl, |dialog| dialog.text_right())
                .close_button(!rtl)
                .w(px(760.))
                .title(
                    DialogHeader::new().child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .when(rtl, |title| title.flex_row_reverse())
                            .child(div().id("console-guide-title").test_support().child(
                                DialogTitle::new().child(format!(
                                    "{} · {}/{}",
                                    language.text(title),
                                    step + 1,
                                    images.len()
                                )),
                            ))
                            .when(rtl, |title| {
                                title.child(
                                    Button::new("console-guide-close")
                                        .small()
                                        .ghost()
                                        .icon(assets::IconName::Close)
                                        .accessibility_label("Close")
                                        .on_click(|_, window, cx| window.close_dialog(cx)),
                                )
                            }),
                    ),
                )
                .child(
                    div()
                        .id("console-guide-description")
                        .test_support()
                        .child(DialogDescription::new().child(language.text(captions[step]))),
                )
                .when(step == warning_step, |dialog| {
                    dialog.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .p_3()
                            .bg(cx.theme().group_box)
                            .border_1()
                            .border_color(cx.theme().primary)
                            .child(language.text("app.ps5_ignore_sample_values"))
                            .child(format!("{}: {address}", language.text("app.proxy_address")))
                            .child(format!("{}: {port}", language.text("app.port"))),
                    )
                })
                .child(
                    img(images[step])
                        .mt_2()
                        .object_fit(ObjectFit::Contain)
                        .id("console-guide-image")
                        .test_support()
                        .w_full()
                        .h(if narrow {
                            rems(15.)
                        } else if console == GuideConsole::Ps4 {
                            rems(22.)
                        } else {
                            rems(31.25)
                        }),
                )
                .footer(
                    DialogFooter::new()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(
                            Button::new("console-guide-previous")
                                .outline()
                                .label(language.text("app.previous"))
                                .disabled(step == 0)
                                .on_click(move |_, _, cx| {
                                    previous_app.update(cx, |app, cx| {
                                        app.console_guide = Some((console, step.saturating_sub(1)));
                                        cx.notify();
                                    });
                                }),
                        )
                        .child(
                            Button::new("console-guide-next")
                                .primary()
                                .label(language.text("app.next"))
                                .disabled(step + 1 == images.len())
                                .on_click(move |_, _, cx| {
                                    next_app.update(cx, |app, cx| {
                                        let last_step = match console {
                                            GuideConsole::Ps4 => PS4_IMAGES.len() - 1,
                                            GuideConsole::Ps5 => PS5_IMAGES.len() - 1,
                                        };
                                        app.console_guide =
                                            Some((console, (step + 1).min(last_step)));
                                        cx.notify();
                                    });
                                }),
                        ),
                )
        });
    }
}
