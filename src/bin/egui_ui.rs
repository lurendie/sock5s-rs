#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use eframe::egui::{
    self, CentralPanel, Color32, ComboBox, Context, FontData, FontDefinitions, FontFamily,
    RichText, ScrollArea, SidePanel, TextEdit, Ui,
};
use eframe::{App, Frame, NativeOptions};
use sock5s::config::{
    AccessConfig, AccessMode, AppConfig, AuthConfig, LogConfig, UserEntry, DEFAULT_CONFIG_PATH,
};
use sock5s::run_server_from_path;
use tokio::sync::watch;
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem},
    MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
};

const APP_TITLE: &str = "sock5s 控制台 · egui";

fn main() -> eframe::Result<()> {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_TITLE)
            .with_inner_size([1160.0, 760.0])
            .with_resizable(true)
            .with_min_inner_size([1024.0, 680.0]),
        ..Default::default()
    };

    eframe::run_native(
        APP_TITLE,
        options,
        Box::new(|cc| {
            configure_fonts(&cc.egui_ctx);
            configure_theme(&cc.egui_ctx);
            Ok(Box::new(EguiConsoleApp::new(cc.egui_ctx.clone())))
        }),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Basic,
    Access,
    Logs,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "概览",
            Self::Basic => "基础设置",
            Self::Access => "访问控制",
            Self::Logs => "日志",
        }
    }
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

#[derive(Clone)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum MessageKind {
    Success,
    Info,
    Error,
}

#[derive(Clone)]
struct MessageDialog {
    kind: MessageKind,
    title: String,
    summary: String,
    detail: Option<String>,
    created_at: Instant,
    auto_close_after: Option<Duration>,
}

struct TrayHandles {
    _tray: TrayIcon,
    _show_item: MenuItem,
    _exit_item: MenuItem,
}

impl TrayHandles {
    fn new(ctx: &Context) -> Self {
        let menu = Menu::new();
        let show_item = MenuItem::new("显示主窗口", true, None);
        let exit_item = MenuItem::new("退出程序", true, None);
        let _ = menu.append(&show_item);
        let _ = menu.append(&exit_item);

        let show_id = show_item.id().clone();
        let exit_id = exit_item.id().clone();
        let menu_ctx = ctx.clone();
        let tray_ctx = ctx.clone();

        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            if event.id == show_id {
                restore_window(&menu_ctx);
                menu_ctx.request_repaint();
            } else if event.id == exit_id {
                force_exit();
            }
        }));

        TrayIconEvent::set_event_handler(Some(move |event| match event {
            TrayIconEvent::DoubleClick { .. } => {
                restore_window(&tray_ctx);
                tray_ctx.request_repaint();
            }
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                restore_window(&tray_ctx);
                tray_ctx.request_repaint();
            }
            _ => {}
        }));

        let mut tray_builder = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_menu_on_left_click(false);
        if let Ok(icon) = build_tray_icon() {
            tray_builder = tray_builder.with_icon(icon);
        }
        let tray = tray_builder
            .build()
            .expect("failed to create tray icon");

        Self {
            _tray: tray,
            _show_item: show_item,
            _exit_item: exit_item,
        }
    }
}

struct EguiConsoleApp {
    page: Page,
    form: ConfigForm,
    baseline_form: ConfigForm,
    status: String,
    message_dialog: Option<MessageDialog>,
    config_loaded_at: String,
    auto_refresh_logs: bool,
    logs: Vec<LogEntry>,
    log_search: String,
    log_filter: LogFilter,
    last_log_refresh: Instant,
    logs_clear_cutoff: Option<String>,
    _tray: TrayHandles,
    close_prompt_open: bool,
    allow_close: bool,
}

impl EguiConsoleApp {
    fn new(ctx: Context) -> Self {
        let form = ConfigForm::load_initial().unwrap_or_default();
        let logs = Vec::new();
        let startup_cutoff = now_display();
        Self {
            page: Page::Overview,
            baseline_form: form.clone(),
            form,
            status: "未启动".to_string(),
            message_dialog: None,
            config_loaded_at: now_display(),
            auto_refresh_logs: true,
            logs,
            log_search: String::new(),
            log_filter: LogFilter::All,
            last_log_refresh: Instant::now(),
            logs_clear_cutoff: Some(startup_cutoff),
            _tray: TrayHandles::new(&ctx),
            close_prompt_open: false,
            allow_close: false,
        }
    }

