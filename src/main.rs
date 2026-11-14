#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod folder;
mod i18n;
mod metadata;
mod parts;
mod proxy;
mod settings;
mod theme;
#[cfg(target_os = "linux")]
mod tray;
mod ui;
mod updates;

rust_i18n::i18n!("locales", fallback = "en");

use gpui_kit::component as gpui_component;
use gpui_kit::component::{
    button::{Button, ButtonVariants as _},
    dialog::{DialogDescription, DialogFooter, DialogHeader, DialogTitle},
    input::InputState,
    link::Link,
    menu::DropdownMenu as _,
    message_scroller::{MessageScroller, MessageScrollerState},
    notification::Notification,
    searchable_list::{SearchableListItem, SearchableVec},
    select::{SelectEvent, SelectState},
    status_bar::StatusBar,
    ActiveTheme as _, Icon, IconName, Side, Sizable as _, TitleBar, WindowExt as _,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use i18n::Language;
use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use ui::AppAssets;

struct CopyNotification;

const MAX_GAME_URLS: usize = 4096;
const MAX_GAME_URL_LENGTH: usize = 2048;
/// Rolling window the upload speed is measured over.
const UPLOAD_WINDOW: Duration = Duration::from_millis(1200);
const UPLOAD_STATUS_GRACE: Duration = Duration::from_secs(2);

fn upload_status_until(
    previous: Option<Instant>,
    active: bool,
    bytes_increased: bool,
    now: Instant,
) -> Option<Instant> {
    if active || bytes_increased {
        Some(now + UPLOAD_STATUS_GRACE)
    } else {
        previous.filter(|until| *until > now)
    }
}

actions!(language, [SelectEnglish, SelectPersian]);

#[cfg(any(test, debug_assertions))]
const TEST_GAME_URL: &str = "http://gst.prod.dl.playstation.net/gst/prod/00/PPSA01547_00/app/pkg/13/f_d918060c4bd9613203567db42557b59e39928edd7c0aa20a129336913acd7ca8/EP1004-PPSA01547_00-GRANDTHEFTAUTO00_0.pkg";

fn main() {
    application().with_assets(AppAssets).run(|cx| {
        rust_i18n::extend!(gpui_component);
        init(cx);
        cx.text_system()
            .add_fonts(vec![Cow::Borrowed(
                include_bytes!("../assets/fonts/InterVariable.ttf").as_slice(),
            )])
            .expect("Failed to load bundled Inter font");
        gpui_kit::component::set_locale("en");
        theme::apply(cx);
        let http_client = cx.http_client();
        #[cfg(target_os = "linux")]
        let (tray_stop, tray_events) = tray::start();
        // X11 only; on Wayland the icon comes from the desktop entry.
        let icon = image::load_from_memory(include_bytes!("../assets/icon-128.png"))
            .ok()
            .map(|icon| Arc::new(icon.to_rgba8()));
        let options = WindowOptions {
            #[cfg(target_os = "linux")]
            window_decorations: Some(WindowDecorations::Client),
            #[cfg(target_os = "linux")]
            app_id: Some("pkgport".into()),
            icon,
            ..TitleBar::window_options()
        };
        open_window(options, cx, |window, cx| {
            window.set_window_title(&format!("PKGPort v{}", env!("CARGO_PKG_VERSION")));
            cx.new(|cx| {
                App::new(
                    window,
                    cx,
                    http_client.clone(),
                    #[cfg(target_os = "linux")]
                    tray_stop,
                    #[cfg(target_os = "linux")]
                    tray_events,
                )
            })
        })
        .expect("Failed to open window");
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuideConsole {
    Ps4,
    Ps5,
}

struct App {
    bind_ip: Entity<InputState>,
    bind_select: Entity<SelectState<SearchableVec<InterfaceOption>>>,
    _bind_subscription: Subscription,
    bind_port: Entity<InputState>,
    package_dir: Option<PathBuf>,
    folder_check: Option<Receiver<Result<folder::Inspection, String>>>,
    folder_inspection: Option<Result<folder::Inspection, String>>,
    interfaces: Vec<LanInterface>,
    selected_interface: Option<Ipv4Addr>,
    interface_select: Entity<SelectState<SearchableVec<InterfaceOption>>>,
    _interface_subscription: Subscription,
    advanced_open: bool,
    copy_feedback: Option<&'static str>,
    copy_feedback_until: Option<Instant>,
    copied_game_url: Option<String>,
    settings_error: Option<String>,
    proxy_error: Option<String>,
    proxy_transition: Option<ProxyTransition>,
    current_step: usize,
    console_guide: Option<(GuideConsole, usize)>,
    available_update: Option<updates::AvailableUpdate>,
    language: Language,
    persian_fonts_loaded: bool,
    show_game_urls: bool,
    game_urls: Vec<String>,
    part_discovery: Option<Receiver<parts::DiscoveryEvent>>,
    part_discovery_result: Option<Result<usize, String>>,
    subnets: Vec<String>,
    proxy: Option<proxy::Proxy>,
    stopping_events: Option<Receiver<String>>,
    message: String,
    log: VecDeque<String>,
    log_scroller: Entity<MessageScrollerState>,
    _log_subscription: Subscription,
    _input_subscriptions: Vec<Subscription>,
    game: Option<GamePackage>,
    transfers: proxy::TransferState,
    package_scroll: ScrollHandle,
    dev_mock: bool,
    upload_samples: VecDeque<(Instant, u64)>,
    upload_rate: f32,
    upload_status_until: Option<Instant>,
    metadata: Option<Receiver<Result<metadata::GameMetadata, String>>>,
    metadata_error: Option<String>,
    cover: Option<Arc<Image>>,
    _poll_task: Task<()>,
    _update_check_task: Task<()>,
    #[cfg(target_os = "linux")]
    _tray_stop: mpsc::Sender<()>,
    #[cfg(target_os = "linux")]
    tray_events: Receiver<()>,
    #[cfg(target_os = "linux")]
    activate_pending: bool,
}

struct GamePackage {
    name: Option<String>,
    device: String,
    title_id: String,
    region: Option<&'static str>,
    url: String,
}

#[derive(Clone)]
struct LanInterface {
    name: String,
    ip: Ipv4Addr,
}

#[derive(Clone)]
struct InterfaceOption {
    label: String,
    ip: Ipv4Addr,
}

impl SearchableListItem for InterfaceOption {
    type Value = Ipv4Addr;

    fn title(&self) -> SharedString {
        self.label.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.ip
    }
}

fn bind_options(interfaces: &[LanInterface], language: Language) -> SearchableVec<InterfaceOption> {
    let mut options = vec![InterfaceOption {
        label: language.text("app.all_interfaces"),
        ip: Ipv4Addr::UNSPECIFIED,
    }];
    options.extend(interfaces.iter().map(|interface| InterfaceOption {
        label: interface.ip.to_string(),
        ip: interface.ip,
    }));
    SearchableVec::new(options)
}

fn interface_options(
    interfaces: &[LanInterface],
    language: Language,
) -> SearchableVec<InterfaceOption> {
    let suggested = suggested_interface(interfaces);
    SearchableVec::new(
        interfaces
            .iter()
            .map(|interface| InterfaceOption {
                label: format!(
                    "{} · {}{}",
                    interface.name,
                    interface.ip,
                    if Some(interface.ip) == suggested {
                        format!(" ({})", language.text("app.suggested"))
                    } else {
                        String::new()
                    }
                ),
                ip: interface.ip,
            })
            .collect::<Vec<_>>(),
    )
}

enum ProxyTransition {
    Starting(Receiver<Result<proxy::Proxy, String>>),
    Stopping(Receiver<()>),
}

impl GamePackage {
    fn from_log(entry: &str) -> Option<Self> {
        let (device, request) = entry.split_once(' ')?;
        let (_, target) = request.split_once(' ')?;
        let url = url::Url::parse(target).ok()?;
        if url.scheme() != "http" || url.host_str()? != "gst.prod.dl.playstation.net" {
            return None;
        }
        let parts: Vec<_> = url.path_segments()?.collect();
        if parts.len() != 9
            || parts[0] != "gst"
            || parts[1] != "prod"
            || parts[4] != "app"
            || parts[5] != "pkg"
        {
            return None;
        }
        let title_id = parts[3].split_once('_')?.0;
        let filename = parts[8];
        let content_id = filename.strip_suffix(".pkg")?;
        if title_id.is_empty() || !content_id.contains(parts[3]) {
            return None;
        }
        let region = package_region(content_id);
        Some(Self {
            name: None,
            device: device.into(),
            title_id: title_id.into(),
            region,
            url: target.into(),
        })
    }
}

fn package_region(content_id: &str) -> Option<&'static str> {
    let (publisher, _) = content_id.split_once('-')?;
    if publisher.len() != 6 || !publisher.as_bytes()[2..].iter().all(u8::is_ascii_digit) {
        return None;
    }
    match publisher.get(..2)? {
        "EP" => Some("app.region_europe"),
        "UP" => Some("app.region_north_america"),
        "JP" => Some("app.region_japan"),
        "HP" => Some("app.region_asia"),
        "AP" => Some("app.region_china"),
        "KP" => Some("app.region_korea"),
        _ => None,
    }
}

fn record_game_url(urls: &mut Vec<String>, url: &str) {
    if url.len() <= MAX_GAME_URL_LENGTH
        && urls.len() < MAX_GAME_URLS
        && !urls.iter().any(|existing| existing == url)
    {
        urls.push(url.to_owned());
    }
}

impl App {
    fn load_persian_fonts(cx: &mut Context<Self>) -> bool {
        match cx.text_system().add_fonts(vec![
            include_bytes!("../assets/fonts/Vazirmatn-Regular.ttf")
                .as_slice()
                .into(),
            include_bytes!("../assets/fonts/Vazirmatn-Bold.ttf")
                .as_slice()
                .into(),
        ]) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("Persian font unavailable: {error}");
                false
            }
        }
    }

    fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        http_client: Arc<dyn gpui_kit::http_client::HttpClient>,
        #[cfg(target_os = "linux")] tray_stop: mpsc::Sender<()>,
        #[cfg(target_os = "linux")] tray_events: Receiver<()>,
    ) -> Self {
        let saved = settings::load();
        let language = if saved.language == "fa" {
            Language::Persian
        } else {
            Language::English
        };
        let persian_fonts_loaded = language.is_rtl() && Self::load_persian_fonts(cx);
        gpui_kit::component::set_locale(if language.is_rtl() { "fa" } else { "en" });
        gpui_kit::component::Theme::update(cx, |theme| {
            theme.font_family = if language.is_rtl() {
                "Vazirmatn".into()
            } else {
                "Inter Variable".into()
            };
        });
        let fixed_bind_ip = saved
            .bind_ip
            .parse::<Ipv4Addr>()
            .ok()
            .filter(|ip| !ip.is_unspecified());
        let interfaces = detected_interfaces();
        let fixed_bind_ip =
            fixed_bind_ip.filter(|ip| interfaces.iter().any(|interface| interface.ip == *ip));
        let initial_bind_ip = fixed_bind_ip
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "0.0.0.0".into());
        let bind_ip = cx.new(|cx| InputState::new(window, cx).default_value(initial_bind_ip));
        let bind_port = cx.new(|cx| InputState::new(window, cx).default_value(saved.port));
        let package_dir = saved.package_dir;
        let selected_interface = fixed_bind_ip
            .filter(|selected| interfaces.iter().any(|interface| interface.ip == *selected))
            .or(saved.selected_interface)
            .filter(|selected| interfaces.iter().any(|interface| interface.ip == *selected))
            .or_else(|| suggested_interface(&interfaces));
        let selected_index = selected_interface.and_then(|ip| {
            interfaces
                .iter()
                .position(|interface| interface.ip == ip)
                .map(gpui_component::IndexPath::new)
        });
        let interface_select = cx.new(|cx| {
            SelectState::new(
                interface_options(&interfaces, language),
                selected_index,
                window,
                cx,
            )
        });
        let bind_index = fixed_bind_ip
            .and_then(|ip| interfaces.iter().position(|interface| interface.ip == ip))
            .map_or(0, |index| index + 1);
        let bind_select = cx.new(|cx| {
            SelectState::new(
                bind_options(&interfaces, language),
                Some(gpui_component::IndexPath::new(bind_index)),
                window,
                cx,
            )
        });
        let bind_subscription = cx.subscribe_in(
            &bind_select,
            window,
            |app, _, event: &SelectEvent<SearchableVec<InterfaceOption>>, window, cx| {
                let SelectEvent::Confirm(Some(ip)) = event else {
                    return;
                };
                let ip = *ip;
                app.bind_ip
                    .update(cx, |input, cx| input.set_value(ip.to_string(), window, cx));
                if app.interfaces.iter().any(|interface| interface.ip == ip) {
                    app.selected_interface = Some(ip);
                    app.interface_select
                        .update(cx, |select, cx| select.set_selected_value(&ip, window, cx));
                }
                app.persist_settings(cx);
                cx.notify();
            },
        );
        let interface_subscription = cx.subscribe(
            &interface_select,
            |app, _, event: &SelectEvent<SearchableVec<InterfaceOption>>, cx| {
                let SelectEvent::Confirm(Some(ip)) = event else {
                    return;
                };
                app.selected_interface = Some(*ip);
                app.copy_feedback = None;
                app.persist_settings(cx);
                cx.notify();
            },
        );
        let folder_check = package_dir.clone().map(folder::inspect);
        let subnets = detected_subnets();
        let log_scroller = cx.new(|cx| MessageScrollerState::new(0, cx));
        let log_subscription = cx.observe(&log_scroller, |_, _, cx| cx.notify());
        let input_subscriptions = vec![
            cx.observe(&bind_ip, |app, _, cx| {
                app.persist_settings(cx);
                cx.notify();
            }),
            cx.observe(&bind_port, |app, _, cx| {
                app.persist_settings(cx);
                cx.notify();
            }),
        ];
        let poll_task = cx.spawn(async move |view, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            if view.update(cx, |app, cx| app.poll(cx)).is_err() {
                break;
            }
        });
        let update_check_task = cx.spawn(async move |view, cx| {
            if let Ok(Some(update)) = updates::check_latest(http_client).await {
                let _ = view.update(cx, |app, cx| {
                    app.available_update = Some(update);
                    cx.notify();
                });
            }
        });
        let app = Self {
            bind_ip,
            bind_select,
            _bind_subscription: bind_subscription,
            bind_port,
            package_dir,
            folder_check,
            folder_inspection: None,
            interfaces,
            selected_interface,
            interface_select,
            _interface_subscription: interface_subscription,
            advanced_open: false,
            copy_feedback: None,
            copy_feedback_until: None,
            copied_game_url: None,
            settings_error: None,
            proxy_error: None,
            proxy_transition: None,
            current_step: 0,
            console_guide: None,
            available_update: None,
            language,
            persian_fonts_loaded,
            show_game_urls: false,
            game_urls: Vec::new(),
            part_discovery: None,
            part_discovery_result: None,
            subnets,
            proxy: None,
            stopping_events: None,
            message: String::new(),
            log: VecDeque::new(),
            log_scroller,
            _log_subscription: log_subscription,
            _input_subscriptions: input_subscriptions,
            game: None,
            transfers: proxy::TransferState::default(),
            package_scroll: ScrollHandle::new(),
            dev_mock: false,
            upload_samples: VecDeque::new(),
            upload_rate: 0.0,
            upload_status_until: None,
            metadata: None,
            metadata_error: None,
            cover: None,
            _poll_task: poll_task,
            _update_check_task: update_check_task,
            #[cfg(target_os = "linux")]
            _tray_stop: tray_stop,
            #[cfg(target_os = "linux")]
            tray_events,
            #[cfg(target_os = "linux")]
            activate_pending: false,
        };
        #[cfg(debug_assertions)]
        let mut app = app;
        #[cfg(debug_assertions)]
        if std::env::var("PKGPORT_DEV_MOCK")
            .or_else(|_| std::env::var("PS_DOWNLOWDER_DEV_MOCK"))
            .as_deref()
            == Ok("1")
        {
            app.apply_dev_mock();
        }
        app
    }

    #[cfg(any(debug_assertions, test))]
    fn apply_dev_mock(&mut self) {
        let path = PathBuf::from("/tmp/pkgport-preview");
        let first_url = TEST_GAME_URL.replace("GRANDTHEFTAUTO00", "GTAVISTANDARD001");
        let first = first_url.rsplit('/').next().unwrap().to_owned();
        let second_url = first_url.replace("_0.pkg", "_1.pkg");
        let second = second_url.rsplit('/').next().unwrap().to_owned();
        self.dev_mock = true;
        self.folder_check = None;
        self.package_dir = Some(path.clone());
        self.folder_inspection = Some(Ok(folder::Inspection {
            path,
            files: HashMap::from([
                (first.clone(), 2_000_000_000),
                (second.clone(), 1_000_000_000),
            ]),
        }));
        self.game = Some(GamePackage {
            name: Some("Grand Theft Auto VI".into()),
            device: "192.168.1.50:54321".into(),
            title_id: "PPSA01547".into(),
            region: Some("app.region_europe"),
            url: first_url.clone(),
        });
        self.game_urls = vec![first_url, second_url];
        self.part_discovery_result = Some(Ok(2));
        self.show_game_urls = true;
        self.transfers = proxy::TransferState {
            bytes: 2_500_000_000,
            last_url: Some(self.game_urls[1].clone()),
            files: HashMap::from([
                (
                    first,
                    proxy::FileTransfer {
                        size: 2_000_000_000,
                        served: 2_000_000_000,
                        complete: true,
                        interrupted: false,
                    },
                ),
                (
                    second.clone(),
                    proxy::FileTransfer {
                        size: 1_000_000_000,
                        served: 500_000_000,
                        complete: false,
                        interrupted: false,
                    },
                ),
            ]),
            active: vec![proxy::ActiveTransfer {
                id: 1,
                peer: SocketAddr::from(([192, 168, 1, 50], 54321)),
                url: self.game_urls[1].clone(),
                file: second,
                sent: 500_000_000,
                total: 1_000_000_000,
            }],
        };
        self.upload_rate = 12.5;
        if std::env::var("PKGPORT_DEV_MOCK_UPDATE").as_deref() == Ok("1") {
            let mut version =
                semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("valid package version");
            version.patch += 1;
            self.available_update = Some(updates::AvailableUpdate {
                version,
                download_url: "https://github.com/miladmeidanshahi/pkg-port/releases".into(),
            });
        }
    }
}

