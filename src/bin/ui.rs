#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use chrono::Local;
use dioxus::prelude::*;
use dioxus::LaunchBuilder;
use dioxus_desktop::tao::dpi::LogicalSize;
use dioxus_desktop::tao::event::Event;
use dioxus_desktop::tao::window::Icon;
use dioxus_desktop::trayicon::{
    self,
    menu::{Menu, MenuItem},
    MouseButton, MouseButtonState, TrayIconEvent,
};
use dioxus_desktop::{
    use_tray_icon_event_handler, use_tray_menu_event_handler, use_window, Config as DesktopConfig,
    WindowBuilder, WindowCloseBehaviour, WindowEvent,
};
use sock5s::config::{
    AccessConfig, AccessMode, AppConfig, AuthConfig, LogConfig, UserEntry, DEFAULT_CONFIG_PATH,
};
use sock5s::run_server_from_path;
use tokio::sync::watch;

const APP_CSS: &str = r#"
body {
    margin: 0;
    font-family: "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif;
    background: #f2f4f7;
    color: #1f2937;
}

.shell {
    min-height: 100vh;
    padding: 10px;
}

.frame {
    max-width: 1540px;
    margin: 0 auto;
    display: grid;
    gap: 8px;
}

.toolbar, .panel, .nav {
    background: #ffffff;
    border: 1px solid #d5dbe3;
    border-radius: 0;
    box-shadow: none;
}