    fn refresh_logs_if_needed(&mut self, ctx: &Context) {
        ctx.request_repaint_after(Duration::from_secs(2));
        if !self.auto_refresh_logs || self.last_log_refresh.elapsed() < Duration::from_secs(2) {
            return;
        }
        self.refresh_logs();
    }

    fn refresh_logs(&mut self) {
        self.logs = read_recent_logs(Path::new(&self.form.log_dir)).unwrap_or_default();
        self.last_log_refresh = Instant::now();
    }

    fn load_config(&mut self) {
        match ConfigForm::load_initial() {
            Ok(next) => {
                self.baseline_form = next.clone();
                self.form = next;
                self.config_loaded_at = now_display();
                self.refresh_logs();
                self.show_success("配置已重新加载", format!("已重新加载配置：{}", DEFAULT_CONFIG_PATH));
            }
            Err(err) => self.show_error("加载配置失败", err),
        }
    }

    fn save_config(&mut self) {
        match self.form.save() {
            Ok(_) => {
                self.baseline_form = self.form.clone();
                self.config_loaded_at = now_display();
                self.show_success("配置已保存", format!("配置已保存到 {}", DEFAULT_CONFIG_PATH));
            }
            Err(err) => self.show_error("保存配置失败", err),
        }
    }

    fn export_snapshot(&mut self) {
        match export_config(&self.form) {
            Ok(path) => self.show_success("快照已导出", format!("已导出配置快照：{}", path.display())),
            Err(err) => self.show_error("导出快照失败", err),
        }
    }

    fn start_proxy(&mut self) {
        match self.form.save().and_then(|_| {
            let mut guard = controller().lock().unwrap();
            guard.start(DEFAULT_CONFIG_PATH.to_string())
        }) {
            Ok(_) => {
                self.baseline_form = self.form.clone();
                self.status = format!("运行中 · {}", self.form.listen);
                self.refresh_logs();
                self.show_success("代理已启动", "代理服务已启动".to_string());
            }
            Err(err) => self.show_error("启动代理失败", err),
        }
    }

    fn stop_proxy(&mut self) {
        match controller().lock().unwrap().stop() {
            Ok(_) => {
                self.status = "未启动".to_string();
                self.refresh_logs();
                self.show_success("代理已停止", "代理服务已停止".to_string());
            }
            Err(err) => self.show_error("停止代理失败", err),
        }
    }

    fn filtered_logs(&self) -> Vec<LogEntry> {
        let base = if let Some(cutoff) = &self.logs_clear_cutoff {
            self.logs
                .iter()
                .filter(|entry| !entry.timestamp.is_empty() && entry.timestamp > *cutoff)
                .cloned()
                .collect::<Vec<_>>()
        } else {
            self.logs.clone()
        };
        filter_logs(&base, self.log_filter, &self.log_search)
    }

    fn validation_errors(&self) -> Vec<String> {
        self.form.validate()
    }

    fn diff_fields(&self) -> Vec<&'static str> {
        diff_fields(&self.baseline_form, &self.form)
    }

    fn export_current_logs(&mut self) {
        match export_logs(&self.filtered_logs()) {
            Ok(path) => self.show_success("日志已导出", format!("已导出当前筛选日志：{}", path.display())),
            Err(err) => self.show_error("导出日志失败", err),
        }
    }

    fn show_success(&mut self, title: impl Into<String>, summary: impl Into<String>) {
        self.message_dialog = Some(MessageDialog {
            kind: MessageKind::Success,
            title: title.into(),
            summary: summary.into(),
            detail: None,
            created_at: Instant::now(),
            auto_close_after: Some(Duration::from_secs(2)),
        });
    }

    fn show_error(&mut self, title: impl Into<String>, detail: impl Into<String>) {
        let detail = detail.into();
        let summary = detail.lines().next().unwrap_or("发生错误").to_string();
        self.message_dialog = Some(MessageDialog {
            kind: MessageKind::Error,
            title: title.into(),
            summary,
            detail: Some(detail),
            created_at: Instant::now(),
            auto_close_after: None,
        });
    }

    fn show_info(&mut self, title: impl Into<String>, summary: impl Into<String>, detail: Option<String>) {
        self.message_dialog = Some(MessageDialog {
            kind: MessageKind::Info,
            title: title.into(),
            summary: summary.into(),
            detail,
            created_at: Instant::now(),
            auto_close_after: None,
        });
    }

    fn show_validation_error(&mut self, errors: &[String]) {
        self.show_error("配置校验未通过", errors.join("\n"));
    }

    fn clear_log_view(&mut self) {
        self.logs_clear_cutoff = Some(now_display());
    }
}