/// Parts of the detected package that exist in the chosen folder, sorted by index.
fn package_parts_from(
    files: &HashMap<String, u64>,
    urls: &[String],
    required: Option<usize>,
) -> Vec<(String, u64)> {
    let Some(prefix) = urls.iter().find_map(|url| {
        parts::part_index(url)?;
        url.rsplit('/')
            .next()?
            .rsplit_once('_')
            .map(|(prefix, _)| prefix)
    }) else {
        return Vec::new();
    };
    let mut parts = files
        .iter()
        .filter_map(|(name, size)| {
            let index = name
                .strip_prefix(prefix)?
                .strip_prefix('_')?
                .strip_suffix(".pkg")?
                .parse::<usize>()
                .ok()?;
            Some((index, (name.clone(), *size)))
        })
        .collect::<Vec<_>>();
    // shortcut: without a verified part count every matching file counts, so
    // leftover higher-index parts inflate the total. Upgrade when discovery
    // succeeds offline.
    if let Some(required) = required {
        parts.retain(|(index, _)| *index < required);
    }
    parts.sort_by_key(|(index, _)| *index);
    parts.into_iter().map(|(_, part)| part).collect()
}

fn valid_proxy_fields(ip: &str, port: &str) -> bool {
    ip.parse::<Ipv4Addr>().is_ok() && port.parse::<u16>().is_ok_and(|value| value > 0)
}