.toolbar {
    padding: 12px 16px;
    display: flex;
    justify-content: space-between;
    gap: 16px;
    align-items: center;
    background: linear-gradient(180deg, #1f3550, #182b42);
    border-color: #182b42;
}

.toolbar-title {
    margin: 0;
    font-size: 16px;
    font-weight: 700;
    color: #f8fbff;
}

.toolbar-meta {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    flex-wrap: wrap;
}

.toolbar-badge {
    border: 1px solid rgba(184, 202, 223, 0.35);
    background: rgba(255, 255, 255, 0.06);
    color: #dfe9f5;
    padding: 4px 10px;
    font-size: 11px;
    letter-spacing: 0.02em;
}

.toolbar-status {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
}

.status-chip {
    border: 1px solid rgba(196, 211, 229, 0.28);
    background: rgba(255, 255, 255, 0.08);
    color: #edf4fb;
    border-radius: 999px;
    padding: 5px 11px;
    font-size: 12px;
}

.status-chip strong {
    font-weight: 600;
}

.workspace {
    display: grid;
    grid-template-columns: 220px minmax(0, 1fr);
    gap: 8px;
    align-items: start;
}

.nav {
    padding: 12px;
    display: grid;
    gap: 8px;
    position: sticky;
    top: 10px;
    background: linear-gradient(180deg, #243447, #1b2735);
    border-color: #1b2735;
}

.nav-title {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: #d9e5f2;
    text-transform: uppercase;
    letter-spacing: 0.06em;
}

.nav-list {
    display: grid;
    gap: 6px;
}

.nav-item {
    border: 1px solid #314255;
    background: #223140;
    color: #c9d7e6;
    border-radius: 0;
    padding: 10px 10px;
    display: grid;
    gap: 3px;
    cursor: pointer;
    text-align: left;
}

.nav-item.active {
    background: #f6fbff;
    border-color: #8ab4e6;
    color: #123f74;
}

.nav-item strong {
    font-size: 12px;
}

.nav-item span {
    font-size: 11px;
    color: inherit;
    opacity: 0.85;
}

.nav-footer {
    border-top: 1px solid #314255;
    padding-top: 10px;
    display: grid;
    gap: 5px;
}

.nav-footer span {
    font-size: 12px;
    color: #a9bbcf;
}

.panel {
    padding: 14px;
    background: #fcfdff;
}

.panel-header {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: start;
    margin-bottom: 10px;
    padding-bottom: 10px;
    border-bottom: 1px solid #d9e0e7;
}

.panel-title {
    margin: 0 0 4px;
    font-size: 19px;
    font-weight: 700;
    color: #182a3d;
}

.panel-side {
    min-width: 260px;
    display: flex;
    gap: 8px;
}

.mini-card {
    border: 1px solid #d7dee7;
    border-radius: 0;
    padding: 8px 10px;
    background: linear-gradient(180deg, #ffffff, #f6f9fc);
    min-width: 126px;
}

.mini-card label {
    display: block;
    margin-bottom: 4px;
    font-size: 11px;
    color: #64717f;
}

.mini-card strong {
    font-size: 13px;
    color: #24364d;
}

.button-row {
    display: grid;
    gap: 10px;
    padding: 0 0 10px;
    border-bottom: 1px solid #d9e0e7;
}

.action-groups {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
}

.action-group {
    border: 1px solid #d7dee7;
    background: #f8fafc;
    padding: 10px;
    display: grid;
    gap: 8px;
}

.action-group-title {
    margin: 0;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: #4f6073;
}

.action-group-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}

.action {
    border: 1px solid #b8c2cf;
    border-radius: 0;
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    background: linear-gradient(180deg, #ffffff, #eef2f6);
    color: #223448;
}

.action:disabled {
    opacity: 0.55;
    cursor: not-allowed;
}

.action.primary { background: linear-gradient(180deg, #2f88ea, #1d6fcb); border-color: #1d6fcb; color: #ffffff; }
.action.secondary { background: linear-gradient(180deg, #ffffff, #edf1f5); color: #334155; }
.action.warn { background: linear-gradient(180deg, #fff8ef, #f7e7ce); border-color: #dfbf8d; color: #8d5a14; }
.action.stop { background: linear-gradient(180deg, #fff4f4, #f5dada); border-color: #d7aaaa; color: #9e2d2d; }

.hint-action {
    border: 1px solid #c7d3df;
    background: #f7fafc;
    color: #35506d;
    padding: 4px 10px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}

.msg-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(20, 29, 40, 0.28);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
    z-index: 9999;
}

.msg-dialog {
    width: min(460px, calc(100vw - 48px));
    background: #ffffff;
    border: 1px solid #d5dbe3;
    box-shadow: 0 18px 50px rgba(17, 24, 39, 0.18);
}

.msg-dialog.success {
    border-color: #b9d8c2;
}

.msg-dialog.error {
    border-color: #e2b8b8;
}

.msg-header {
    padding: 12px 14px;
    border-bottom: 1px solid #dde4eb;
    font-size: 14px;
    font-weight: 700;
    color: #213246;
}

.msg-dialog.success .msg-header {
    background: #f1f9f3;
    color: #245534;
}

.msg-dialog.error .msg-header {
    background: #fdf3f3;
    color: #8c2f2f;
}

.msg-body {
    padding: 14px;
    display: grid;
    gap: 10px;
    font-size: 13px;
    color: #314255;
}

.msg-summary {
    line-height: 1.6;
}

.msg-meta {
    font-size: 12px;
    color: #6b7785;
}

.msg-detail {
    border: 1px solid #dde4eb;
    background: #f8fafc;
    padding: 10px 12px;
    font-size: 12px;
    color: #415063;
    white-space: pre-wrap;
    word-break: break-word;
}

.msg-actions {
    display: flex;
    justify-content: flex-end;
    padding: 0 14px 14px;
}

.content-stack {
    display: grid;
    gap: 10px;
    margin-top: 12px;
}

.summary-grid {
    display: grid;
    grid-template-columns: 1.2fr 1fr;
    gap: 10px;
}

.overview-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
}

.overview-card {
    border: 1px solid #d7dee7;
    border-radius: 0;
    padding: 12px;
    background: #ffffff;
    display: grid;
    gap: 6px;
}

.overview-card label {
    color: #607082;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
}

.overview-card strong {
    font-size: 20px;
    color: #24364d;
}

.overview-card span {
    color: #5f6b7a;
    font-size: 13px;
    line-height: 1.55;
}

.section {
    border: 1px solid #d7dee7;
    border-radius: 0;
    padding: 12px;
    background: #ffffff;
}

.section-title {
    margin: 0 0 10px;
    font-size: 13px;
    font-weight: 600;
    color: #24364d;
}

.section-header {
    display: flex;
    justify-content: space-between;
    gap: 10px;
    align-items: start;
    margin-bottom: 10px;
}

.section-header .section-title {
    margin: 0 0 4px;
}

.section-meta {
    font-size: 11px;
    color: #718092;
}

.grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px 12px;
}

.field, .wide {
    display: grid;
    gap: 6px;
}

.wide {
    grid-column: 1 / -1;
}

.field label, .wide label {
    font-size: 12px;
    font-weight: 600;
    color: #334155;
}

.input, .textarea {
    width: 100%;
    box-sizing: border-box;
    border: 1px solid #b7c1cd;
    border-radius: 0;
    padding: 8px 9px;
    font-size: 13px;
    background: #ffffff;
    color: #1f2937;
    outline: none;
}

.input:focus, .textarea:focus {
    border-color: #2b78d6;
    box-shadow: inset 0 0 0 1px #2b78d6;
}

.textarea {
    min-height: 120px;
    resize: vertical;
    line-height: 1.5;
}

.toggle-row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}

.toggle {
    border: 1px solid #b8c2cf;
    background: linear-gradient(180deg, #ffffff, #edf1f5);
    color: #334155;
    padding: 7px 11px;
    border-radius: 0;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
}

.toggle.active {
    background: linear-gradient(180deg, #dcecff, #c7defa);
    border-color: #74a7e7;
    color: #0d4f9c;
}

.logs-toolbar {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
}

.logs-meta {
    font-size: 12px;
    color: #607082;
}

.logs-controls {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
}

.search-input {
    min-width: 240px;
}

.status-strip {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
}

.status-box {
    border: 1px solid #cfd6df;
    background: #f7f9fb;
    padding: 10px 12px;
}

.status-box label {
    display: block;
    font-size: 11px;
    color: #6a7787;
    margin-bottom: 4px;
}

.status-box strong {
    font-size: 13px;
    color: #213548;
}

.logs {
    min-height: 620px;
    border-radius: 0;
    border: 1px solid #d7dee7;
    background: #ffffff;
    color: #1f2937;
    padding: 0;
}

.log-list {
    display: grid;
    gap: 0;
}

.log-item {
    border-bottom: 1px solid #dde3ea;
    padding: 8px 10px;
    background: #ffffff;
}

.log-headless {
    background: #edf2f7;
    color: #33485d;
}

.log-head {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    align-items: center;
    margin-bottom: 5px;
}

.log-time {
    font-weight: 600;
    color: #23364c;
    font-size: 12px;
}

.log-kind {
    font-size: 11px;
    padding: 2px 8px;
    border-radius: 999px;
    background: #edf2f7;
    color: #405469;
    border: 1px solid #d7dee8;
}

.log-grid {
    display: grid;
    grid-template-columns: 160px 1fr 150px 1fr 1fr;
    gap: 8px;
    font-size: 12px;
    color: #4b5563;
}

.log-empty {
    color: #667281;
    font-size: 13px;
    line-height: 1.6;
    padding: 12px;
}

@media (max-width: 1200px) {
    .workspace, .panel-header, .summary-grid {
        grid-template-columns: 1fr;
        display: grid;
    }

    .panel-side {
        min-width: 0;
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

@media (max-width: 900px) {
    .overview-grid, .grid, .log-grid, .panel-side, .toolbar, .status-strip, .action-groups {
        grid-template-columns: 1fr;
    }
}
"#;

fn main() {
    let mut config = DesktopConfig::new().with_window(
        WindowBuilder::new()
            .with_title("sock5s 控制中心")
            .with_inner_size(LogicalSize::new(1360.0, 900.0))
            .with_resizable(false),
    )
    .with_close_behaviour(WindowCloseBehaviour::LastWindowHides)
    .with_custom_event_handler(|event, _| {
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            match prompt_close_action() {
                CloseAction::Exit => graceful_exit(),
                CloseAction::Tray => {}
            }
        }
    });
    if let Ok(icon) = build_window_icon() {
        config = config.with_icon(icon);
    }

    LaunchBuilder::desktop().with_cfg(config).launch(App);
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum UiPage {
    Overview,
    Basic,
    Access,
    Logs,
}

impl UiPage {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Basic => "基础设置",
            Self::Access => "账号与访问控制",
            Self::Logs => "日志监控",
        }
    }

    fn note(self) -> &'static str {
        match self {
            Self::Overview => "集中查看服务状态、配置摘要和日志概况，不再把所有配置都堆在一个长表单里。",
            Self::Basic => "单独维护监听地址、日志目录和日志保留策略。",
            Self::Access => "单独维护代理账号、白名单或黑名单策略，以及域名转发开关。",
            Self::Logs => "单独查看最近日志、刷新状态和运行过程中的失败原因。",
        }
    }
}

#[component]
fn App() -> Element {
    let initial = ConfigForm::load_initial().unwrap_or_default();
    let initial_log_dir = initial.log_dir.clone();
    let window = use_window();
    let form = use_signal(|| initial);
    let config_loaded_at = use_signal(now_display);
    let mut page = use_signal(|| UiPage::Overview);
    let status = use_signal(|| "未启动".to_string());
    let message = use_signal(|| None::<UiMessage>);
    let auto_refresh_logs = use_signal(|| true);
    let log_search = use_signal(String::new);
    let log_filter = use_signal(|| LogFilter::All);
    let mut logs = use_signal(|| read_recent_logs(Path::new(&initial_log_dir)).unwrap_or_default());
    let tray = use_hook(init_tray_handles);

    use_tray_menu_event_handler({
        let window = window.clone();
        let show_id = tray.show_item.id().clone();
        let exit_id = tray.exit_item.id().clone();
        move |event| {
            if event.id == show_id {
                restore_window(&window);
            } else if event.id == exit_id {
                graceful_exit();
            }
        }
    });

    use_tray_icon_event_handler({
        let window = window.clone();
        move |event| match event {
            TrayIconEvent::DoubleClick { .. } => restore_window(&window),
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => restore_window(&window),
            _ => {}
        }
    });

    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !*auto_refresh_logs.read() {
                continue;
            }
            let log_dir = form.read().log_dir.clone();
            logs.set(read_recent_logs(Path::new(&log_dir)).unwrap_or_default());
        }
    });

    let running = controller().lock().unwrap().is_running();
    let snapshot = form.read().clone();
    let validation_errors = snapshot.validate();
    let mode_label = match snapshot.access_mode {
        AccessMode::Blacklist => "黑名单",
        AccessMode::Whitelist => "白名单",
    };
    let running_label = if running { "运行中" } else { "未运行" };
    let auth_label = if snapshot.users_text.trim().is_empty() {
        "无认证"
    } else {
        "用户名密码"
    };
    let domain_forwarding_label = if snapshot.allow_domains { "允许" } else { "禁止" };
    let auto_refresh_label = if *auto_refresh_logs.read() {
        "自动刷新已开启"
    } else {
        "自动刷新已关闭"
    };
    let filtered_logs = filter_logs(
        &logs.read(),
        *log_filter.read(),
        log_search.read().as_str(),
    );
    let page_title = page.read().title();
    let page_note = page.read().note();

    rsx! {
        style { "{APP_CSS}" }
        div { class: "shell",
            div { class: "frame",
                div { class: "toolbar",
                    div {
                        h1 { class: "toolbar-title", "sock5s 控制中心" }
                    }
                    div { class: "toolbar-status",
                        div { class: "status-chip", strong { "状态：" } "{status.read()}" }
                        div { class: "status-chip", strong { "认证：" } "{auth_label}" }
                        div { class: "status-chip", strong { "策略：" } "{mode_label}" }
                        div { class: "status-chip", strong { "域名：" } "{domain_forwarding_label}" }
                    }
                }

                div { class: "workspace",
                    Sidebar {
                        current: *page.read(),
                        running_label: running_label.to_string(),
                        mode_label: mode_label.to_string(),
                        domain_forwarding_label: domain_forwarding_label.to_string(),
                        message,
                        onchange: move |next| page.set(next),
                    }

                    div { class: "panel",
                        div { class: "panel-header",
                            div {
                                h2 { class: "panel-title", "{page_title}" }
                                HintButton {
                                    message,
                                    label: "模块说明",
                                    title: format!("{page_title} 说明"),
                                    summary: page_note.to_string(),
                                }
                            }
                            div { class: "panel-side",
                                div { class: "mini-card",
                                    label { "日志目录" }
                                    strong { "{snapshot.log_dir}" }
                                }
                            }
                        }

                        ActionBar {
                            form,
                            logs,
                            config_loaded_at,
                            status,
                            message,
                            auto_refresh_logs,
                            validation_errors: validation_errors.clone(),
                        }

                        div { class: "content-stack",
                            if *page.read() == UiPage::Overview {
                                OverviewPage {
                                    snapshot: snapshot.clone(),
                                    running_label: running_label.to_string(),
                                    mode_label: mode_label.to_string(),
                                    auth_label: auth_label.to_string(),
                                    domain_forwarding_label: domain_forwarding_label.to_string(),
                                    auto_refresh_label: auto_refresh_label.to_string(),
                                    log_count: logs.read().len(),
                                    message,
                                }
                            }

                            if *page.read() == UiPage::Basic {
                                BasicSettingsPage { form, snapshot: snapshot.clone(), message }
                            }

                            if *page.read() == UiPage::Access {
                                AccessSettingsPage { form, snapshot: snapshot.clone(), mode_label: mode_label.to_string(), message }
                            }

                            if *page.read() == UiPage::Logs {
                                LogsPage {
                                    entries: filtered_logs,
                                    total_entries: logs.read().len(),
                                    running_label: running_label.to_string(),
                                    domain_forwarding_label: domain_forwarding_label.to_string(),
                                    auto_refresh_label: auto_refresh_label.to_string(),
                                    log_search,
                                    log_filter,
                                    message,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn Sidebar(
    current: UiPage,
    running_label: String,
    mode_label: String,
    domain_forwarding_label: String,
    message: Signal<Option<UiMessage>>,
    onchange: EventHandler<UiPage>,
) -> Element {
    rsx! {
        aside { class: "nav",
            div {
                h3 { class: "nav-title", "模块导航" }
                HintButton {
                    message,
                    label: "导航说明",
                    title: "模块导航说明",
                    summary: "按模块切换配置页。",
                }
            }
            div { class: "nav-list",
                NavButton {
                    active: current == UiPage::Overview,
                    title: "概览",
                    note: "服务状态和配置摘要",
                    onclick: move |_| onchange.call(UiPage::Overview),
                }
                NavButton {
                    active: current == UiPage::Basic,
                    title: "基础设置",
                    note: "监听地址、日志、配置文件",
                    onclick: move |_| onchange.call(UiPage::Basic),
                }
                NavButton {
                    active: current == UiPage::Access,
                    title: "账号与访问控制",
                    note: "账号、白名单/黑名单、域名策略",
                    onclick: move |_| onchange.call(UiPage::Access),
                }
                NavButton {
                    active: current == UiPage::Logs,
                    title: "日志监控",
                    note: "最近日志和失败原因",
                    onclick: move |_| onchange.call(UiPage::Logs),
                }
            }
            div { class: "nav-footer",
                span { "服务：{running_label}" }
                span { "策略：{mode_label}" }
                span { "域名转发：{domain_forwarding_label}" }
            }
        }
    }
}

#[component]
fn NavButton(
    active: bool,
    title: &'static str,
    note: &'static str,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if active { "nav-item active" } else { "nav-item" },
            onclick: move |evt| onclick.call(evt),
            strong { "{title}" }
            span { "{note}" }
        }
    }
}

#[component]
fn HintButton(
    message: Signal<Option<UiMessage>>,
    label: &'static str,
    title: String,
    summary: String,
    detail: Option<String>,
) -> Element {
    rsx! {
        button {
            class: "hint-action",
            onclick: move |_| show_info_message(message, title.clone(), summary.clone(), detail.clone()),
            "{label}"
        }
    }
}

#[component]
fn MessageDialog(message: Signal<Option<UiMessage>>) -> Element {
    let current = message.read().clone();
    let Some(current) = current else {
        return rsx! {};
    };

    let dialog_class = match current.kind {
        UiMessageKind::Success => "msg-dialog success",
        UiMessageKind::Info => "msg-dialog",
        UiMessageKind::Error => "msg-dialog error",
    };

    rsx! {
        div {
            class: "msg-backdrop",
            onclick: move |_| message.set(None),
            div {
                class: "{dialog_class}",
                onclick: move |evt| evt.stop_propagation(),
                div { class: "msg-header", "{current.title}" }
                div { class: "msg-body",
                    p { class: "msg-summary", "{current.summary}" }
                    if let Some(detail) = current.detail.clone() {
                        details {
                            summary { class: "msg-meta", "详情" }
                            div { class: "msg-detail", "{detail}" }
                        }
                    }
                }
                div { class: "msg-actions",
                    button {
                        class: if current.kind == UiMessageKind::Success { "action secondary" } else { "action stop" },
                        onclick: move |_| message.set(None),
                        "确定"
                    }
                }
            }
        }
    }
}

#[component]
fn ActionBar(
    form: Signal<ConfigForm>,
    logs: Signal<Vec<LogEntry>>,
    config_loaded_at: Signal<String>,
    status: Signal<String>,
    message: Signal<Option<UiMessage>>,
    auto_refresh_logs: Signal<bool>,
    validation_errors: Vec<String>,
) -> Element {
    let save_validation_errors = validation_errors.clone();
    let export_validation_errors = validation_errors.clone();
    let start_validation_errors = validation_errors.clone();

    rsx! {
        div { class: "button-row",
            div { class: "action-groups",
                div { class: "action-group",
                    p { class: "action-group-title", "配置管理" }
                    div { class: "action-group-row",
                        button {
                            class: "action secondary",
                            onclick: move |_| {
                                match ConfigForm::load_initial() {
                                    Ok(next) => {
                                        let log_dir = next.log_dir.clone();
                                        form.set(next);
                                        config_loaded_at.set(now_display());
                                        logs.set(read_recent_logs(Path::new(&log_dir)).unwrap_or_default());
                                        show_success_message(message, "配置已重新加载", format!("已从 {} 重新加载配置", DEFAULT_CONFIG_PATH));
                                    }
                                    Err(err) => show_error_message(message, "加载配置失败", err),
                                }
                            },
                            "重新加载"
                        }
                        button {
                            class: "action secondary",
                            onclick: move |_| {
                                if !save_validation_errors.is_empty() {
                                    show_validation_message(message, &save_validation_errors);
                                    return;
                                }
                                let current = form.read().clone();
                                match current.save() {
                                    Ok(_) => {
                                        config_loaded_at.set(now_display());
                                        show_success_message(message, "配置已保存", format!("配置已保存到 {}", DEFAULT_CONFIG_PATH));
                                    }
                                    Err(err) => show_error_message(message, "保存配置失败", err),
                                }
                            },
                            "保存配置"
                        }
                        button {
                            class: "action secondary",
                            onclick: move |_| {
                                if !export_validation_errors.is_empty() {
                                    show_validation_message(message, &export_validation_errors);
                                    return;
                                }
                                let current = form.read().clone();
                                match export_config(&current) {
                                    Ok(path) => show_success_message(message, "快照已导出", format!("已导出配置快照：{}", path.display())),
                                    Err(err) => show_error_message(message, "导出快照失败", err),
                                }
                            },
                            "导出快照"
                        }
                    }
                }
                div { class: "action-group",
                    p { class: "action-group-title", "服务控制" }
                    div { class: "action-group-row",
                        button {
                            class: "action primary",
                            onclick: move |_| {
                                if !start_validation_errors.is_empty() {
                                    show_validation_message(message, &start_validation_errors);
                                    return;
                                }
                                let current = form.read().clone();
                                match current.save().and_then(|_| {
                                    let mut guard = controller().lock().unwrap();
                                    guard.start(DEFAULT_CONFIG_PATH.to_string())
                                }) {
                                    Ok(_) => {
                                        status.set(format!("运行中 · {}", current.listen));
                                        logs.set(read_recent_logs(Path::new(&current.log_dir)).unwrap_or_default());
                                        show_success_message(message, "代理已启动", "代理服务已启动");
                                    }
                                    Err(err) => show_error_message(message, "启动代理失败", err),
                                }
                            },
                            "启动代理"
                        }
                        button {
                            class: "action stop",
                            onclick: move |_| {
                                let log_dir = form.read().log_dir.clone();
                                match controller().lock().unwrap().stop() {
                                    Ok(_) => {
                                        status.set("未启动".to_string());
                                        logs.set(read_recent_logs(Path::new(&log_dir)).unwrap_or_default());
                                        show_success_message(message, "代理已停止", "代理服务已停止");
                                    }
                                    Err(err) => show_error_message(message, "停止代理失败", err),
                                }
                            },
                            "停止代理"
                        }
                    }
                }
                div { class: "action-group",
                    p { class: "action-group-title", "日志运维" }
                    div { class: "action-group-row",
                        button {
                            class: "action warn",
                            onclick: move |_| {
                                let log_dir = form.read().log_dir.clone();
                                logs.set(read_recent_logs(Path::new(&log_dir)).unwrap_or_default());
                                show_success_message(message, "日志已刷新", "日志已刷新");
                            },
                            "刷新日志"
                        }
                        button {
                            class: if *auto_refresh_logs.read() { "action secondary" } else { "action warn" },
                            onclick: move |_| {
                                let next = !*auto_refresh_logs.read();
                                auto_refresh_logs.set(next);
                            },
                            if *auto_refresh_logs.read() { "关闭自动刷新" } else { "开启自动刷新" }
                        }
                    }
                }
            }
            MessageDialog { message }
        }
    }
}

fn show_success_message(mut message: Signal<Option<UiMessage>>, title: impl Into<String>, summary: impl Into<String>) {
    message.set(Some(UiMessage {
        kind: UiMessageKind::Success,
        title: title.into(),
        summary: summary.into(),
        detail: None,
    }));
}

fn show_error_message(mut message: Signal<Option<UiMessage>>, title: impl Into<String>, detail: impl Into<String>) {
    let detail = detail.into();
    let summary = detail.lines().next().unwrap_or("发生错误").to_string();
    message.set(Some(UiMessage {
        kind: UiMessageKind::Error,
        title: title.into(),
        summary,
        detail: Some(detail),
    }));
}

fn show_validation_message(message: Signal<Option<UiMessage>>, errors: &[String]) {
    let detail = errors.join("\n");
    show_error_message(message, "配置校验未通过", detail);
}

fn show_info_message(
    mut message: Signal<Option<UiMessage>>,
    title: impl Into<String>,
    summary: impl Into<String>,
    detail: Option<String>,
) {
    message.set(Some(UiMessage {
        kind: UiMessageKind::Info,
        title: title.into(),
        summary: summary.into(),
        detail,
    }));
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogFilter {
    All,
    Access,
    Auth,
    Failed,
    Policy,
    System,
}

#[derive(Clone, PartialEq)]
enum UiMessageKind {
    Success,
    Info,
    Error,
}

#[derive(Clone, PartialEq)]
struct UiMessage {
    kind: UiMessageKind,
    title: String,
    summary: String,
    detail: Option<String>,
}

impl LogFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "全部事件",
            Self::Access => "访问成功",
            Self::Auth => "认证事件",
            Self::Failed => "连接失败",
            Self::Policy => "策略拦截",
            Self::System => "系统事件",
        }
    }
}

#[component]
fn OverviewPage(
    snapshot: ConfigForm,
    running_label: String,
    mode_label: String,
    auth_label: String,
    domain_forwarding_label: String,
    auto_refresh_label: String,
    log_count: usize,
    message: Signal<Option<UiMessage>>,
) -> Element {
    let user_count = snapshot
        .users_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let ip_count = snapshot
        .ips_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();
    let cidr_count = snapshot
        .cidrs_text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count();

    rsx! {
        div { class: "summary-grid",
            div { class: "section",
                div { class: "section-header",
                    div {
                        h3 { class: "section-title", "运行摘要" }
                        HintButton {
                            message,
                            label: "说明",
                            title: "运行摘要说明".to_string(),
                            summary: "用于快速确认当前代理实例的运行状态、接入方式和访问边界。".to_string(),
                        }
                    }
                    div { class: "section-meta", "面向运维与内部交付场景" }
                }
                    div { class: "status-strip",
                    div { class: "status-box",
                        label { "托盘行为" }
                        strong { "关闭时提示后缩到托盘" }
                    }
                    div { class: "status-box",
                        label { "主窗口恢复" }
                        strong { "托盘左键或双击恢复" }
                    }
                    div { class: "status-box",
                        label { "配置状态" }
                        strong { if snapshot.validate().is_empty() { "校验通过" } else { "待修正" } }
                    }
                    div { class: "status-box",
                        label { "固定配置" }
                        strong { "{DEFAULT_CONFIG_PATH}" }
                    }
                }
            }
            div { class: "section",
                div { class: "section-header",
                    div {
                        h3 { class: "section-title", "治理规则" }
                        HintButton {
                            message,
                            label: "说明",
                            title: "治理规则说明".to_string(),
                            summary: "将认证方式、访问策略和日志采集行为集中展示，便于交付说明。".to_string(),
                        }
                    }
                    div { class: "section-meta", "统一策略视图" }
                }
                div { class: "grid",
                    div { class: "field",
                        label { "认证模式" }
                        input { class: "input", value: "{auth_label}", readonly: true }
                    }
                    div { class: "field",
                        label { "访问策略" }
                        input { class: "input", value: "{mode_label}", readonly: true }
                    }
                    div { class: "field",
                        label { "域名转发" }
                        input { class: "input", value: "{domain_forwarding_label}", readonly: true }
                    }
                    div { class: "field",
                        label { "日志刷新" }
                        input { class: "input", value: "{auto_refresh_label}", readonly: true }
                    }
                }
            }
        }
        div { class: "overview-grid",
            div { class: "overview-card",
                label { "服务状态" }
                strong { "{running_label}" }
                span { "监听地址：{snapshot.listen}" }
            }
            div { class: "overview-card",
                label { "认证方式" }
                strong { "{auth_label}" }
                span { "当前代理账号数量：{user_count}" }
            }
            div { class: "overview-card",
                label { "访问策略" }
                strong { "{mode_label}" }
                span { "目标 IP：{ip_count} 条 ｜ CIDR：{cidr_count} 条" }
            }
            div { class: "overview-card",
                label { "域名与日志" }
                strong { "{domain_forwarding_label}" }
                span { "日志：{log_count} 条 ｜ {auto_refresh_label}" }
            }
        }
        div { class: "section",
                div { class: "section-header",
                    div {
                        h3 { class: "section-title", "当前配置摘要" }
                        HintButton {
                            message,
                            label: "说明",
                            title: "当前配置摘要说明".to_string(),
                            summary: "这里仅用于快速确认当前生效配置，详细编辑请进入左侧对应模块。".to_string(),
                        }
                }
                div { class: "section-meta", "配置基线视图" }
            }
            div { class: "grid",
                div { class: "field",
                    label { "日志目录" }
                    input { class: "input", value: "{snapshot.log_dir}", readonly: true }
                }
                div { class: "field",
                    label { "日志保留天数" }
                    input { class: "input", value: "{snapshot.retention_days}", readonly: true }
                }
                div { class: "field",
                    label { "单文件大小上限（MB）" }
                    input { class: "input", value: "{snapshot.max_file_size_mb}", readonly: true }
                }
                div { class: "field",
                    label { "固定配置文件" }
                    input { class: "input", value: "{DEFAULT_CONFIG_PATH}", readonly: true }
                }
            }
        }
    }
}

#[component]
fn BasicSettingsPage(form: Signal<ConfigForm>, snapshot: ConfigForm, message: Signal<Option<UiMessage>>) -> Element {
    rsx! {
        div { class: "section",
                div { class: "section-header",
                    div {
                        h3 { class: "section-title", "基础设置" }
                        HintButton {
                            message,
                            label: "说明",
                            title: "基础设置说明".to_string(),
                            summary: "集中维护监听地址和日志策略，配置固定写入运行目录下的 config.toml。".to_string(),
                        }
                }
                div { class: "section-meta", "基础参数模块" }
            }
            div { class: "grid",
                div { class: "field",
                    label { "监听地址" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "监听地址说明".to_string(),
                        summary: "格式示例：0.0.0.0:1080".to_string(),
                    }
                    input {
                        class: "input",
                        value: "{snapshot.listen}",
                        oninput: move |evt| form.with_mut(|f| f.listen = evt.value()),
                    }
                }
                div { class: "field",
                    label { "日志目录" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "日志目录说明".to_string(),
                        summary: "用于保存 7 天滚动日志文件。".to_string(),
                    }
                    input {
                        class: "input",
                        value: "{snapshot.log_dir}",
                        oninput: move |evt| form.with_mut(|f| f.log_dir = evt.value()),
                    }
                }
                div { class: "field",
                    label { "日志保留天数" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "日志保留天数说明".to_string(),
                        summary: "建议与企业内网审计周期保持一致。".to_string(),
                    }
                    input {
                        class: "input",
                        value: "{snapshot.retention_days}",
                        oninput: move |evt| form.with_mut(|f| f.retention_days = evt.value()),
                    }
                }
                div { class: "field",
                    label { "单文件大小上限（MB）" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "单文件大小上限说明".to_string(),
                        summary: "超过上限时自动切分新日志文件。".to_string(),
                    }
                    input {
                        class: "input",
                        value: "{snapshot.max_file_size_mb}",
                        oninput: move |evt| form.with_mut(|f| f.max_file_size_mb = evt.value()),
                    }
                }
                div { class: "field",
                    label { "固定配置文件" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "固定配置文件说明".to_string(),
                        summary: "启动目录下不存在时会自动生成默认 config.toml。".to_string(),
                    }
                    input { class: "input", value: "{DEFAULT_CONFIG_PATH}", readonly: true }
                }
            }
        }
    }
}

#[component]
fn AccessSettingsPage(
    form: Signal<ConfigForm>,
    snapshot: ConfigForm,
    mode_label: String,
    message: Signal<Option<UiMessage>>,
) -> Element {
    let domain_toggle_label = if snapshot.allow_domains { "已开启" } else { "已关闭" };

    rsx! {
        div { class: "section",
            div { class: "section-header",
                div {
                    h3 { class: "section-title", "账号与访问控制" }
                    HintButton {
                        message,
                        label: "说明",
                        title: "访问控制说明".to_string(),
                        summary: "将认证、目标访问边界和域名转发策略统一收口，适合内部治理场景。".to_string(),
                    }
                }
                div { class: "section-meta", "访问治理模块" }
            }
            div { class: "grid",
                div { class: "wide",
                    label { "代理账号（每行一个，格式：用户名=密码）" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "代理账号说明".to_string(),
                        summary: "留空表示无认证；填写后启用用户名密码认证。".to_string(),
                    }
                    textarea {
                        class: "textarea",
                        value: "{snapshot.users_text}",
                        oninput: move |evt| form.with_mut(|f| f.users_text = evt.value()),
                    }
                }
                div { class: "field",
                    label { "访问策略" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "访问策略说明".to_string(),
                        summary: "黑名单表示禁止命中；白名单表示仅允许命中。".to_string(),
                    }
                    div { class: "toggle-row",
                        button {
                            class: if snapshot.access_mode == AccessMode::Blacklist { "toggle active" } else { "toggle" },
                            onclick: move |_| form.with_mut(|f| f.access_mode = AccessMode::Blacklist),
                            "黑名单"
                        }
                        button {
                            class: if snapshot.access_mode == AccessMode::Whitelist { "toggle active" } else { "toggle" },
                            onclick: move |_| form.with_mut(|f| f.access_mode = AccessMode::Whitelist),
                            "白名单"
                        }
                    }
                }
                div { class: "field",
                    label { "域名转发" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "域名转发说明".to_string(),
                        summary: "关闭后仅允许目标为 IP，适合更强约束场景。".to_string(),
                    }
                    div { class: "toggle-row",
                        button {
                            class: if snapshot.allow_domains { "toggle active" } else { "toggle" },
                            onclick: move |_| form.with_mut(|f| f.allow_domains = !f.allow_domains),
                            "{domain_toggle_label}"
                        }
                    }
                }
                div { class: "field",
                    label { "当前策略说明" }
                    input { class: "input", value: "{mode_label}", readonly: true }
                }
                div { class: "field",
                    label { "域名目标支持" }
                    input {
                        class: "input",
                        value: if snapshot.allow_domains { "允许域名转发" } else { "仅允许目标为 IP" },
                        readonly: true
                    }
                }
                div { class: "wide",
                    label { "目标 IP 列表（每行一个）" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "目标 IP 列表说明".to_string(),
                        summary: "用于精确控制可达目标地址。".to_string(),
                    }
                    textarea {
                        class: "textarea",
                        value: "{snapshot.ips_text}",
                        oninput: move |evt| form.with_mut(|f| f.ips_text = evt.value()),
                    }
                }
                div { class: "wide",
                    label { "目标网段 CIDR（每行一个）" }
                    HintButton {
                        message,
                        label: "字段说明",
                        title: "目标网段说明".to_string(),
                        summary: "用于按网段统一控制访问边界。".to_string(),
                    }
                    textarea {
                        class: "textarea",
                        value: "{snapshot.cidrs_text}",
                        oninput: move |evt| form.with_mut(|f| f.cidrs_text = evt.value()),
                    }
                }
            }
        }
    }
}

#[component]
fn LogsPage(
    entries: Vec<LogEntry>,
    total_entries: usize,
    running_label: String,
    domain_forwarding_label: String,
    auto_refresh_label: String,
    log_search: Signal<String>,
    log_filter: Signal<LogFilter>,
    message: Signal<Option<UiMessage>>,
) -> Element {
    rsx! {
        div { class: "section",
            div { class: "logs-toolbar",
                div {
                    HintButton {
                        message,
                        label: "日志说明",
                        title: "日志页说明".to_string(),
                        summary: "这里展示最近日志，用于定位认证失败、访问拦截、目标连接失败和会话关闭。".to_string(),
                    }
                    div { class: "logs-meta", "服务：{running_label} ｜ 域名转发：{domain_forwarding_label} ｜ {auto_refresh_label}" }
                }
                div { class: "logs-controls",
                    input {
                        class: "input search-input",
                        placeholder: "搜索用户、客户端、目标、原因",
                        value: "{log_search.read()}",
                        oninput: move |evt| log_search.set(evt.value()),
                    }
                    div { class: "toggle-row",
                        for filter in [
                            LogFilter::All,
                            LogFilter::Access,
                            LogFilter::Auth,
                            LogFilter::Failed,
                            LogFilter::Policy,
                            LogFilter::System,
                        ] {
                            button {
                                class: if *log_filter.read() == filter { "toggle active" } else { "toggle" },
                                onclick: move |_| log_filter.set(filter),
                                "{filter.label()}"
                            }
                        }
                    }
                }
            }
        }
        div { class: "logs",
            if entries.is_empty() {
                div { class: "log-empty", "当前没有可显示的日志内容。" }
            } else {
                div { class: "log-list",
                    div { class: "log-item log-headless", style: "font-weight:600;",
                        div { class: "log-grid",
                            div { "时间 / 类型" }
                            div { "用户 / 客户端" }
                            div { "客户端 IP" }
                            div { "目标" }
                            div { "原因" }
                        }
                    }
                    div { class: "log-item log-headless", style: "font-size:11px;",
                        "显示 {entries.len()} / {total_entries} 条日志"
                    }
                    for entry in entries.iter() {
                        div { class: "log-item",
                            div { class: "log-head",
                                div { class: "log-time", "{entry.timestamp}" }
                                div { class: "log-kind", "{entry.kind_label()}" }
                            }
                            div { class: "log-grid",
                                div { "{entry.timestamp} / {entry.kind_label()}" }
                                div { "{entry.user} / {entry.client_addr}" }
                                div { "{entry.client_ip}" }
                                div { "{entry.target}" }
                                div { "{entry.reason}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[derive(Clone, PartialEq)]
struct ConfigForm {
    listen: String,
    users_text: String,
    access_mode: AccessMode,
    allow_domains: bool,
    ips_text: String,
    cidrs_text: String,
    log_dir: String,
    retention_days: String,
    max_file_size_mb: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
struct LogEntry {
    timestamp: String,
    kind: String,
    user: String,
    client_ip: String,
    client_addr: String,
    target: String,
    reason: String,
}

impl LogEntry {
    fn kind_label(&self) -> &str {
        if self.kind.is_empty() {
            "日志"
        } else {
            &self.kind
        }
    }
}

impl Default for ConfigForm {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:1080".to_string(),
            users_text: String::new(),
            access_mode: AccessMode::Blacklist,
            allow_domains: false,
            ips_text: String::new(),
            cidrs_text: String::new(),
            log_dir: "logs".to_string(),
            retention_days: "7".to_string(),
            max_file_size_mb: "100".to_string(),
        }
    }
}

impl ConfigForm {
    fn load_initial() -> std::result::Result<Self, String> {
        let config =
            AppConfig::ensure_runtime_file(DEFAULT_CONFIG_PATH).map_err(|err| err.to_string())?;
        Ok(Self::from_config(config))
    }

    fn from_config(config: AppConfig) -> Self {
        Self {
            listen: config.listen.to_string(),
            users_text: config
                .auth
                .users
                .into_iter()
                .map(|user| format!("{}={}", user.username, user.password))
                .collect::<Vec<_>>()
                .join("\n"),
            access_mode: config.access.mode,
            allow_domains: config.access.allow_domains,
            ips_text: config
                .access
                .ips
                .into_iter()
                .map(|ip| ip.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            cidrs_text: config
                .access
                .cidrs
                .into_iter()
                .map(|cidr| cidr.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            log_dir: config.log.dir.to_string_lossy().to_string(),
            retention_days: config.log.retention_days.to_string(),
            max_file_size_mb: config.log.max_file_size_mb.to_string(),
        }
    }

    fn save(&self) -> std::result::Result<(), String> {
        let config = self.to_config()?;
        config
            .save_to_file(DEFAULT_CONFIG_PATH)
            .map_err(|err| format!("保存配置失败：{err}"))
    }

    fn to_config(&self) -> std::result::Result<AppConfig, String> {
        let listen = SocketAddr::from_str(self.listen.trim())
            .map_err(|err| format!("监听地址格式错误：{err}"))?;
        let retention_days = self
            .retention_days
            .trim()
            .parse::<u64>()
            .map_err(|err| format!("日志保留天数格式错误：{err}"))?;
        let max_file_size_mb = self
            .max_file_size_mb
            .trim()
            .parse::<u64>()
            .map_err(|err| format!("日志大小上限格式错误：{err}"))?;

        let users = parse_users(&self.users_text)?;
        let ips = parse_lines(&self.ips_text, "目标 IP", std::net::IpAddr::from_str)?;
        let cidrs = parse_lines(&self.cidrs_text, "目标网段", ipnet::IpNet::from_str)?;

        Ok(AppConfig {
            listen,
            auth: AuthConfig { users },
            access: AccessConfig {
                mode: self.access_mode,
                allow_domains: self.allow_domains,
                ips,
                cidrs,
            },
            log: LogConfig {
                dir: PathBuf::from(self.log_dir.trim()),
                retention_days,
                max_file_size_mb,
            },
        })
    }

    fn validate(&self) -> Vec<String> {
        let mut errors = Vec::new();

        if let Err(err) = SocketAddr::from_str(self.listen.trim()) {
            errors.push(format!("监听地址格式错误：{err}"));
        }

        if let Err(err) = self.retention_days.trim().parse::<u64>() {
            errors.push(format!("日志保留天数必须是整数：{err}"));
        }

        if let Err(err) = self.max_file_size_mb.trim().parse::<u64>() {
            errors.push(format!("日志大小上限必须是整数：{err}"));
        }

        if self.log_dir.trim().is_empty() {
            errors.push("日志目录不能为空。".to_string());
        }

        if let Err(err) = parse_users(&self.users_text) {
            errors.push(err);
        }

        if let Err(err) = parse_lines(&self.ips_text, "目标 IP", std::net::IpAddr::from_str) {
            errors.push(err);
        }

        if let Err(err) = parse_lines(&self.cidrs_text, "目标网段", ipnet::IpNet::from_str) {
            errors.push(err);
        }

        if self.access_mode == AccessMode::Whitelist
            && self.ips_text.trim().is_empty()
            && self.cidrs_text.trim().is_empty()
        {
            errors.push("白名单模式至少需要填写一个 IP 或 CIDR。".to_string());
        }

        errors
    }
}

fn export_config(form: &ConfigForm) -> std::result::Result<PathBuf, String> {
    let config = form.to_config()?;
    let exports_dir = PathBuf::from("exports");
    fs::create_dir_all(&exports_dir).map_err(|err| format!("创建导出目录失败：{err}"))?;
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let path = exports_dir.join(format!("config-{timestamp}.toml"));
    config
        .save_to_file(path.to_string_lossy().as_ref())
        .map_err(|err| format!("导出配置失败：{err}"))?;
    Ok(path)
}

fn build_window_icon() -> std::result::Result<Icon, String> {
    let (rgba, width, height) = build_app_icon_rgba();
    Icon::from_rgba(rgba, width as u32, height as u32)
        .map_err(|err| format!("创建窗口图标失败：{err}"))
}

#[derive(Default)]
struct ProxyController {
    runner: Option<ProxyRunner>,
}

struct ProxyRunner {
    stop_tx: watch::Sender<bool>,
    join_handle: thread::JoinHandle<()>,
}

impl ProxyController {
    fn start(&mut self, config_path: String) -> std::result::Result<(), String> {
        if self.runner.is_some() {
            return Err("代理服务已经在运行。".to_string());
        }

        let (stop_tx, stop_rx) = watch::channel(false);
        let join_handle = thread::Builder::new()
            .name("sock5s-ui-runtime".to_string())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create runtime");
                let _ = runtime.block_on(run_server_from_path(&config_path, stop_rx));
            })
            .map_err(|err| format!("启动代理线程失败：{err}"))?;

        self.runner = Some(ProxyRunner {
            stop_tx,
            join_handle,
        });
        Ok(())
    }

    fn stop(&mut self) -> std::result::Result<(), String> {
        let Some(runner) = self.runner.take() else {
            return Err("代理服务当前未运行。".to_string());
        };

        let _ = runner.stop_tx.send(true);
        runner
            .join_handle
            .join()
            .map_err(|_| "代理线程异常退出。".to_string())
    }

    fn is_running(&self) -> bool {
        self.runner.is_some()
    }
}

fn controller() -> &'static Arc<Mutex<ProxyController>> {
    static CONTROLLER: OnceLock<Arc<Mutex<ProxyController>>> = OnceLock::new();
    CONTROLLER.get_or_init(|| Arc::new(Mutex::new(ProxyController::default())))
}

fn parse_users(input: &str) -> std::result::Result<Vec<UserEntry>, String> {
    let mut users = Vec::new();

    for (index, line) in input.lines().enumerate() {
        let entry = line.trim();
        if entry.is_empty() {
            continue;
        }

        let pair = entry
            .split_once('=')
            .or_else(|| entry.split_once(':'))
            .ok_or_else(|| {
                format!(
                    "第 {} 行账号格式错误，请使用 `用户名=密码`。",
                    index + 1
                )
            })?;

        let username = pair.0.trim();
        let password = pair.1.trim();
        if username.is_empty() || password.is_empty() {
            return Err(format!(
                "第 {} 行账号格式错误，用户名和密码都不能为空。",
                index + 1
            ));
        }

        users.push(UserEntry {
            username: username.to_string(),
            password: password.to_string(),
        });
    }

    Ok(users)
}

fn parse_lines<T, F>(input: &str, label: &str, parser: F) -> std::result::Result<Vec<T>, String>
where
    F: Fn(&str) -> std::result::Result<T, <T as FromStr>::Err>,
    T: FromStr,
    <T as FromStr>::Err: std::fmt::Display,
{
    let mut values = Vec::new();

    for (index, line) in input.lines().enumerate() {
        let entry = line.trim();
        if entry.is_empty() {
            continue;
        }

        let value = parser(entry).map_err(|err| {
            format!("{label} 第 {} 行格式错误：{entry}（{err}）", index + 1)
        })?;
        values.push(value);
    }

    Ok(values)
}

fn read_recent_logs(dir: &Path) -> std::result::Result<Vec<LogEntry>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(dir)
        .map_err(|err| format!("读取日志目录失败：{err}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().extension().and_then(|ext| ext.to_str()) == Some("log"))
        .collect::<Vec<_>>();

    entries.sort_by_key(|entry| entry.metadata().and_then(|meta| meta.modified()).ok());
    entries.reverse();

    let mut merged = Vec::new();
    for entry in entries.into_iter().take(3).rev() {
        let path = entry.path();
        let content = fs::read_to_string(&path)
            .map_err(|err| format!("读取 {} 失败：{err}", path.display()))?;
        merged.extend(content.lines().filter_map(parse_log_line));
    }

    let start = merged.len().saturating_sub(220);
    Ok(merged[start..].to_vec())
}

fn parse_log_line(line: &str) -> Option<LogEntry> {
    let line = line.trim();
    if line.len() < 19 {
        return None;
    }

    let timestamp = line[..19].to_string();
    let mut entry = LogEntry {
        timestamp,
        kind: "日志".to_string(),
        user: "-".to_string(),
        client_ip: "-".to_string(),
        client_addr: "-".to_string(),
        target: "-".to_string(),
        reason: String::new(),
    };

    for part in line[20..].split_whitespace() {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "protocol" => entry.kind = format!("访问/{value}"),
                "event" => entry.kind = value.to_string(),
                "user" => entry.user = value.to_string(),
                "client_ip" => entry.client_ip = value.to_string(),
                "client_addr" => entry.client_addr = value.to_string(),
                "target" => entry.target = value.to_string(),
                "reason" => entry.reason = value.to_string(),
                _ => {}
            }
        }
    }

    Some(entry)
}

fn now_display() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn filter_logs(entries: &[LogEntry], filter: LogFilter, keyword: &str) -> Vec<LogEntry> {
    let keyword = keyword.trim().to_ascii_lowercase();

    entries
        .iter()
        .filter(|entry| matches_filter(entry, filter))
        .filter(|entry| {
            if keyword.is_empty() {
                return true;
            }

            let haystack = format!(
                "{} {} {} {} {} {}",
                entry.timestamp, entry.kind, entry.user, entry.client_ip, entry.client_addr, entry.target
            )
            .to_ascii_lowercase();
            let reason = entry.reason.to_ascii_lowercase();

            haystack.contains(&keyword) || reason.contains(&keyword)
        })
        .cloned()
        .collect()
}

fn matches_filter(entry: &LogEntry, filter: LogFilter) -> bool {
    match filter {
        LogFilter::All => true,
        LogFilter::Access => entry.kind.starts_with("访问/"),
        LogFilter::Auth => entry.kind == "auth_succeeded" || entry.kind == "auth_failed",
        LogFilter::Failed => entry.kind == "connect_failed" || entry.kind == "session_error",
        LogFilter::Policy => entry.kind == "access_denied",
        LogFilter::System => entry.kind == "server_started"
            || entry.kind == "server_stopped"
            || entry.kind == "connection_closed",
    }
}

#[derive(Clone)]
struct TrayHandles {
    _tray: trayicon::TrayIcon,
    show_item: MenuItem,
    exit_item: MenuItem,
}

impl TrayHandles {
    fn new() -> Self {
        let menu = Menu::new();
        let show_item = MenuItem::new("显示主窗口", true, None);
        let exit_item = MenuItem::new("退出程序", true, None);
        let _ = menu.append(&show_item);
        let _ = menu.append(&exit_item);

        let icon = build_tray_icon().ok();
        let tray = trayicon::init_tray_icon(menu, icon);

        Self {
            _tray: tray,
            show_item,
            exit_item,
        }
    }
}

fn init_tray_handles() -> TrayHandles {
    TrayHandles::new()
}

fn restore_window(window: &dioxus_desktop::DesktopContext) {
    window.set_minimized(false);
    window.set_visible(true);
    window.set_focus();
}

fn graceful_exit() -> ! {
    let _ = controller().lock().unwrap().stop();
    std::process::exit(0);
}

#[derive(Clone, Copy)]
enum CloseAction {
    Exit,
    Tray,
}

#[cfg(target_os = "windows")]
fn prompt_close_action() -> CloseAction {
    use std::ffi::c_void;

    const MB_ICONQUESTION: u32 = 0x0000_0020;
    const MB_YESNO: u32 = 0x0000_0004;
    const MB_DEFBUTTON2: u32 = 0x0000_0100;
    const IDYES: i32 = 6;

    unsafe extern "system" {
        fn MessageBoxW(
            hwnd: *mut c_void,
            lp_text: *const u16,
            lp_caption: *const u16,
            u_type: u32,
        ) -> i32;
    }

    let text = wide("是否退出程序？\n\n选择“是”立即退出。\n选择“否”缩到托盘，程序继续后台运行。");
    let caption = wide("sock5s 控制中心");
    let result = unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            MB_ICONQUESTION | MB_YESNO | MB_DEFBUTTON2,
        )
    };

    if result == IDYES {
        CloseAction::Exit
    } else {
        CloseAction::Tray
    }
}

#[cfg(not(target_os = "windows"))]
fn prompt_close_action() -> CloseAction {
    CloseAction::Tray
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn build_tray_icon() -> std::result::Result<trayicon::Icon, String> {
    let (rgba, width, height) = build_app_icon_rgba();
    trayicon::Icon::from_rgba(rgba, width, height)
        .map_err(|err| format!("创建托盘图标失败：{err}"))
}

fn build_app_icon_rgba() -> (Vec<u8>, u32, u32) {
    let width = 64usize;
    let height = 64usize;
    let mut rgba = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        for x in 0..width {
            let dx = x as f32 - 32.0;
            let dy = y as f32 - 32.0;
            let distance = (dx * dx + dy * dy).sqrt();
            let (r, g, b, a) = if distance < 28.0 {
                let top_mix = (y as f32 / height as f32).clamp(0.0, 1.0);
                let r = (15.0 + 12.0 * top_mix) as u8;
                let g = (106.0 + 40.0 * top_mix) as u8;
                let b = (216.0 - 48.0 * top_mix) as u8;
                (r, g, b, 255)
            } else {
                (0, 0, 0, 0)
            };

            let ring = distance > 14.0 && distance < 17.0;
            let accent = x > 21 && x < 43 && y > 28 && y < 34;
            let node = (x > 18 && x < 26 && y > 18 && y < 26)
                || (x > 38 && x < 46 && y > 18 && y < 26)
                || (x > 28 && x < 36 && y > 38 && y < 46);

            if ring || accent || node {
                rgba.extend_from_slice(&[244, 248, 255, 255]);
            } else {
                rgba.extend_from_slice(&[r, g, b, a]);
            }
        }
    }

    (rgba, width as u32, height as u32)
}