impl App for EguiConsoleApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        self.refresh_logs_if_needed(ctx);

        if let Some(dialog) = &self.message_dialog {
            if let Some(timeout) = dialog.auto_close_after {
                if dialog.created_at.elapsed() >= timeout {
                    self.message_dialog = None;
                } else {
                    ctx.request_repaint_after(timeout - dialog.created_at.elapsed());
                }
            }
        }

        if ctx.input(|input| input.viewport().close_requested()) && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.close_prompt_open = true;
        }

        let running = controller().lock().unwrap().is_running();
        let mode_label = match self.form.access_mode {
            AccessMode::Blacklist => "黑名单",
            AccessMode::Whitelist => "白名单",
        };
        let auth_label = if self.form.users_text.trim().is_empty() {
            "无认证"
        } else {
            "用户名密码"
        };
        let domain_label = if self.form.allow_domains { "允许" } else { "禁止" };
        let validation_errors = self.validation_errors();
        let diff_fields = self.diff_fields();
        let failure_summary = recent_failures(&self.logs);

        SidePanel::left("navigation")
            .exact_width(172.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("sock5s");
                    if info_button(ui, "说明") {
                        self.show_info(
                            "控制台说明",
                            "当前界面用于管理固定配置文件 config.toml。",
                            Some("保存、启动、导出等动作都会使用运行目录下的 config.toml。".to_string()),
                        );
                    }
                });
                ui.add_space(10.0);

                nav_button(ui, &mut self.page, Page::Overview, "概览");
                nav_button(ui, &mut self.page, Page::Basic, "基础设置");
                nav_button(ui, &mut self.page, Page::Access, "访问控制");
                nav_button(ui, &mut self.page, Page::Logs, "日志");

                ui.add_space(16.0);
                ui.separator();
                sidebar_meta(ui, "服务", &self.status);
                sidebar_meta(ui, "策略", mode_label);
                sidebar_meta(ui, "域名", domain_label);
                sidebar_meta(ui, "变更", &format!("{} 项", diff_fields.len()));
            });

        CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                page_title(ui, self.page.title(), &self.status);

                action_toolbar(
                    ui,
                    self,
                    &validation_errors,
                );

                ui.add_space(8.0);

                match self.page {
                    Page::Overview => overview_page(ui, self, running, auth_label, mode_label, domain_label, &diff_fields, &failure_summary),
                    Page::Basic => basic_page(ui, &mut self.form),
                    Page::Access => access_page(ui, &mut self.form, mode_label),
                    Page::Logs => logs_page(ui, self, running, domain_label),
                }
            });
        });

        if let Some(dialog) = self.message_dialog.clone() {
            let (title_color, fill_color, stroke_color, button_text) = match dialog.kind {
                MessageKind::Success => (
                    Color32::from_rgb(33, 94, 52),
                    Color32::from_rgb(241, 249, 243),
                    Color32::from_rgb(182, 216, 190),
                    "确定",
                ),
                MessageKind::Info => (
                    Color32::from_rgb(44, 78, 118),
                    Color32::from_rgb(243, 247, 252),
                    Color32::from_rgb(191, 207, 226),
                    "关闭",
                ),
                MessageKind::Error => (
                    Color32::from_rgb(145, 47, 47),
                    Color32::from_rgb(253, 244, 244),
                    Color32::from_rgb(232, 193, 193),
                    "关闭",
                ),
            };

            egui::Window::new(dialog.title.clone())
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(fill_color)
                        .stroke(egui::Stroke::new(1.0, stroke_color))
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_min_width(420.0);
                            ui.label(RichText::new(&dialog.summary).strong().color(title_color));

                            if dialog.kind == MessageKind::Success {
                                if let Some(timeout) = dialog.auto_close_after {
                                    let remain = timeout.saturating_sub(dialog.created_at.elapsed()).as_secs_f32();
                                    ui.add_space(6.0);
                                    ui.label(
                                        RichText::new(format!("将在 {:.1} 秒后自动关闭", remain.max(0.0)))
                                            .size(12.0)
                                            .weak(),
                                    );
                                }
                            } else if let Some(detail) = &dialog.detail {
                                ui.add_space(8.0);
                                ui.collapsing("详情", |ui| {
                                    let mut detail_text = detail.clone();
                                    ui.add(
                                        TextEdit::multiline(&mut detail_text)
                                            .desired_width(f32::INFINITY)
                                            .desired_rows(6)
                                            .interactive(false),
                                    );
                                });
                            }
                        });

                    ui.add_space(10.0);
                    if ui.button(button_text).clicked() {
                        self.message_dialog = None;
                    }
                });
        }

        if self.close_prompt_open {
            egui::Window::new("退出确认")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(Color32::from_rgb(243, 247, 252))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(191, 207, 226)))
                        .corner_radius(8.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.set_min_width(420.0);
                            ui.label(
                                RichText::new("是否退出程序？")
                                    .strong()
                                    .color(Color32::from_rgb(44, 78, 118)),
                            );
                            ui.add_space(6.0);
                            ui.label("选择“退出程序”会直接结束。");
                            ui.label("选择“缩到托盘”会隐藏主窗口，程序继续后台运行。");
                        });

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        let exit_btn = egui::Button::new(
                            RichText::new("退出程序").color(Color32::from_rgb(140, 34, 34)),
                        )
                        .fill(Color32::from_rgb(255, 240, 240))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(224, 176, 176)));
                        if ui.add(exit_btn).clicked() {
                            self.close_prompt_open = false;
                            self.allow_close = true;
                            let _ = controller().lock().unwrap().stop();
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        let tray_btn = egui::Button::new(
                            RichText::new("缩到托盘").color(Color32::from_rgb(250, 252, 255)),
                        )
                        .fill(Color32::from_rgb(46, 102, 177))
                        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(33, 82, 147)));
                        if ui.add(tray_btn).clicked() {
                            self.close_prompt_open = false;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }
                        let cancel_btn = egui::Button::new("取消")
                            .fill(Color32::from_rgb(246, 249, 253))
                            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(201, 213, 226)));
                        if ui.add(cancel_btn).clicked() {
                            self.close_prompt_open = false;
                        }
                    });
                });
        }
    }
}