fn detected_interfaces() -> Vec<LanInterface> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| {
            let if_addrs::IfAddr::V4(v4) = interface.addr else {
                return None;
            };
            if !v4.ip.is_private()
                || !matches!(
                    interface.oper_status,
                    if_addrs::IfOperStatus::Up | if_addrs::IfOperStatus::Unknown
                )
            {
                return None;
            }
            let mask = u32::from(v4.netmask);
            (mask >= u32::from(Ipv4Addr::new(255, 255, 0, 0))
                && (!mask) & ((!mask).wrapping_add(1)) == 0)
                .then_some(LanInterface {
                    name: interface.name,
                    ip: v4.ip,
                })
        })
        .collect()
}

fn detected_subnets() -> Vec<String> {
    if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|interface| {
            let if_addrs::IfAddr::V4(v4) = interface.addr else {
                return None;
            };
            if !v4.ip.is_private() {
                return None;
            }
            let mask = u32::from(v4.netmask);
            (mask >= u32::from(Ipv4Addr::new(255, 255, 0, 0))
                && (!mask) & ((!mask).wrapping_add(1)) == 0)
                .then(|| subnet_label(v4.ip, mask))
        })
        .collect()
}

fn subnet_label(ip: Ipv4Addr, mask: u32) -> String {
    format!(
        "{}/{}",
        Ipv4Addr::from(u32::from(ip) & mask),
        mask.count_ones()
    )
}

