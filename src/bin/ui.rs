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
use dioxus_desktop::tao::window::Icon;
use dioxus_desktop::{Config as DesktopConfig, WindowBuilder};
use serde::{Deserialize, Serialize};
use sock5s::config::{AccessConfig, AccessMode, AppConfig, AuthConfig, LogConfig, UserEntry};
use sock5s::run_server_from_path;
use tokio::sync::watch;

const APP_CSS: &str = r#"
body {
    margin: 0;
    font-family: "Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", sans-serif;
    background: #e9edf2;
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
    border: 1px solid #cfd6df;
    border-radius: 0;
    box-shadow: none;
}

.toolbar {
    padding: 10px 14px;
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
}

.toolbar-title {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: #1b2a3a;
}

.toolbar-note {
    margin: 2px 0 0;
    color: #667281;
    font-size: 12px;
}

.toolbar-status {
    display: flex;
    gap: 8px;
    align-items: center;
    flex-wrap: wrap;
}

.status-chip {
    border: 1px solid #cfd6df;
    background: #f4f6f8;
    color: #32475b;
    border-radius: 999px;
    padding: 4px 10px;
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
    padding: 10px;
    display: grid;
    gap: 8px;
    position: sticky;
    top: 10px;
    background: #f5f7fa;
}

.nav-title {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    color: #304254;
    text-transform: uppercase;
}

.nav-note {
    margin: 0;
    color: #6b7785;
    font-size: 11px;
    line-height: 1.45;
}

.nav-list {
    display: grid;
    gap: 6px;
}

.nav-item {
    border: 1px solid #cfd6df;
    background: #ffffff;
    color: #334155;
    border-radius: 0;
    padding: 10px 10px;
    display: grid;
    gap: 3px;
    cursor: pointer;
    text-align: left;
}

.nav-item.active {
    background: #dcecff;
    border-color: #74a7e7;
    color: #0d4f9c;
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
    border-top: 1px solid #e2e8f0;
    padding-top: 10px;
    display: grid;
    gap: 5px;
}

.nav-footer span {
    font-size: 12px;
    color: #607082;
}

.panel {
    padding: 12px;
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
    font-size: 18px;
    font-weight: 600;
}

.panel-note {
    margin: 0;
    color: #6b7785;
    line-height: 1.5;
    font-size: 12px;
}

.panel-side {
    min-width: 260px;
    display: flex;
    gap: 8px;
}

.mini-card {
    border: 1px solid #cfd6df;
    border-radius: 0;
    padding: 8px 10px;
    background: #f8fafb;
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
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    padding: 0 0 10px;
    border-bottom: 1px solid #d9e0e7;
}