fn configure_theme(ctx: &Context) {
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(10.0, 10.0);
    style.spacing.button_padding = egui::vec2(12.0, 8.0);
    style.spacing.window_margin = egui::Margin::same(10);
    style.spacing.menu_margin = egui::Margin::same(8);
    style.visuals = egui::Visuals::light();
    style.visuals.window_fill = Color32::from_rgb(248, 250, 253);
    style.visuals.panel_fill = Color32::from_rgb(242, 246, 250);
    style.visuals.extreme_bg_color = Color32::from_rgb(232, 238, 245);
    style.visuals.faint_bg_color = Color32::from_rgb(240, 244, 249);
    style.visuals.code_bg_color = Color32::from_rgb(238, 242, 247);
    style.visuals.window_stroke = egui::Stroke::new(1.0, Color32::from_rgb(208, 216, 226));
    style.visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(250, 252, 255);
    style.visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, Color32::from_rgb(212, 220, 229));
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(28, 96, 176);
    style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(21, 79, 147));
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(229, 237, 246);
    style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::from_rgb(120, 155, 197));
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(255, 255, 255);
    style.visuals.widgets.inactive.bg_stroke =
        egui::Stroke::new(1.0, Color32::from_rgb(204, 213, 223));
    style.visuals.selection.bg_fill = Color32::from_rgb(212, 227, 245);
    style.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(42, 123, 216));
    ctx.set_style(style);
}

fn configure_fonts(ctx: &Context) {
    let mut fonts = FontDefinitions::default();

    if let Some((font_name, font_data)) = load_chinese_font() {
        fonts
            .font_data
            .insert(font_name.clone(), FontData::from_owned(font_data).into());

        if let Some(family) = fonts.families.get_mut(&FontFamily::Proportional) {
            family.insert(0, font_name.clone());
        }
        if let Some(family) = fonts.families.get_mut(&FontFamily::Monospace) {
            family.insert(0, font_name);
        }
    }

    ctx.set_fonts(fonts);
}

fn load_chinese_font() -> Option<(String, Vec<u8>)> {
    let candidates = [
        (
            "microsoft_yahei",
            PathBuf::from(r"C:\Windows\Fonts\msyh.ttc"),
        ),
        (
            "microsoft_yahei_ui",
            PathBuf::from(r"C:\Windows\Fonts\msyhbd.ttc"),
        ),
        ("simsun", PathBuf::from(r"C:\Windows\Fonts\simsun.ttc")),
        (
            "pingfang",
            PathBuf::from("/System/Library/Fonts/PingFang.ttc"),
        ),
        (
            "noto_sans_cjk",
            PathBuf::from("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc"),
        ),
    ];

    for (name, path) in candidates {
        if let Ok(data) = fs::read(&path) {
            return Some((name.to_string(), data));
        }
    }

    None
}