fn suggested_interface(interfaces: &[LanInterface]) -> Option<Ipv4Addr> {
    let physical = interfaces
        .iter()
        .filter(|interface| {
            let name = interface.name.to_ascii_lowercase();
            ![
                "docker", "veth", "virbr", "br-", "tun", "tap", "vmnet", "vbox",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
        })
        .collect::<Vec<_>>();
    if physical.len() == 1 {
        Some(physical[0].ip)
    } else if interfaces.len() == 1 {
        Some(interfaces[0].ip)
    } else {
        None
    }
}

impl App {
    fn copy_to_clipboard(
        &mut self,
        text: String,
        feedback: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let preview: String = text.chars().take(120).collect();
        let preview = if text.chars().count() > 120 {
            format!("{preview}…")
        } else {
            preview
        };
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        window.push_notification(
            Notification::success(preview)
                .id::<CopyNotification>()
                .title(self.language.text(feedback)),
            cx,
        );
        self.copy_feedback = Some(feedback);
        self.copy_feedback_until = Some(Instant::now() + Duration::from_secs(2));
        cx.notify();
    }

    fn persist_settings(&mut self, cx: &Context<Self>) {
        if self.dev_mock {
            return;
        }
        let current = settings::Settings {
            bind_ip: self.bind_ip.read(cx).value().to_string(),
            port: self.bind_port.read(cx).value().to_string(),
            selected_interface: self.selected_interface,
            package_dir: self.package_dir.clone(),
            language: if self.language.is_rtl() { "fa" } else { "en" }.into(),
        };
        self.settings_error = settings::save(&current)
            .err()
            .map(|error| error.to_string());
    }

    fn select_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == language {
            return;
        }
        self.language = language;
        if language.is_rtl() && !self.persian_fonts_loaded {
            self.persian_fonts_loaded = Self::load_persian_fonts(cx);
        }
        self.message.clear();
        gpui_kit::component::set_locale(if language.is_rtl() { "fa" } else { "en" });
        gpui_kit::component::Theme::update(cx, |theme| {
            theme.font_family = if language.is_rtl() {
                "Vazirmatn".into()
            } else {
                "Inter Variable".into()
            };
        });
        let options = bind_options(&self.interfaces, language);
        self.bind_select
            .update(cx, |select, cx| select.set_items(options, window, cx));
        let options = interface_options(&self.interfaces, language);
        self.interface_select
            .update(cx, |select, cx| select.set_items(options, window, cx));
        self.persist_settings(cx);
        cx.notify();
    }

    fn device_address(&self, cx: &Context<Self>) -> Option<Ipv4Addr> {
        let bind = if let Some(proxy) = &self.proxy {
            proxy.address.ip()
        } else {
            std::net::IpAddr::V4(self.bind_ip.read(cx).value().parse().ok()?)
        };
        let std::net::IpAddr::V4(bind) = bind else {
            return None;
        };
        let selected = if bind.is_unspecified() {
            self.selected_interface?
        } else {
            bind
        };
        self.interfaces
            .iter()
            .any(|interface| interface.ip == selected)
            .then_some(selected)
    }

    fn folder_ready(&self) -> bool {
        let (Some(Ok(inspection)), Some(Ok(count)), Some(first)) = (
            &self.folder_inspection,
            &self.part_discovery_result,
            self.game_urls
                .iter()
                .find(|url| parts::part_index(url) == Some(0)),
        ) else {
            return false;
        };
        *count > 0 && folder::matching_parts(first, *count, &inspection.files) == *count
    }

    fn metadata_lookup(title_id: String) -> Receiver<Result<metadata::GameMetadata, String>> {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(metadata::lookup(&title_id));
        });
        receiver
    }

    fn retry_metadata(&mut self, cx: &mut Context<Self>) {
        if self.metadata.is_some() || self.metadata_error.is_none() {
            return;
        }
        if let Some(game) = &self.game {
            self.metadata = Some(Self::metadata_lookup(game.title_id.clone()));
            self.metadata_error = None;
            cx.notify();
        }
    }

    fn retry_part_discovery(&mut self, cx: &mut Context<Self>) {
        if let Some(first_url) = self
            .game_urls
            .iter()
            .find(|url| parts::part_index(url) == Some(0))
        {
            self.part_discovery = parts::discover(first_url);
            self.part_discovery_result = None;
            cx.notify();
        }
    }

    fn drain_events(&mut self, cx: &mut Context<Self>) -> bool {
        let mut changed = false;
        if let Some(proxy) = &self.proxy {
            while let Ok(event) = proxy.events.try_recv() {
                changed = true;
                if let Some(mut game) = GamePackage::from_log(&event) {
                    if self
                        .game
                        .as_ref()
                        .is_none_or(|previous| previous.title_id != game.title_id)
                    {
                        self.metadata = Some(Self::metadata_lookup(game.title_id.clone()));
                        self.metadata_error = None;
                        self.cover = None;
                        self.show_game_urls = true;
                        self.game_urls.clear();
                        self.part_discovery = None;
                        self.part_discovery_result = None;
                    } else {
                        game.name = self
                            .game
                            .as_ref()
                            .and_then(|previous| previous.name.clone());
                    }
                    record_game_url(&mut self.game_urls, &game.url);
                    if self.part_discovery.is_none() && self.part_discovery_result.is_none() {
                        self.part_discovery = parts::discover(&game.url);
                    }
                    self.game = Some(game);
                }
                if self.log.len() == 200 {
                    self.log.pop_front();
                }
                self.log.push_back(event);
            }
        }
        if let Some(receiver) = &self.stopping_events {
            while let Ok(event) = receiver.try_recv() {
                changed = true;
                if self.log.len() == 200 {
                    self.log.pop_front();
                }
                self.log.push_back(event);
            }
        }
        if changed {
            self.log_scroller
                .update(cx, |scroller, cx| scroller.reset(self.log.len(), cx));
        }
        changed
    }

    fn poll(&mut self, cx: &mut Context<Self>) {
        let mut changed = self.drain_events(cx);
        if self
            .copy_feedback_until
            .is_some_and(|until| Instant::now() >= until)
        {
            self.copy_feedback = None;
            self.copied_game_url = None;
            self.copy_feedback_until = None;
            changed = true;
        }
        if let Some(transition) = self.proxy_transition.take() {
            match transition {
                ProxyTransition::Starting(receiver) => match receiver.try_recv() {
                    Ok(Ok(proxy)) => {
                        if let Some(path) = &self.package_dir {
                            if let Err(error) = proxy.set_package_dir(Some(path.clone())) {
                                self.proxy_error = Some(error.to_string());
                            }
                        }
                        self.log.clear();
                        self.log_scroller
                            .update(cx, |scroller, cx| scroller.reset(0, cx));
                        self.game = None;
                        self.game_urls.clear();
                        self.part_discovery = None;
                        self.part_discovery_result = None;
                        self.show_game_urls = false;
                        self.metadata = None;
                        self.metadata_error = None;
                        self.cover = None;
                        self.proxy = Some(proxy);
                        self.upload_status_until = None;
                        changed = true;
                    }
                    Ok(Err(error)) => {
                        self.proxy_error = Some(error);
                        changed = true;
                    }
                    Err(TryRecvError::Empty) => {
                        self.proxy_transition = Some(ProxyTransition::Starting(receiver))
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.proxy_error = Some(self.language.text("app.proxy_start_stopped"));
                        changed = true;
                    }
                },
                ProxyTransition::Stopping(receiver) => match receiver.try_recv() {
                    Ok(()) => {
                        self.drain_events(cx);
                        self.stopping_events = None;
                        changed = true;
                    }
                    Err(TryRecvError::Empty) => {
                        self.proxy_transition = Some(ProxyTransition::Stopping(receiver))
                    }
                    Err(TryRecvError::Disconnected) => {
                        self.proxy_error = Some(self.language.text("app.proxy_stop_stopped"));
                        changed = true;
                    }
                },
            }
        }
        if let Some(receiver) = &self.folder_check {
            match receiver.try_recv() {
                Ok(result) => {
                    if result.as_ref().map_or(true, |inspection| {
                        self.package_dir.as_ref() == Some(&inspection.path)
                    }) {
                        self.folder_inspection = Some(result);
                    }
                    self.folder_check = None;
                    changed = true;
                }
                Err(TryRecvError::Disconnected) => {
                    self.folder_inspection =
                        Some(Err(self.language.text("app.folder_check_stopped")));
                    self.folder_check = None;
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        if let Some(receiver) = &self.part_discovery {
            loop {
                match receiver.try_recv() {
                    Ok(parts::DiscoveryEvent::Found(url)) => {
                        record_game_url(&mut self.game_urls, &url);
                        changed = true;
                    }
                    Ok(parts::DiscoveryEvent::Complete(count)) => {
                        self.part_discovery_result = Some(Ok(count));
                        self.part_discovery = None;
                        changed = true;
                        break;
                    }
                    Ok(parts::DiscoveryEvent::Incomplete(error)) => {
                        self.part_discovery_result = Some(Err(error));
                        self.part_discovery = None;
                        changed = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.part_discovery_result = Some(Err("Part search stopped".into()));
                        self.part_discovery = None;
                        changed = true;
                        break;
                    }
                }
            }
        }
        #[cfg(target_os = "linux")]
        if self.tray_events.try_recv().is_ok() {
            self.activate_pending = true;
            changed = true;
        }
        if let Some(receiver) = &self.metadata {
            match receiver.try_recv() {
                Ok(Ok(details)) => {
                    if let Some(game) = &mut self.game {
                        game.name = Some(details.name);
                    }
                    match details.cover {
                        Ok(png) => {
                            self.cover = Some(Arc::new(Image::from_bytes(ImageFormat::Png, png)));
                        }
                        Err(error) => {
                            self.metadata_error = Some(format!(
                                "{}: {error}",
                                self.language.text("app.cover_unavailable")
                            ));
                        }
                    }
                    self.metadata = None;
                    changed = true;
                }
                Ok(Err(error)) => {
                    self.metadata_error = Some(error);
                    self.metadata = None;
                    changed = true;
                }
                Err(TryRecvError::Disconnected) => {
                    self.metadata_error = Some(self.language.text("app.metadata_lookup_stopped"));
                    self.metadata = None;
                    changed = true;
                }
                Err(TryRecvError::Empty) => {}
            }
        }
        let now = Instant::now();
        let snapshot = self.proxy.as_ref().map(|proxy| proxy.transfers.snapshot());
        if let Some(snapshot) = snapshot {
            let next_status = upload_status_until(
                self.upload_status_until,
                !snapshot.active.is_empty(),
                snapshot.bytes > self.transfers.bytes,
                now,
            );
            if self.upload_status_until.is_some() != next_status.is_some() {
                changed = true;
            }
            self.upload_status_until = next_status;
            self.upload_samples.push_back((now, snapshot.bytes));
            while self
                .upload_samples
                .front()
                .is_some_and(|(at, _)| now.duration_since(*at) > UPLOAD_WINDOW)
            {
                self.upload_samples.pop_front();
            }
            let rate = match (self.upload_samples.front(), self.upload_samples.back()) {
                (Some((first_at, first_bytes)), Some((last_at, last_bytes)))
                    if last_bytes > first_bytes =>
                {
                    let seconds = last_at.duration_since(*first_at).as_secs_f32();
                    if seconds > 0.0 {
                        (last_bytes - first_bytes) as f32 / seconds / 1_000_000.0
                    } else {
                        0.0
                    }
                }
                _ => 0.0,
            };
            if (rate - self.upload_rate).abs() > 0.1 {
                self.upload_rate = rate;
                changed = true;
            }
            let previous_url = self
                .transfers
                .active
                .last()
                .map(|active| active.url.clone())
                .or_else(|| self.transfers.last_url.clone());
            let next_url = snapshot
                .active
                .last()
                .map(|active| active.url.clone())
                .or_else(|| snapshot.last_url.clone());
            if previous_url != next_url {
                self.copy_feedback = self.copy_feedback.filter(|key| *key != "app.url_copied");
            }
            if snapshot != self.transfers {
                self.transfers = snapshot;
                if previous_url != next_url {
                    self.scroll_to_active_package();
                }
                changed = true;
            }
        } else {
            if self.upload_status_until.take().is_some() {
                changed = true;
            }
            if !self.dev_mock
                && (self.transfers != proxy::TransferState::default()
                    || !self.upload_samples.is_empty())
            {
                self.transfers = proxy::TransferState::default();
                self.upload_samples.clear();
                self.upload_rate = 0.0;
                changed = true;
            }
        }
        if changed {
            cx.notify();
        }
    }

    fn start(&mut self, cx: &mut Context<Self>) {
        if self.proxy.is_some() || self.proxy_transition.is_some() {
            return;
        }
        let address = self
            .bind_ip
            .read(cx)
            .value()
            .parse::<Ipv4Addr>()
            .map_err(|_| self.language.text("app.invalid_bind_address"))
            .and_then(|ip| {
                self.bind_port
                    .read(cx)
                    .value()
                    .parse::<u16>()
                    .ok()
                    .filter(|port| *port > 0)
                    .map(|port| SocketAddr::from((ip, port)))
                    .ok_or_else(|| self.language.text("app.invalid_port"))
            });
        let Ok(address) = address else {
            self.proxy_error = address.err();
            cx.notify();
            return;
        };
        if self.device_address(cx).is_none() {
            self.proxy_error = Some(self.language.text("app.no_device_address"));
            cx.notify();
            return;
        }
        let package_dir = self.package_dir.clone();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender
                .send(proxy::Proxy::start(address, package_dir).map_err(|error| error.to_string()));
        });
        self.proxy_error = None;
        self.proxy_transition = Some(ProxyTransition::Starting(receiver));
        cx.notify();
    }

    fn stop(&mut self, cx: &mut Context<Self>) {
        if self.proxy_transition.is_some() {
            return;
        }
        self.drain_events(cx);
        if let Some(mut proxy) = self.proxy.take() {
            let (_, empty_events) = mpsc::channel();
            self.stopping_events = Some(std::mem::replace(&mut proxy.events, empty_events));
            let (sender, receiver) = mpsc::channel();
            thread::spawn(move || {
                drop(proxy);
                let _ = sender.send(());
            });
            self.proxy_transition = Some(ProxyTransition::Stopping(receiver));
        }
        self.proxy_error = None;
        cx.notify();
    }

    fn open_logs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, _, cx| {
            let state = app.read(cx);
            let entries = std::rc::Rc::new(state.log.iter().cloned().collect::<Vec<_>>());
            let scroller = state.log_scroller.clone();
            dialog
                .when(state.language.is_rtl(), |dialog| dialog.text_right())
                .w(px(900.))
                .h(px(600.))
                .title(state.language.text("app.network_logs"))
                .child(
                    state
                        .language
                        .text("app.http_urls_omit_query_values_https_paths_remain_e"),
                )
                .child(
                    Button::new("clear-logs")
                        .label(state.language.text("app.clear"))
                        .on_click({
                            let app = app.clone();
                            move |_, _, cx| {
                                app.update(cx, |app, cx| {
                                    app.log.clear();
                                    app.log_scroller
                                        .update(cx, |scroller, cx| scroller.reset(0, cx));
                                    cx.notify();
                                });
                            }
                        }),
                )
                .child(
                    MessageScroller::new("network-logs", scroller, move |index, _, _| {
                        div()
                            .min_w_0()
                            .text_left()
                            .child(entries.get(index).cloned().unwrap_or_default())
                            .into_any_element()
                    })
                    .w_full()
                    .h(px(480.)),
                )
        });
    }

    fn open_update_dialog(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(update) = &self.available_update else {
            return;
        };
        let version = update.version.to_string();
        let download_url = update.download_url.clone();
        let language = self.language;
        let rtl = language.is_rtl();

        window.open_dialog(cx, move |dialog, _, cx| {
            let download_url = download_url.clone();
            dialog
                .overlay(true)
                .when(rtl, |dialog| dialog.text_right())
                .close_button(!rtl)
                .title(
                    DialogHeader::new().child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .w_full()
                            .when(rtl, |row| row.flex_row_reverse())
                            .child(DialogTitle::new().child(language.text("app.update_available")))
                            .when(rtl, |row| {
                                row.child(
                                    Button::new("update-dialog-close")
                                        .small()
                                        .ghost()
                                        .icon(assets::IconName::Close)
                                        .accessibility_label(language.text("app.close"))
                                        .on_click(|_, window, cx| window.close_dialog(cx)),
                                )
                            }),
                    ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_4()
                        .p_4()
                        .bg(cx.theme().group_box)
                        .rounded_md()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(
                            div()
                                .size_12()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_full()
                                .bg(cx.theme().primary.opacity(0.15))
                                .child(
                                    Icon::new(assets::IconName::PackageCheck)
                                        .text_lg()
                                        .text_color(cx.theme().primary),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(language.text("app.update_available"))
                                .child(format!("v{version}"))
                                .child(format!(
                                    "{}: v{}",
                                    language.text("app.current_version"),
                                    env!("CARGO_PKG_VERSION")
                                )),
                        ),
                )
                .child(
                    div()
                        .id("update-dialog-description")
                        .test_support()
                        .w_full()
                        .min_w_0()
                        .child(
                            DialogDescription::new()
                                .w_full()
                                .min_w_0()
                                .whitespace_normal()
                                .when(rtl, |description| description.text_right())
                                .child(
                                    div()
                                        .id("update-dialog-description-text")
                                        .test_support()
                                        .mt_3()
                                        .w_full()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .when(rtl, |text| {
                                            // shortcut: GPUI 0.9.1 clips soft-wrapped RTL glyphs; use flex word wrapping until its text renderer is fixed.
                                            text.flex().flex_col().children(
                                                language
                                                    .text("app.update_dialog_description")
                                                    .lines()
                                                    .map(|line| {
                                                        div()
                                                            .flex()
                                                            .flex_row_reverse()
                                                            .flex_wrap()
                                                            .gap_x_1()
                                                            .children(line.split_whitespace().map(
                                                                |word| {
                                                                    div()
                                                                        .flex_shrink_0()
                                                                        .whitespace_nowrap()
                                                                        .child(word.to_owned())
                                                                },
                                                            ))
                                                    }),
                                            )
                                        })
                                        .when(!rtl, |text| {
                                            text.child(
                                                language.text("app.update_dialog_description"),
                                            )
                                        }),
                                ),
                        ),
                )
                .footer(
                    DialogFooter::new()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(
                            Button::new("update-later")
                                .outline()
                                .label(language.text("app.maybe_later"))
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            Button::new("update-download")
                                .primary()
                                .icon(assets::IconName::Download)
                                .label(language.text("app.download_update"))
                                .on_click(move |_, _, cx| cx.open_url(&download_url)),
                        ),
                )
        });
    }
}

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        #[cfg(target_os = "linux")]
        if self.activate_pending {
            self.activate_pending = false;
            window.activate_window();
        }
        let language = self.language;
        let rtl = language.is_rtl();
        let running = self.proxy.is_some();
        let narrow = window.bounds().size.width < px(640.);
        let status = if let Some(transition) = &self.proxy_transition {
            language.text(match transition {
                ProxyTransition::Starting(_) => "app.starting",
                ProxyTransition::Stopping(_) => "app.stopping",
            })
        } else if let Some(proxy) = &self.proxy {
            let address = self
                .device_address(cx)
                .map(|ip| SocketAddr::from((ip, proxy.address.port())))
                .unwrap_or(proxy.address);
            format!("{} {}", language.text("app.proxy_listening_on"), address)
        } else if self.proxy_error.is_some() {
            language.text("app.failed")
        } else {
            language.text("app.proxy_stopped").to_string()
        };
        let status_button = Button::new("proxy-status")
            .ghost()
            .xsmall()
            .accessibility_label(status.clone())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(rtl, |row| row.flex_row_reverse())
                    .child(
                        div()
                            .id("proxy-status-icon")
                            .test_support()
                            .child(Icon::new(if running {
                                IconName::CircleCheck
                            } else {
                                IconName::CircleX
                            })),
                    )
                    .child(div().id("proxy-status-label").test_support().child(status)),
            )
            .text_color(if running {
                cx.theme().success
            } else {
                cx.theme().muted_foreground
            })
            .on_click(cx.listener(|app, _, _, cx| {
                app.current_step = 0;
                cx.notify();
            }));
        let logs_button = Button::new("open-logs")
            .ghost()
            .xsmall()
            .label(format!(
                "{} ({})",
                language.text("app.logs"),
                self.log.len()
            ))
            .on_click(cx.listener(|app, _, window, cx| app.open_logs(window, cx)));
        let language_button = Button::new("language-select")
            .ghost()
            .xsmall()
            .accessibility_label(if rtl {
                "Language: فارسی"
            } else {
                "Language: English"
            })
            .child(
                img(if rtl { "flags/ir.svg" } else { "flags/gb.svg" })
                    .w(px(18.))
                    .h(px(12.)),
            )
            .child(if rtl { "فارسی" } else { "English" })
            .dropdown_menu(move |menu, _, _| {
                menu.check_side(if rtl { Side::Left } else { Side::Right })
                    .menu_element_with_check(!rtl, Box::new(SelectEnglish), move |_, _| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(rtl, |row| row.flex_row_reverse())
                            .child(img("flags/gb.svg").w(px(18.)).h(px(12.)))
                            .child("English")
                    })
                    .menu_element_with_check(rtl, Box::new(SelectPersian), move |_, _| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .when(rtl, |row| row.flex_row_reverse())
                            .child(img("flags/ir.svg").w(px(18.)).h(px(12.)))
                            .child("فارسی")
                    })
            });
        let status_bar = if rtl {
            StatusBar::new()
                .left(language_button)
                .child(logs_button)
                .right(status_button)
        } else {
            StatusBar::new()
                .left(status_button)
                .child(logs_button)
                .right(language_button)
        };
        let complete = [
            running,
            self.game.is_some(),
            self.folder_inspection
                .as_ref()
                .is_some_and(|result| result.as_ref().is_ok_and(|folder| !folder.files.is_empty())),
            self.upload_done(),
        ];
        let steps = [
            (0, assets::IconName::Network, language.text("app.proxy")),
            (
                1,
                assets::IconName::Package,
                language.text("app.game_content"),
            ),
            (2, assets::IconName::Folder, language.text("app.pkg_folder")),
            (
                3,
                assets::IconName::Upload,
                language.text("app.upload_to_console"),
            ),
        ];
        let mut steps = steps.into_iter().enumerate().collect::<Vec<_>>();
        if rtl {
            steps.reverse();
        }
        let step_count = steps.len();
        let mut stepper = div()
            .id("wizard-stepper")
            .test_support()
            .flex()
            .items_center()
            .gap_2()
            .w_full();
        for (position, (index, icon, title)) in steps {
            let active = self.current_step == index;
            let done = complete[index] && !active;
            let state = language.text(if active {
                "app.current_step"
            } else if done {
                "app.completed_step"
            } else {
                "app.upcoming_step"
            });
            let step_icon = if done {
                assets::IconName::CircleCheck
            } else {
                icon
            };
            let mut item = div()
                .flex()
                .items_center()
                .when(rtl, |item| item.flex_row_reverse())
                .gap_2()
                .flex_1()
                .min_w_0()
                .child(
                    Button::new(("setup-step", index))
                        .ghost()
                        .accessibility_label(format!("{title}, {state}"))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .when(rtl, |row| row.flex_row_reverse())
                                .child(
                                    div()
                                        .id(("step-icon", index))
                                        .test_support()
                                        .w(px(30.))
                                        .h(px(30.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded_md()
                                        .border_1()
                                        .border_color(if active {
                                            rgb(0x6C3632).into()
                                        } else {
                                            cx.theme().border
                                        })
                                        .bg(if active {
                                            rgb(0x362321).into()
                                        } else {
                                            cx.theme().group_box
                                        })
                                        .child(Icon::new(step_icon).text_color(if active {
                                            rgb(0xFF6B5F).into()
                                        } else if done {
                                            cx.theme().success
                                        } else {
                                            cx.theme().muted_foreground
                                        })),
                                )
                                .child(
                                    div()
                                        .id(("step-label", index))
                                        .test_support()
                                        .min_w_0()
                                        .text_xs()
                                        .text_color(if active {
                                            cx.theme().foreground
                                        } else if done {
                                            cx.theme().success
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .child(title),
                                ),
                        )
                        .on_click(cx.listener(move |app, _, _, cx| {
                            app.current_step = index;
                            if index == 3 {
                                app.scroll_to_active_package();
                            }
                            cx.notify();
                        })),
                );
            if position + 1 < step_count {
                item = item.child(div().h(px(1.)).flex_1().bg(cx.theme().border));
            }
            stepper = stepper.child(item);
        }
        div()
            .id("app")
            .test_support()
            .on_action(cx.listener(|app, _: &SelectEnglish, window, cx| {
                app.select_language(Language::English, window, cx)
            }))
            .on_action(cx.listener(|app, _: &SelectPersian, window, cx| {
                app.select_language(Language::Persian, window, cx)
            }))
            .size_full()
            .flex()
            .flex_col()
            .child(
                TitleBar::new().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(rtl, |row| row.flex_row_reverse())
                        .child(
                            div()
                                .id("app-brand-icon")
                                .test_support()
                                .w(px(23.))
                                .h(px(23.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(img("icon.svg").w_full().h_full()),
                        )
                        .child(div().id("app-brand-name").test_support().child("PKGPort"))
                        .child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
                        )
                        .child(
                            Link::new("repository-link")
                                .href("https://github.com/miladmeidanshahi/pkg-port")
                                .child("GitHub"),
                        ),
                ),
            )
            .child(
                div()
                    .id("content")
                    .test_support()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_6()
                    .w_full()
                    .max_w(px(906.))
                    .mx_auto()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .when(rtl, |view| view.text_right())
                    .when_some(self.available_update.as_ref(), |view, update| {
                        view.child(
                            Button::new("update-available")
                                .outline()
                                .accessibility_label(format!(
                                    "{} v{}",
                                    language.text("app.update_available"),
                                    update.version
                                ))
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .when(rtl, |row| row.flex_row_reverse())
                                        .child(Icon::new(assets::IconName::PackageCheck))
                                        .child(format!(
                                            "{} · v{}",
                                            language.text("app.update_available"),
                                            update.version
                                        )),
                                )
                                .on_click(cx.listener(|app, _, window, cx| {
                                    app.open_update_dialog(window, cx);
                                })),
                        )
                    })
                    .child(stepper)
                    .when(self.dev_mock, |view| {
                        view.child(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(language.text("app.dev_preview_notice")),
                        )
                    })
                    .when_some(self.settings_error.as_ref(), |view, error| {
                        view.child(div().text_color(cx.theme().danger).child(format!(
                            "{}: {error}",
                            language.text("app.settings_not_saved")
                        )))
                    })
                    .child(match self.current_step {
                        0 => self.proxy_view(cx, narrow),
                        1 => self.game_step_view(cx, narrow),
                        2 => self.folder_view(cx, narrow),
                        _ => self.upload_view(window, cx, narrow),
                    }),
            )
            .child(status_bar)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        package_parts_from, record_game_url, subnet_label, suggested_interface,
        upload_status_until, App, GamePackage, GuideConsole, IconName, LanInterface, TEST_GAME_URL,
    };
    use crate::ui::{disclosure_chevron, AppAssets};
    use gpui_kit::component::WindowExt as _;
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        px, size, AppContext as _, AssetSource as _, Bounds, Point, TestAppContext, WindowBounds,
        WindowOptions,
    };
    use std::collections::HashMap;
    use std::net::{Ipv4Addr, SocketAddr};
    use std::path::PathBuf;
    use std::time::{Duration, Instant};

    #[test]
    fn uploading_status_survives_short_gaps_and_expires_after_inactivity() {
        let now = Instant::now();
        let until = upload_status_until(None, true, false, now).unwrap();
        assert_eq!(
            upload_status_until(Some(until), false, false, now + Duration::from_secs(1)),
            Some(until)
        );
        assert_eq!(upload_status_until(Some(until), false, false, until), None);
        assert!(upload_status_until(None, false, true, now).is_some());
        assert_eq!(upload_status_until(None, false, false, now), None);
    }

    #[gpui_kit::test]
    fn persian_wizard_renders_and_navigates_at_desktop_and_narrow_sizes(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            super::theme::apply(cx);
            cx.text_system()
                .add_fonts(vec![std::borrow::Cow::Borrowed(
                    include_bytes!("../assets/fonts/InterVariable.ttf").as_slice(),
                )])
                .expect("load Inter font for UI test");
        });
        #[cfg(target_os = "linux")]
        let (tray_stop, tray_events) = std::sync::mpsc::channel();
        let (window, app) = cx
            .update(|cx| {
                let http_client = cx.http_client();
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(Bounds {
                            origin: Point::default(),
                            size: size(px(900.), px(1000.)),
                        })),
                        ..Default::default()
                    },
                    cx,
                    move |window, cx| {
                        cx.new(|cx| {
                            App::new(
                                window,
                                cx,
                                http_client,
                                #[cfg(target_os = "linux")]
                                tray_stop,
                                #[cfg(target_os = "linux")]
                                tray_events,
                            )
                        })
                    },
                )
            })
            .expect("open app test window");

        app.update(cx, |app, cx| {
            app.apply_dev_mock();
            assert_eq!(
                app.game.as_ref().and_then(|game| game.name.as_deref()),
                Some("Grand Theft Auto VI")
            );
            assert_eq!(app.package_parts().len(), 2);
            assert_eq!(app.upload_totals(), (2_500_000_000, 3_000_000_000));
            assert!(app.folder_ready());
            app.language = super::Language::Persian;
            app.persian_fonts_loaded = App::load_persian_fonts(cx);
            let selected_folder = PathBuf::from(
                "/test/a-very-long-folder-name-that-must-not-push-the-package-count-offscreen",
            );
            app.package_dir = Some(selected_folder.clone());
            app.folder_check = None;
            let first_file = TEST_GAME_URL.rsplit('/').next().unwrap().to_owned();
            app.folder_inspection = Some(Ok(super::folder::Inspection {
                path: selected_folder,
                files: HashMap::from([(first_file.clone(), 1_000)]),
            }));
            app.game = Some(GamePackage {
                name: Some("Test Game".into()),
                device: "192.168.1.5:1234".into(),
                title_id: "PPSA01547".into(),
                region: Some("app.region_europe"),
                url: TEST_GAME_URL.into(),
            });
            app.game_urls = vec![TEST_GAME_URL.into()];
            app.show_game_urls = true;
            app.part_discovery_result = Some(Ok(1));
            app.transfers = super::proxy::TransferState {
                bytes: 250,
                last_url: Some(TEST_GAME_URL.into()),
                files: HashMap::from([(
                    first_file.clone(),
                    super::proxy::FileTransfer {
                        size: 1_000,
                        served: 250,
                        complete: false,
                        interrupted: false,
                    },
                )]),
                active: vec![super::proxy::ActiveTransfer {
                    id: 1,
                    peer: SocketAddr::from(([192, 168, 1, 50], 54321)),
                    url: TEST_GAME_URL.into(),
                    file: first_file,
                    sent: 250,
                    total: 1_000,
                }],
            };
            gpui_kit::component::set_locale("fa");
            gpui_kit::component::Theme::update(cx, |theme| {
                theme.font_family = "Vazirmatn".into();
            });
        });

        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let viewport = window.viewport_size();
            assert!(window.find("app").visible());
            assert!(window.find("content").bounds().size.width <= viewport.width);
            for id in ["proxy-status", "open-logs", "language-select"] {
                let bounds = window.find(id).bounds();
                assert!(bounds.origin.x >= px(0.), "{id} starts off-window");
                assert!(bounds.right() <= viewport.width, "{id} ends off-window");
            }

            // In RTL, step 1 is the rightmost step and each step's label is
            // visually to the left of its icon.
            assert!(
                window.find(("setup-step", 0usize)).bounds().left()
                    > window.find(("setup-step", 1usize)).bounds().left()
            );
            assert!(
                window.find(("step-label", 0usize)).bounds().left()
                    < window.find(("step-icon", 0usize)).bounds().left()
            );
            assert!(
                window.find("language-select").bounds().left()
                    < window.find("proxy-status").bounds().left()
            );
            assert!(
                window.find("proxy-status-icon").bounds().left()
                    > window.find("proxy-status-label").bounds().left()
            );
            assert!(
                window.find("app-brand-name").bounds().left()
                    < window.find("app-brand-icon").bounds().left()
            );
            assert!(
                window.find("proxy-guide-title").bounds().left()
                    < window.find("proxy-guide-icon").bounds().left()
            );
            assert!(
                window.find("proxy-guide-note-text").bounds().left()
                    < window.find("proxy-guide-note-icon").bounds().left()
            );
            let note = window.find("proxy-guide-note-text").bounds();
            let row = window.find("proxy-guide-note-row").bounds();
            assert!(note.left() >= row.left());
            assert!(note.right() <= row.right());
            assert!(
                note.size.height > px(20.),
                "Persian guide note should wrap on desktop"
            );
            let text = super::i18n::Language::Persian
                .text("app.any_device_on_those_subnets_may_use_the_proxy_wh");
            let lines = window
                .text_system()
                .shape_text(
                    text.clone().into(),
                    px(12.),
                    &[gpui_kit::TextRun {
                        len: text.len(),
                        font: gpui_kit::font("Vazirmatn"),
                        ..Default::default()
                    }],
                    Some(note.size.width),
                    None,
                )
                .expect("shape Persian guide note");
            for line in &lines {
                assert!(
                    line.size(px(16.)).width <= note.size.width,
                    "Persian guide glyphs must fit the note width"
                );
            }

            window.click("ps5-guide-toggle", cx);
            window.render_frame(cx);
            assert!(window.find("console-guide-image").visible());
            assert!(
                window.find("console-guide-close").bounds().right()
                    < window.find("console-guide-title").bounds().left(),
                "RTL dialog close button should sit to the left of its title"
            );
            assert_eq!(app.read(cx).console_guide, Some((GuideConsole::Ps5, 0)));
            window.scroll(
                "content",
                gpui_kit::ScrollDelta::Pixels(gpui_kit::point(px(0.), px(-800.))),
                cx,
            );
            window.click("console-guide-next", cx);
            window.render_frame(cx);
            assert_eq!(app.read(cx).console_guide, Some((GuideConsole::Ps5, 1)));
            window.close_dialog(cx);
            window.render_frame(cx);
            window.click("ps4-guide-toggle", cx);
            window.render_frame(cx);
            assert_eq!(app.read(cx).console_guide, Some((GuideConsole::Ps4, 0)));
            window.click("console-guide-next", cx);
            window.render_frame(cx);
            assert_eq!(app.read(cx).console_guide, Some((GuideConsole::Ps4, 1)));
            window.click("ps4-guide-toggle", cx);
            window.render_frame(cx);
            window.close_dialog(cx);
            window.render_frame(cx);

            app.update(cx, |app, cx| {
                app.current_step = 1;
                cx.notify();
            });
            window.render_frame(cx);
            for (label, value) in [
                ("game-title-id-label", "game-title-id-value"),
                ("game-region-label", "game-region-value"),
                ("game-device-label", "game-device-value"),
            ] {
                assert!(
                    window.find(label).bounds().left() > window.find(value).bounds().left(),
                    "RTL detail order: {label} should follow the value from the left",
                );
            }
            assert!(
                window.find("game-guide-title").bounds().left()
                    < window.find("game-guide-icon").bounds().left()
            );
            assert!(
                window.find(("copy-game-url", 0usize)).bounds().left()
                    < window.find(("package-url-text", 0usize)).bounds().left()
            );
            assert!(
                window.find("copy-all-game-urls").bounds().left()
                    < window.find("package-part-count").bounds().left()
            );
            assert!(
                window.find("game-next-label").bounds().left()
                    < window.find("game-next-icon").bounds().left()
            );
            window.click("toggle-game-urls", cx);
            window.render_frame(cx);
            assert!(window.try_find("package-part-count").is_none());
            window.click("toggle-game-urls", cx);
            window.render_frame(cx);
            assert!(window.find("package-part-count").visible());

            app.update(cx, |app, cx| {
                app.current_step = 2;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                window.find("folder-guide-title").bounds().left()
                    < window.find("folder-guide-icon").bounds().left()
            );
            let count = window.find("folder-pkg-count");
            assert!(count.visible());
            assert!(count.bounds().size.width > px(0.));
            assert!(count.bounds().left() > window.find("folder-pkg-count-label").bounds().left());
            assert!(
                window.find("copy-folder-path").bounds().left()
                    < window.find("folder-path-value").bounds().left()
            );
            assert!(
                window.find("folder-next-label").bounds().left()
                    < window.find("folder-next-icon").bounds().left()
            );

            app.update(cx, |app, cx| {
                app.current_step = 3;
                cx.notify();
            });
            window.render_frame(cx);
            assert!(
                window.find("upload-back-label").bounds().left()
                    < window.find("upload-back-icon").bounds().left()
            );
            assert!(
                window.find("copy-upload-url").bounds().left()
                    < window.find("upload-url-value").bounds().left()
            );
            assert!(
                window.find(("upload-file-name", 0usize)).bounds().left()
                    > window.find(("upload-file-size", 0usize)).bounds().left()
            );

            app.update(cx, |app, cx| {
                let first = TEST_GAME_URL.trim_end_matches("_0.pkg");
                let mut files = HashMap::new();
                app.game_urls = (0..52)
                    .map(|index| {
                        let url = format!("{first}_{index}.pkg");
                        files.insert(url.rsplit('/').next().unwrap().to_owned(), 1_000);
                        url
                    })
                    .collect();
                app.folder_inspection
                    .as_mut()
                    .unwrap()
                    .as_mut()
                    .unwrap()
                    .files = files;
                app.part_discovery_result = Some(Ok(52));
                let active = app.transfers.active.last_mut().unwrap();
                active.url = app.game_urls[51].clone();
                active.file = active.url.rsplit('/').next().unwrap().to_owned();
                app.scroll_to_active_package();
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.find("package-file-list").bounds().size.height <= px(320.));
            assert!(app.read(cx).package_scroll.offset().y < px(0.));

            for (step, button) in [
                (0, "start"),
                (1, "game-next-folder"),
                (2, "choose-folder"),
                (3, "back-to-folder"),
            ] {
                app.update(cx, |app, cx| {
                    app.current_step = step;
                    cx.notify();
                });
                window.render_frame(cx);
                assert!(window.find(button).visible(), "step {step}: {button}");
                let bounds = window.find(button).bounds();
                assert!(
                    bounds.origin.x >= px(0.),
                    "step {step}: {button} starts off-window"
                );
                assert!(
                    bounds.right() <= viewport.width,
                    "step {step}: {button} ends off-window"
                );
            }

            window.click(("setup-step", 1usize), cx);
            assert!(window.find("game-next-folder").visible());
            window.click("game-next-folder", cx);
            assert!(window.find("choose-folder").visible());
            window.click("continue-to-upload", cx);
            assert!(window.find("back-to-folder").visible());
            window.click("back-to-folder", cx);
            assert!(window.find("continue-to-upload").visible());
        })
        .expect("inspect desktop Persian layout");

        cx.simulate_window_resize(window, size(px(420.), px(1000.)));
        cx.update_window(window, |_, window, cx| {
            for (step, button) in [
                (0, "start"),
                (1, "game-next-folder"),
                (2, "choose-folder"),
                (3, "back-to-folder"),
            ] {
                app.update(cx, |app, cx| {
                    app.current_step = step;
                    cx.notify();
                });
                window.render_frame(cx);
                if step == 0 {
                    let note = window.find("proxy-guide-note-text").bounds();
                    let row = window.find("proxy-guide-note-row").bounds();
                    assert!(note.left() >= row.left());
                    assert!(note.right() <= row.right());
                    assert!(note.size.height > px(20.), "Persian guide note should wrap");
                    assert!(
                        window.find("proxy-guide-note-icon").bounds().top() <= note.top() + px(4.)
                    );
                    app.update(cx, |app, cx| {
                        app.console_guide = Some((GuideConsole::Ps5, 6));
                        cx.notify();
                    });
                    app.update(cx, |app, cx| {
                        app.console_guide = Some((GuideConsole::Ps4, 10));
                        cx.notify();
                    });
                    app.update(cx, |app, cx| {
                        app.console_guide = None;
                        cx.notify();
                    });
                }
                if step == 2 {
                    let count = window.find("folder-pkg-count");
                    assert!(count.visible());
                    assert!(count.bounds().size.width > px(0.));
                    assert!(count.bounds().right() <= px(420.));
                }
                let bounds = window.find(button).bounds();
                assert!(
                    bounds.origin.x >= px(0.),
                    "narrow step {step}: {button} starts off-window"
                );
                assert!(
                    bounds.right() <= px(420.),
                    "narrow step {step}: {button} ends off-window"
                );
            }
            app.update(cx, |app, cx| {
                app.transfers.active[0].file = format!("{}.pkg", "long-name".repeat(20));
                cx.notify();
            });
            window.render_frame(cx);
            let name = window.find(("upload-file-name", 0usize)).bounds();
            let size = window.find(("upload-file-size", 0usize)).bounds();
            assert!(size.top() >= name.top() && size.bottom() <= name.bottom());
            app.update(cx, |app, cx| {
                app.language = super::i18n::Language::English;
                cx.notify();
            });
            window.render_frame(cx);
            let name = window.find(("upload-file-name", 0usize)).bounds();
            let size = window.find(("upload-file-size", 0usize)).bounds();
            assert!(size.top() >= name.top() && size.bottom() <= name.bottom());
            window.click("copy-upload-url", cx);
            assert_eq!(app.read(cx).copy_feedback, Some("app.url_copied"));
            app.update(cx, |app, cx| {
                app.copy_feedback_until = Some(Instant::now() - Duration::from_secs(1));
                app.poll(cx);
                assert_eq!(app.copy_feedback, None);
                app.transfers.active.clear();
                cx.notify();
            });
            window.render_frame(cx);
            assert!(window.find("copy-upload-url").visible());
            app.update(cx, |app, cx| {
                app.current_step = 2;
                app.language = super::i18n::Language::Persian;
                cx.notify();
            });
            window.render_frame(cx);
            let clear = window.find("clear-folder");
            assert!(clear.visible());
            assert!(clear.bounds().right() <= px(420.));
            window.click("clear-folder", cx);
            app.update(cx, |app, cx| {
                app.available_update = Some(super::updates::AvailableUpdate {
                    version: "0.1.2".parse().unwrap(),
                    download_url: "https://github.com/miladmeidanshahi/pkg-port/releases".into(),
                });
                app.open_update_dialog(window, cx);
            });
            window.render_frame(cx);
            let description = window.find("update-dialog-description").bounds();
            assert!(description.left() >= px(0.) && description.right() <= px(420.));
            let text = window.find("update-dialog-description-text").bounds();
            assert!(text.left() >= description.left() && text.right() <= description.right());
            assert!(text.size.height > window.rem_size() * 2.);
            window.close_dialog(cx);
            let app = app.read(cx);
            assert!(app.package_dir.is_none());
            assert!(app.folder_check.is_none());
            assert!(app.folder_inspection.is_none());
            assert!(!app.folder_ready());
        })
        .expect("inspect narrow Persian layout");
    }

    #[test]
    fn disclosure_chevron_respects_rtl_and_open_state() {
        assert!(matches!(
            disclosure_chevron(false, false),
            IconName::ChevronRight
        ));
        assert!(matches!(
            disclosure_chevron(false, true),
            IconName::ChevronLeft
        ));
        assert!(matches!(
            disclosure_chevron(true, false),
            IconName::ChevronDown
        ));
        assert!(matches!(
            disclosure_chevron(true, true),
            IconName::ChevronDown
        ));
    }

    #[test]
    fn package_parts_use_folder_sizes_without_online_discovery() {
        let files = HashMap::from([
            ("game_0.pkg".to_string(), 10),
            ("game_1.pkg".to_string(), 20),
            ("other_0.pkg".to_string(), 5),
            ("notes.txt".to_string(), 1),
        ]);
        let urls = vec!["http://gst.prod.dl.playstation.net/gst/game_1.pkg".to_string()];
        assert_eq!(
            package_parts_from(&files, &urls, None),
            vec![
                ("game_0.pkg".to_string(), 10),
                ("game_1.pkg".to_string(), 20)
            ]
        );
        assert_eq!(package_parts_from(&files, &urls, Some(1)).len(), 1);
        assert!(package_parts_from(&files, &[], None).is_empty());
    }

    #[test]
    fn subnet_label_uses_network_address() {
        assert_eq!(
            subnet_label(
                Ipv4Addr::new(192, 168, 1, 35),
                u32::from(Ipv4Addr::new(255, 255, 255, 0)),
            ),
            "192.168.1.0/24"
        );
    }

    #[test]
    fn suggested_interface_prefers_one_physical_lan() {
        let lan = LanInterface {
            name: "wlan0".into(),
            ip: Ipv4Addr::new(192, 168, 1, 35),
        };
        let virtual_lan = LanInterface {
            name: "docker0".into(),
            ip: Ipv4Addr::new(172, 17, 0, 1),
        };
        assert_eq!(
            suggested_interface(&[lan.clone(), virtual_lan]),
            Some(lan.ip)
        );
        assert_eq!(
            suggested_interface(&[
                lan.clone(),
                LanInterface {
                    name: "eth0".into(),
                    ..lan
                }
            ]),
            None
        );
    }

    #[test]
    fn app_assets_include_color_flags_and_default_icons() {
        for path in [
            "icon.svg",
            "flags/gb.svg",
            "flags/ir.svg",
            "icons/inbox.svg",
            "icons/download.svg",
            "icons/package.svg",
            "icons/monitor.svg",
            "icons/package-check.svg",
            "icons/shield.svg",
        ] {
            assert!(AppAssets.load(path).unwrap().is_some());
        }
        assert!(AppAssets.list("flags/").unwrap().len() == 2);
        for step in 1..=9 {
            assert!(AppAssets
                .load(&format!("guides/ps5/{step}.webp"))
                .unwrap()
                .is_some());
        }
        for picture in 6..=17 {
            assert!(AppAssets
                .load(&format!("guides/ps4/{picture}.jpg"))
                .unwrap()
                .is_some());
        }
    }

    #[test]
    fn package_urls_are_unique_and_bounded() {
        let mut urls = Vec::new();
        record_game_url(&mut urls, TEST_GAME_URL);
        record_game_url(&mut urls, TEST_GAME_URL);
        record_game_url(&mut urls, &"x".repeat(super::MAX_GAME_URL_LENGTH + 1));
        assert_eq!(urls, [TEST_GAME_URL]);
    }
    #[test]
    fn detects_playstation_game_package_url() {
        let entry = format!("192.168.1.5:1234 GET {TEST_GAME_URL}");
        let game = GamePackage::from_log(&entry).unwrap();
        assert_eq!(game.title_id, "PPSA01547");
        assert_eq!(game.url, TEST_GAME_URL);
        assert_eq!(game.device, "192.168.1.5:1234");
        assert!(
            GamePackage::from_log("192.168.1.5:1234 CONNECT gst.prod.dl.playstation.net:443")
                .is_none()
        );
        assert!(GamePackage::from_log(
            &entry.replace("gst.prod.dl.playstation.net", "example.com")
        )
        .is_none());
    }
}