.action {
    border: 1px solid #b8c2cf;
    border-radius: 0;
    padding: 7px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    background: linear-gradient(180deg, #ffffff, #edf1f5);
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

.message {
    margin: 10px 0 0;
    min-height: 1.5em;
    font-size: 12px;
    color: #21548c;
}

.validation {
    margin: 14px 0 0;
    padding: 12px 14px;
    border-radius: 8px;
    background: #fff7e8;
    border: 1px solid #eed3a8;
    color: #8a581b;
    font-size: 13px;
    line-height: 1.6;
}

.validation strong {
    display: block;
    margin-bottom: 6px;
}

.content-stack {
    display: grid;
    gap: 10px;
    margin-top: 12px;
}

.overview-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
}

.overview-card {
    border: 1px solid #cfd6df;
    border-radius: 0;
    padding: 12px;
    background: #f8fafb;
    display: grid;
    gap: 6px;
}

.overview-card label {
    color: #607082;
    font-size: 12px;
}

.overview-card strong {
    font-size: 18px;
    color: #24364d;
}

.overview-card span {
    color: #5f6b7a;
    font-size: 13px;
    line-height: 1.55;
}

.section {
    border: 1px solid #cfd6df;
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

.hint {
    margin: 0;
    color: #6b7280;
    font-size: 12px;
    line-height: 1.55;
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

.logs {
    min-height: 620px;
    border-radius: 0;
    border: 1px solid #cfd6df;
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
    .workspace, .panel-header {
        grid-template-columns: 1fr;
        display: grid;
    }

    .panel-side {
        min-width: 0;
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}

@media (max-width: 900px) {
    .overview-grid, .grid, .log-grid, .panel-side, .toolbar {
        grid-template-columns: 1fr;
    }
}
"#;

fn main() {
    let mut config = DesktopConfig::new().with_window(
        WindowBuilder::new()
            .with_title("sock5s 控制中心")
            .with_resizable(true),
    );
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
            Self::Basic => "单独维护监听地址、配置文件路径、日志目录和日志保留策略。",
            Self::Access => "单独维护代理账号、白名单或黑名单策略，以及域名转发开关。",
            Self::Logs => "单独查看最近日志、刷新状态和运行过程中的失败原因。",
        }
    }
}

#[component]
fn App() -> Element {
    let initial = ConfigForm::load_initial();
    let initial_log_dir = initial.log_dir.clone();
    let form = use_signal(|| initial);
    let mut page = use_signal(|| UiPage::Overview);
    let status = use_signal(|| "未启动".to_string());
    let message = use_signal(String::new);
    let auto_refresh_logs = use_signal(|| true);
    let mut logs = use_signal(|| read_recent_logs(Path::new(&initial_log_dir)).unwrap_or_default());

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
    let has_validation_errors = !validation_errors.is_empty();
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
    let page_title = page.read().title();
    let page_note = page.read().note();

    rsx! {
        style { "{APP_CSS}" }
        div { class: "shell",
            div { class: "frame",
                div { class: "toolbar",
                    div {
                        h1 { class: "toolbar-title", "sock5s 控制中心" }
                        p { class: "toolbar-note",
                            "桌面管理界面，按模块拆分显示。"
                        }
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
                        onchange: move |next| page.set(next),
                    }

                    div { class: "panel",
                        div { class: "panel-header",
                            div {
                                h2 { class: "panel-title", "{page_title}" }
                                p { class: "panel-note", "{page_note}" }
                            }
                            div { class: "panel-side",
                                div { class: "mini-card",
                                    label { "当前配置文件" }
                                    strong { "{snapshot.config_path}" }
                                }
                                div { class: "mini-card",
                                    label { "日志目录" }
                                    strong { "{snapshot.log_dir}" }
                                }
                            }
                        }

                        ActionBar {
                            form,
                            logs,
                            status,
                            message,
                            auto_refresh_logs,
                            has_validation_errors,
                        }

                        if has_validation_errors {
                            div { class: "validation",
                                strong { "配置校验未通过" }
                                for err in validation_errors.iter() {
                                    div { "{err}" }
                                }
                            }
                        }

                        p { class: "message", "{message.read()}" }

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
                                }
                            }

                            if *page.read() == UiPage::Basic {
                                BasicSettingsPage { form, snapshot: snapshot.clone() }
                            }

                            if *page.read() == UiPage::Access {
                                AccessSettingsPage { form, snapshot: snapshot.clone(), mode_label: mode_label.to_string() }
                            }

                            if *page.read() == UiPage::Logs {
                                LogsPage {
                                    entries: logs.read().clone(),
                                    running_label: running_label.to_string(),
                                    domain_forwarding_label: domain_forwarding_label.to_string(),
                                    auto_refresh_label: auto_refresh_label.to_string(),
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
    onchange: EventHandler<UiPage>,
) -> Element {
    rsx! {
        aside { class: "nav",
            div {
                h3 { class: "nav-title", "模块导航" }
                p { class: "nav-note", "按模块切换配置页。" }
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
fn ActionBar(
    form: Signal<ConfigForm>,
    logs: Signal<Vec<LogEntry>>,
    status: Signal<String>,
    message: Signal<String>,
    auto_refresh_logs: Signal<bool>,
    has_validation_errors: bool,
) -> Element {
    rsx! {
        div { class: "button-row",
            button {
                class: "action secondary",
                onclick: move |_| {
                    let path = form.read().config_path.clone();
                    match ConfigForm::load_from_path(Path::new(&path)) {
                        Ok(next) => {
                            let log_dir = next.log_dir.clone();
                            form.set(next);
                            logs.set(read_recent_logs(Path::new(&log_dir)).unwrap_or_default());
                            let _ = save_ui_state(&UiState { config_path: path.clone() });
                            message.set(format!("已从 {path} 重新加载配置"));
                        }
                        Err(err) => message.set(err),
                    }
                },
                "重新加载"
            }
            button {
                class: "action secondary",
                onclick: move |_| {
                    let current = form.read().clone();
                    match current.save() {
                        Ok(_) => {
                            let _ = save_ui_state(&UiState { config_path: current.config_path.clone() });
                            message.set(format!("配置已保存到 {}", current.config_path));
                        }
                        Err(err) => message.set(err),
                    }
                },
                "保存配置"
            }
            button {
                class: "action secondary",
                disabled: has_validation_errors,
                onclick: move |_| {
                    let current = form.read().clone();
                    match export_config(&current) {
                        Ok(path) => message.set(format!("已导出配置快照：{}", path.display())),
                        Err(err) => message.set(err),
                    }
                },
                "导出快照"
            }
            button {
                class: "action primary",
                disabled: has_validation_errors,
                onclick: move |_| {
                    let current = form.read().clone();
                    match current.save().and_then(|_| {
                        let mut guard = controller().lock().unwrap();
                        guard.start(current.config_path.clone())
                    }) {
                        Ok(_) => {
                            let _ = save_ui_state(&UiState { config_path: current.config_path.clone() });
                            status.set(format!("运行中 · {}", current.listen));
                            logs.set(read_recent_logs(Path::new(&current.log_dir)).unwrap_or_default());
                            message.set("代理服务已启动".to_string());
                        }
                        Err(err) => message.set(err),
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
                            message.set("代理服务已停止".to_string());
                        }
                        Err(err) => message.set(err),
                    }
                },
                "停止代理"
            }
            button {
                class: "action warn",
                onclick: move |_| {
                    let log_dir = form.read().log_dir.clone();
                    logs.set(read_recent_logs(Path::new(&log_dir)).unwrap_or_default());
                    message.set("日志已刷新".to_string());
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

#[component]
fn OverviewPage(
    snapshot: ConfigForm,
    running_label: String,
    mode_label: String,
    auth_label: String,
    domain_forwarding_label: String,
    auto_refresh_label: String,
    log_count: usize,
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
            h3 { class: "section-title", "当前配置摘要" }
            p { class: "hint", "这里仅用于快速确认当前生效配置，详细编辑请进入左侧对应模块。" }
            div { class: "grid",
                div { class: "field",
                    label { "配置文件路径" }
                    input { class: "input", value: "{snapshot.config_path}", readonly: true }
                }
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
            }
        }
    }
}

#[component]
fn BasicSettingsPage(form: Signal<ConfigForm>, snapshot: ConfigForm) -> Element {
    rsx! {
        div { class: "section",
            h3 { class: "section-title", "基础设置" }
            p { class: "hint", "基础参数拆到单独页面，便于集中调整监听地址、日志策略和配置文件位置。" }
            div { class: "grid",
                div { class: "wide",
                    label { "配置文件路径" }
                    input {
                        class: "input",
                        value: "{snapshot.config_path}",
                        oninput: move |evt| form.with_mut(|f| f.config_path = evt.value()),
                    }
                }
                div { class: "field",
                    label { "监听地址" }
                    input {
                        class: "input",
                        value: "{snapshot.listen}",
                        oninput: move |evt| form.with_mut(|f| f.listen = evt.value()),
                    }
                }
                div { class: "field",
                    label { "日志目录" }
                    input {
                        class: "input",
                        value: "{snapshot.log_dir}",
                        oninput: move |evt| form.with_mut(|f| f.log_dir = evt.value()),
                    }
                }
                div { class: "field",
                    label { "日志保留天数" }
                    input {
                        class: "input",
                        value: "{snapshot.retention_days}",
                        oninput: move |evt| form.with_mut(|f| f.retention_days = evt.value()),
                    }
                }
                div { class: "field",
                    label { "单文件大小上限（MB）" }
                    input {
                        class: "input",
                        value: "{snapshot.max_file_size_mb}",
                        oninput: move |evt| form.with_mut(|f| f.max_file_size_mb = evt.value()),
                    }
                }
            }
        }
    }
}

#[component]
fn AccessSettingsPage(form: Signal<ConfigForm>, snapshot: ConfigForm, mode_label: String) -> Element {
    let domain_toggle_label = if snapshot.allow_domains { "已开启" } else { "已关闭" };

    rsx! {
        div { class: "section",
            h3 { class: "section-title", "账号与访问控制" }
            p { class: "hint", "账号、白名单或黑名单、域名转发策略拆成独立页面，避免和日志设置混在一起。" }
            div { class: "grid",
                div { class: "wide",
                    label { "代理账号（每行一个，格式：用户名=密码）" }
                    textarea {
                        class: "textarea",
                        value: "{snapshot.users_text}",
                        oninput: move |evt| form.with_mut(|f| f.users_text = evt.value()),
                    }
                }
                div { class: "field",
                    label { "访问策略" }
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
                    textarea {
                        class: "textarea",
                        value: "{snapshot.ips_text}",
                        oninput: move |evt| form.with_mut(|f| f.ips_text = evt.value()),
                    }
                }
                div { class: "wide",
                    label { "目标网段 CIDR（每行一个）" }
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
    running_label: String,
    domain_forwarding_label: String,
    auto_refresh_label: String,
) -> Element {
    rsx! {
        div { class: "section",
            div { class: "logs-toolbar",
                p { class: "hint", "这里只展示最近日志，重点用于排查认证失败、访问拦截、目标连接失败和会话关闭。" }
                div { class: "logs-meta", "服务：{running_label} ｜ 域名转发：{domain_forwarding_label} ｜ {auto_refresh_label}" }
            }
        }
        div { class: "logs",
            if entries.is_empty() {
                div { class: "log-empty", "当前没有可显示的日志内容。" }
            } else {
                div { class: "log-list",
                    div { class: "log-item", style: "background:#f3f6f9; font-weight:600;",
                        div { class: "log-grid",
                            div { "时间 / 类型" }
                            div { "用户 / 客户端" }
                            div { "客户端 IP" }
                            div { "目标" }
                            div { "原因" }
                        }
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
    config_path: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct UiState {
    config_path: String,
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
            config_path: "config.toml".to_string(),
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
    fn load_initial() -> Self {
        let config_path = load_ui_state()
            .map(|state| PathBuf::from(state.config_path))
            .filter(|path| path.exists())
            .unwrap_or_else(|| PathBuf::from("config.toml"));
        if config_path.exists() {
            return Self::load_from_path(&config_path).unwrap_or_default();
        }

        let example_path = PathBuf::from("config.toml.example");
        if example_path.exists() {
            return Self::load_from_path(&example_path)
                .map(|mut form| {
                    form.config_path = "config.toml".to_string();
                    form
                })
                .unwrap_or_default();
        }

        Self::default()
    }

    fn load_from_path(path: &Path) -> std::result::Result<Self, String> {
        let path_str = path.to_string_lossy().to_string();
        let config = AppConfig::from_file(&path_str).map_err(|err| err.to_string())?;
        Ok(Self::from_config(path_str, config))
    }

    fn from_config(config_path: String, config: AppConfig) -> Self {
        Self {
            config_path,
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
            .save_to_file(&self.config_path)
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
    let width = 64;
    let height = 64;
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

fn load_ui_state() -> Option<UiState> {
    let path = PathBuf::from("ui-state.toml");
    let content = fs::read_to_string(path).ok()?;
    toml::from_str(&content).ok()
}

fn save_ui_state(state: &UiState) -> std::result::Result<(), String> {
    let content = toml::to_string_pretty(state).map_err(|err| format!("保存界面状态失败：{err}"))?;
    fs::write("ui-state.toml", content).map_err(|err| format!("写入界面状态失败：{err}"))
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