fn action_toolbar(ui: &mut Ui, app: &mut EguiConsoleApp, validation_errors: &[String]) {
    let has_errors = !validation_errors.is_empty();
    section_card(ui, "", |ui| {
        ui.columns(3, |columns| {
            toolbar_group(&mut columns[0], "配置管理", |ui| {
                tool_group_title(ui, "配置");
                ui.horizontal(|ui| {
                    if ui.button("重新加载").clicked() {
                        app.load_config();
                    }
                    if ui.button("保存配置").clicked() {
                        if has_errors {
                            app.show_validation_error(validation_errors);
                        } else {
                            app.save_config();
                        }
                    }
                    if ui.button("导出快照").clicked() {
                        if has_errors {
                            app.show_validation_error(validation_errors);
                        } else {
                            app.export_snapshot();
                        }
                    }
                });
            });

            toolbar_group(&mut columns[1], "服务控制", |ui| {
                tool_group_title(ui, "服务");
                ui.horizontal(|ui| {
                    if ui.button("启动代理").clicked() {
                        if has_errors {
                            app.show_validation_error(validation_errors);
                        } else {
                            app.start_proxy();
                        }
                    }
                    if ui.button("停止代理").clicked() {
                        app.stop_proxy();
                    }
                });
            });

            toolbar_group(&mut columns[2], "日志运维", |ui| {
                tool_group_title(ui, "日志");
                ui.horizontal(|ui| {
                if ui.button("刷新日志").clicked() {
                    app.refresh_logs();
                    app.show_success("日志已刷新", "日志已刷新".to_string());
                }
                    if ui.button("清空显示").clicked() {
                        app.clear_log_view();
                        app.show_success("已清空显示", "已清空历史日志显示，后续新日志会继续显示。".to_string());
                    }
                    if ui.button("导出当前日志").clicked() {
                        app.export_current_logs();
                    }
                    if ui
                        .button(if app.auto_refresh_logs {
                            "关闭自动刷新"
                        } else {
                            "开启自动刷新"
                        })
                        .clicked()
                    {
                        app.auto_refresh_logs = !app.auto_refresh_logs;
                    }
                });
            });
        });
    });
}

fn overview_page(
    ui: &mut Ui,
    app: &EguiConsoleApp,
    running: bool,
    auth_label: &str,
    mode_label: &str,
    domain_label: &str,
    diff_fields: &[&'static str],
    failure_summary: &[LogEntry],
) {
    let user_count = non_empty_line_count(&app.form.users_text);
    let ip_count = non_empty_line_count(&app.form.ips_text);
    let cidr_count = non_empty_line_count(&app.form.cidrs_text);

    ui.columns(4, |columns| {
        metrics_card(&mut columns[0], "服务状态", if running { "运行中" } else { "未运行" }, &app.form.listen);
        metrics_card(&mut columns[1], "认证方式", auth_label, &format!("账号数：{user_count}"));
        metrics_card(&mut columns[2], "访问策略", mode_label, &format!("IP：{ip_count} ｜ CIDR：{cidr_count}"));
        metrics_card(
            &mut columns[3],
            "日志状态",
            if app.auto_refresh_logs { "自动刷新" } else { "手动刷新" },
            &format!("最近日志：{} 条", app.logs.len()),
        );
    });

    ui.add_space(8.0);
    ui.columns(2, |columns| {
        section_card(&mut columns[0], "当前配置", |ui| {
            read_only_row(ui, "日志目录", &app.form.log_dir);
            read_only_row(ui, "日志保留", &app.form.retention_days);
            read_only_row(ui, "单文件上限", &app.form.max_file_size_mb);
            read_only_row(ui, "域名转发", domain_label);
            read_only_row(ui, "自动刷新", if app.auto_refresh_logs { "开启" } else { "关闭" });
            read_only_row(ui, "固定配置", DEFAULT_CONFIG_PATH);
        });

        section_card(&mut columns[1], "治理规则", |ui| {
            read_only_row(ui, "认证模式", auth_label);
            read_only_row(ui, "访问策略", mode_label);
            read_only_row(ui, "关闭行为", "提示后缩到托盘");
            read_only_row(ui, "窗口恢复", "托盘左键/双击");
            read_only_row(ui, "配置状态", if app.validation_errors().is_empty() { "校验通过" } else { "待修正" });
        });
    });

    ui.add_space(8.0);
    ui.columns(2, |columns| {
        section_card(&mut columns[0], "待保存变更", |ui| {
            ui.horizontal(|ui| {
                if info_button(ui, "说明") {
                    ui.ctx().memory_mut(|_| {});
                }
            });
            if diff_fields.is_empty() {
                ui.label(RichText::new("当前表单与已加载配置一致。").weak());
            } else {
                for field in diff_fields {
                    ui.label(format!("• {field}"));
                }
            }
        });

        section_card(&mut columns[1], "最近失败摘要", |ui| {
            if failure_summary.is_empty() {
                ui.label(RichText::new("最近日志中没有失败或拦截事件。").weak());
            } else {
                for item in failure_summary.iter().take(6) {
                    ui.label(
                        RichText::new(format!(
                            "{} ｜ {} ｜ {} ｜ {}",
                            item.timestamp,
                            item.kind_label(),
                            item.target,
                            if item.reason.is_empty() { "-" } else { &item.reason }
                        ))
                        .size(12.0),
                    );
                }
            }
        });
    });
}

fn basic_page(ui: &mut Ui, form: &mut ConfigForm) {
    section_card(ui, "基础设置", |ui| {
        ui.columns(2, |columns| {
            field_edit(&mut columns[0], "监听地址", &mut form.listen, false);
            field_edit(&mut columns[1], "日志目录", &mut form.log_dir, false);
            field_edit(&mut columns[0], "日志保留天数", &mut form.retention_days, false);
            field_edit(&mut columns[1], "单文件大小上限（MB）", &mut form.max_file_size_mb, false);
            read_only_row(&mut columns[0], "固定配置文件", DEFAULT_CONFIG_PATH);
        });
    });
}

fn access_page(ui: &mut Ui, form: &mut ConfigForm, mode_label: &str) {
    section_card(ui, "账号与访问控制", |ui| {
        ui.columns(2, |columns| {
            field_edit_multiline(
                &mut columns[0],
                "代理账号（每行一个，格式：用户名=密码）",
                &mut form.users_text,
                8,
            );
            columns[1].vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label("访问策略");
                    ui.selectable_value(&mut form.access_mode, AccessMode::Blacklist, "黑名单");
                    ui.selectable_value(&mut form.access_mode, AccessMode::Whitelist, "白名单");
                });
                ui.checkbox(&mut form.allow_domains, "允许域名转发");
                read_only_row(ui, "当前策略说明", mode_label);
            });
        });
        ui.columns(2, |columns| {
            field_edit_multiline(
                &mut columns[0],
                "目标 IP 列表（每行一个）",
                &mut form.ips_text,
                8,
            );
            field_edit_multiline(
                &mut columns[1],
                "目标网段 CIDR（每行一个）",
                &mut form.cidrs_text,
                8,
            );
        });
    });
}

fn logs_page(ui: &mut Ui, app: &mut EguiConsoleApp, running: bool, domain_label: &str) {
    let filtered = app.filtered_logs();
    section_card(ui, "日志筛选", |ui| {
        ui.horizontal(|ui| {
            filter_badge(ui, &format!("服务 {}", if running { "运行中" } else { "未运行" }));
            filter_badge(ui, &format!("域名 {domain_label}"));
            filter_badge(ui, &format!("刷新 {}", if app.auto_refresh_logs { "自动" } else { "手动" }));
        });

        ui.horizontal_wrapped(|ui| {
            ui.add(
                TextEdit::singleline(&mut app.log_search)
                    .desired_width(280.0)
                    .hint_text("搜索用户、客户端、目标、原因"),
            );
            ComboBox::from_label("事件类别")
                .selected_text(app.log_filter.label())
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut app.log_filter, LogFilter::All, LogFilter::All.label());
                    ui.selectable_value(&mut app.log_filter, LogFilter::Access, LogFilter::Access.label());
                    ui.selectable_value(&mut app.log_filter, LogFilter::Auth, LogFilter::Auth.label());
                    ui.selectable_value(&mut app.log_filter, LogFilter::Failed, LogFilter::Failed.label());
                    ui.selectable_value(&mut app.log_filter, LogFilter::Policy, LogFilter::Policy.label());
                    ui.selectable_value(&mut app.log_filter, LogFilter::System, LogFilter::System.label());
                });
            ui.label(RichText::new(format!("显示 {} / {} 条", filtered.len(), app.logs.len())).weak());
        });
    });

    ui.add_space(8.0);
    section_card(ui, "日志列表", |ui| {
        ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
            egui::Grid::new("logs_grid")
                .num_columns(5)
                .striped(true)
                .spacing([14.0, 10.0])
                .min_col_width(110.0)
                .show(ui, |ui| {
                    grid_header(ui, "时间 / 类型");
                    grid_header(ui, "用户 / 客户端");
                    grid_header(ui, "客户端 IP");
                    grid_header(ui, "目标");
                    grid_header(ui, "原因");
                    ui.end_row();

                    for entry in filtered {
                        ui.label(RichText::new(format!("{} / {}", entry.timestamp, entry.kind_label())).size(12.5));
                        ui.label(RichText::new(format!("{} / {}", entry.user, entry.client_addr)).size(12.5));
                        ui.label(RichText::new(entry.client_ip.clone()).size(12.5));
                        ui.label(RichText::new(entry.target.clone()).size(12.5));
                        ui.label(
                            RichText::new(if entry.reason.is_empty() {
                                "-".to_string()
                            } else {
                                entry.reason
                            })
                            .size(12.5),
                        );
                        ui.end_row();
                    }
                });
        });
    });
}

fn nav_button(ui: &mut Ui, page: &mut Page, target: Page, title: &str) {
    let selected = *page == target;
    let button = egui::Button::new(
        RichText::new(title)
            .size(13.0)
            .strong()
            .color(if selected {
                Color32::from_rgb(18, 56, 104)
            } else {
                Color32::from_rgb(218, 228, 238)
            }),
    )
    .fill(if selected {
        Color32::from_rgb(247, 251, 255)
    } else {
        Color32::from_rgb(42, 57, 72)
    })
    .stroke(if selected {
        egui::Stroke::new(1.0, Color32::from_rgb(145, 180, 219))
    } else {
        egui::Stroke::new(1.0, Color32::from_rgb(54, 72, 88))
    });

    if ui.add_sized([148.0, 38.0], button).clicked() {
        *page = target;
    }
}

fn section_card(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(252, 253, 255))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(212, 220, 229)))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            if !title.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).strong().size(14.0));
                });
                ui.add_space(6.0);
                ui.separator();
                ui.add_space(4.0);
            }
            add_contents(ui)
        });
}

fn toolbar_group(ui: &mut Ui, _title: &str, add_contents: impl FnOnce(&mut Ui)) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(246, 249, 253))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(213, 221, 230)))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| add_contents(ui));
}

fn metrics_card(ui: &mut Ui, title: &str, value: &str, desc: &str) {
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(255, 255, 255))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(215, 223, 232)))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.label(RichText::new(title).small().strong().color(Color32::from_rgb(91, 105, 121)));
            ui.add_space(2.0);
            ui.label(RichText::new(value).size(22.0).strong().color(Color32::from_rgb(23, 51, 88)));
            ui.label(RichText::new(desc).weak());
        });
}

fn read_only_row(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.add_sized([140.0, 20.0], egui::Label::new(RichText::new(label).strong()));
        let mut text = value.to_string();
        ui.add_sized(
            [ui.available_width(), 26.0],
            TextEdit::singleline(&mut text).interactive(false),
        );
    });
}

fn field_edit(ui: &mut Ui, label: &str, value: &mut String, password: bool) {
    ui.label(RichText::new(label).strong());
    let mut edit = TextEdit::singleline(value).desired_width(f32::INFINITY);
    if password {
        edit = edit.password(true);
    }
    ui.add(edit);
    ui.add_space(4.0);
}

fn field_edit_multiline(ui: &mut Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(RichText::new(label).strong());
    ui.add(
        TextEdit::multiline(value)
            .desired_width(f32::INFINITY)
            .desired_rows(rows),
    );
    ui.add_space(4.0);
}

fn page_title(ui: &mut Ui, title: &str, status: &str) {
    ui.horizontal(|ui| {
        ui.heading(title);
        ui.add_space(8.0);
        ui.label(
            RichText::new(status)
                .size(12.0)
                .color(Color32::from_rgb(96, 108, 122)),
        );
    });
    ui.add_space(4.0);
}

fn sidebar_meta(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).weak());
        ui.label(RichText::new(value).strong().size(12.0));
    });
}

fn tool_group_title(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).strong().size(13.0));
    ui.add_space(2.0);
}

fn filter_badge(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(238, 243, 249))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(214, 221, 230)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(8, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(Color32::from_rgb(80, 96, 113)));
        });
}

fn grid_header(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).strong().color(Color32::from_rgb(74, 88, 103)));
}

fn info_button(ui: &mut Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).size(12.0))
            .fill(Color32::from_rgb(244, 248, 252))
            .stroke(egui::Stroke::new(1.0, Color32::from_rgb(200, 212, 225))),
    )
    .clicked()
}

fn restore_window(ctx: &Context) {
    #[cfg(target_os = "windows")]
    native_restore_window_windows(APP_TITLE);

    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
}

fn force_exit() -> ! {
    let _ = controller().lock().unwrap().stop();
    std::process::exit(0);
}

#[cfg(target_os = "windows")]
fn native_restore_window_windows(title: &str) {
    use std::ffi::c_void;

    const SW_RESTORE: i32 = 9;
    const SW_SHOW: i32 = 5;

    unsafe extern "system" {
        fn FindWindowW(class_name: *const u16, window_name: *const u16) -> *mut c_void;
        fn ShowWindow(hwnd: *mut c_void, n_cmd_show: i32) -> i32;
        fn SetForegroundWindow(hwnd: *mut c_void) -> i32;
    }

    let wide_title = wide(title);
    let hwnd = unsafe { FindWindowW(std::ptr::null(), wide_title.as_ptr()) };
    if hwnd.is_null() {
        return;
    }

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
    }
}

#[cfg(target_os = "windows")]
fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn build_tray_icon() -> std::result::Result<tray_icon::Icon, String> {
    let (rgba, width, height) = build_app_icon_rgba();
    tray_icon::Icon::from_rgba(rgba, width, height)
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
            let (r, g, b, a) = if ring {
                (255, 255, 255, 235)
            } else if accent {
                (255, 255, 255, 255)
            } else {
                (r, g, b, a)
            };

            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }

    (rgba, width as u32, height as u32)
}

fn non_empty_line_count(input: &str) -> usize {
    input.lines().filter(|line| !line.trim().is_empty()).count()
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
            .name("sock5s-egui-runtime".to_string())
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
            .ok_or_else(|| format!("第 {} 行账号格式错误，请使用 `用户名=密码`。", index + 1))?;

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
        let value = parser(entry)
            .map_err(|err| format!("{label} 第 {} 行格式错误：{entry}（{err}）", index + 1))?;
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

    let start = merged.len().saturating_sub(1000);
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
                "{} {} {} {} {} {} {}",
                entry.timestamp,
                entry.kind,
                entry.user,
                entry.client_ip,
                entry.client_addr,
                entry.target,
                entry.reason
            )
            .to_ascii_lowercase();
            haystack.contains(&keyword)
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
        LogFilter::System => {
            entry.kind == "server_started"
                || entry.kind == "server_stopped"
                || entry.kind == "connection_closed"
        }
    }
}

fn now_display() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn diff_fields<'a>(baseline: &'a ConfigForm, current: &'a ConfigForm) -> Vec<&'static str> {
    let mut diffs = Vec::new();

    if baseline.listen != current.listen {
        diffs.push("监听地址");
    }
    if baseline.users_text != current.users_text {
        diffs.push("代理账号");
    }
    if baseline.access_mode != current.access_mode {
        diffs.push("访问策略");
    }
    if baseline.allow_domains != current.allow_domains {
        diffs.push("域名转发");
    }
    if baseline.ips_text != current.ips_text {
        diffs.push("目标 IP 列表");
    }
    if baseline.cidrs_text != current.cidrs_text {
        diffs.push("目标网段 CIDR");
    }
    if baseline.log_dir != current.log_dir {
        diffs.push("日志目录");
    }
    if baseline.retention_days != current.retention_days {
        diffs.push("日志保留天数");
    }
    if baseline.max_file_size_mb != current.max_file_size_mb {
        diffs.push("单文件大小上限");
    }

    diffs
}

fn recent_failures(entries: &[LogEntry]) -> Vec<LogEntry> {
    entries
        .iter()
        .filter(|entry| {
            entry.kind == "auth_failed"
                || entry.kind == "connect_failed"
                || entry.kind == "access_denied"
                || entry.kind == "session_error"
        })
        .cloned()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn export_logs(entries: &[LogEntry]) -> std::result::Result<PathBuf, String> {
    let exports_dir = PathBuf::from("exports");
    fs::create_dir_all(&exports_dir).map_err(|err| format!("创建导出目录失败：{err}"))?;
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let path = exports_dir.join(format!("logs-{timestamp}.txt"));

    let mut content = String::new();
    for entry in entries {
        let reason = if entry.reason.is_empty() { "-" } else { &entry.reason };
        content.push_str(&format!(
            "{}\tkind={}\tuser={}\tclient_ip={}\tclient_addr={}\ttarget={}\treason={}\n",
            entry.timestamp, entry.kind, entry.user, entry.client_ip, entry.client_addr, entry.target, reason
        ));
    }

    fs::write(&path, content).map_err(|err| format!("导出日志失败：{err}"))?;
    Ok(path)
}
