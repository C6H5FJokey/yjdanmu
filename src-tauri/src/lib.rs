use std::{
    collections::HashMap,
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::Arc,
};

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tauri::Manager;

mod sse_server;
pub mod bili_websocket_client;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleProfile {
    /// 基础样式（默认应用）
    pub base: sse_server::Config,
    /// 按消息类型覆盖样式（消息的 type 字段，如 danmu/gift/superChat 等）
    #[serde(default)]
    pub by_type: HashMap<String, sse_server::Config>,
    /// 佩戴本房间粉丝牌（media_ruid 与主播 uid 匹配）时的覆盖样式（可选）
    pub own_medal: Option<sse_server::Config>,
    /// 舰队高亮：总督（privilege_type=1，仅普通弹幕 type=danmu 生效）
    pub guard_governor: Option<sse_server::Config>,
    /// 舰队高亮：提督（privilege_type=2，仅普通弹幕 type=danmu 生效）
    pub guard_admiral: Option<sse_server::Config>,
    /// 舰队高亮：舰长（privilege_type=3，仅普通弹幕 type=danmu 生效）
    pub guard_captain: Option<sse_server::Config>,
    /// 主播/本人弹幕覆盖样式（可选，仅普通弹幕 type=danmu 生效；用于视觉强调）
    pub streamer: Option<sse_server::Config>,
    /// 房管弹幕覆盖样式（可选）
    pub moderator: Option<sse_server::Config>,
}

impl Default for StyleProfile {
    fn default() -> Self {
        Self {
            base: sse_server::Config::default(),
            by_type: HashMap::new(),
            own_medal: None,
            guard_governor: None,
            guard_admiral: None,
            guard_captain: None,
            streamer: None,
            moderator: None,
        }
    }
}

static STYLE_PROFILE: Lazy<Arc<RwLock<StyleProfile>>> = Lazy::new(|| Arc::new(RwLock::new(StyleProfile::default())));

fn style_profile_path(app_handle: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app_handle.path().app_config_dir().ok()?;
    Some(dir.join("yjdanmu-style.json"))
}

fn legacy_room_styles_path(app_handle: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app_handle.path().app_config_dir().ok()?;
    Some(dir.join("yjdanmu-room-styles.json"))
}

fn load_style_profile(app_handle: &tauri::AppHandle) -> StyleProfile {
    // 新版：全局样式配置
    if let Some(path) = style_profile_path(app_handle) {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(profile) = serde_json::from_slice::<StyleProfile>(&bytes) {
                return profile;
            }
        }
    }

    // 兼容：旧版“按房间”配置文件 -> 尝试取 global 或第一项
    if let Some(path) = legacy_room_styles_path(app_handle) {
        if let Ok(bytes) = fs::read(path) {
            if let Ok(map) = serde_json::from_slice::<HashMap<String, StyleProfile>>(&bytes) {
                if let Some(p) = map.get("global") {
                    return p.clone();
                }
                if let Some((_, p)) = map.into_iter().next() {
                    return p;
                }
            }
        }
    }

    StyleProfile::default()
}

fn save_style_profile(app_handle: &tauri::AppHandle, profile: &StyleProfile) -> Result<(), String> {
    let path = style_profile_path(app_handle).ok_or_else(|| "无法获取配置目录".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(profile).map_err(|e| format!("序列化样式配置失败: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("写入样式配置失败: {e}"))?;
    Ok(())
}

async fn broadcast_config_to_sse(config: sse_server::Config) -> Result<(), String> {
    if let Some(state) = get_sse_state().await {
        *state.config.write().await = config.clone();
        let msg = serde_json::json!({
            "type": "config",
            "config": config,
        });
        sse_server::send_to_all_connections(&state, msg).await;
        Ok(())
    } else {
        Err("SSE服务器未启动".to_string())
    }
}

pub async fn apply_style_to_sse_message(mut val: serde_json::Value) -> serde_json::Value {
    let Some(obj) = val.as_object_mut() else {
        return val;
    };
    let Some(msg_type) = obj.get("type").and_then(|v| v.as_str()) else {
        return val;
    };
    if msg_type == "config" || msg_type == "ping" {
        return val;
    }

    let profile = STYLE_PROFILE.read().await;

    // 合成策略：
    // 1) 先得到“普通弹幕 danmu”的基础样式：base -> byType[danmu]。
    // 2) gift/superChat 等其它类型默认继承 danmu（让它们“跟普通弹幕一个机制”）；如果存在 byType[type] 才替换为该 type 的样式。
    // 3) danmu 子类高亮（粉丝牌/舰队/房管）只覆盖视觉字段。
    let mut danmu_effective = profile.base.clone();
    if let Some(danmu_cfg) = profile.by_type.get("danmu") {
        danmu_effective = danmu_cfg.clone();
    }

    let mut effective = if msg_type == "danmu" {
        danmu_effective.clone()
    } else {
        // 其它类型默认继承 danmu
        danmu_effective.clone()
    };

    if msg_type != "danmu" {
        if let Some(type_cfg) = profile.by_type.get(msg_type) {
            // 目前是“整份替换”策略（UI 也是整份配置）；没有覆盖时就继承 danmu。
            effective = type_cfg.clone();
        }
    }

    let mut apply_visual_overlay = |cfg: &sse_server::Config| {
        effective.font_size = cfg.font_size;
        if cfg.color.is_some() {
            effective.color = cfg.color.clone();
        }
        if cfg.stroke_color.is_some() {
            effective.stroke_color = cfg.stroke_color.clone();
        }
        effective.stroke_width = cfg.stroke_width;
    };

    if msg_type == "danmu" {
        let has_own_medal = obj
            .get("hasOwnMedal")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if has_own_medal {
            if let Some(medal_cfg) = profile.own_medal.as_ref() {
                // 高亮层：只覆盖“视觉强调”字段
                apply_visual_overlay(medal_cfg);
            }
        }

        // 舰队高亮：也是普通弹幕的“子类高亮”
        // blivedm/web.py: privilege_type: 0非舰队, 1总督, 2提督, 3舰长
        let guard_level = obj
            .get("guardLevel")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        match guard_level {
            1 => {
                if let Some(cfg) = profile.guard_governor.as_ref() {
                    apply_visual_overlay(cfg);
                }
            }
            2 => {
                if let Some(cfg) = profile.guard_admiral.as_ref() {
                    apply_visual_overlay(cfg);
                }
            }
            3 => {
                if let Some(cfg) = profile.guard_captain.as_ref() {
                    apply_visual_overlay(cfg);
                }
            }
            _ => {}
        }

        let is_streamer = obj
            .get("isStreamer")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_streamer {
            if let Some(cfg) = profile.streamer.as_ref() {
                // 主播/本人：同样只做视觉强调覆盖
                apply_visual_overlay(cfg);
            }
        }

        let is_moderator = obj
            .get("isModerator")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if is_moderator {
            if let Some(mod_cfg) = profile.moderator.as_ref() {
                // 房管优先级更高：同样只做视觉强调覆盖
                apply_visual_overlay(mod_cfg);
            }
        }
    }

    // 明确写入样式字段，优先级高于 preview.html 的 defaultConfig
    obj.insert(
        "fontSize".to_string(),
        serde_json::Value::Number(serde_json::Number::from(effective.font_size)),
    );
    // 如果样式配置里 color/strokeColor 为 null，则回退到 websocket 原始颜色（消息里的 color）。
    let ws_color = obj
        .get("color")
        .and_then(|v| v.as_str())
        .unwrap_or("#ffffff")
        .to_string();

    let final_color = effective.color.clone().unwrap_or_else(|| ws_color.clone());
    let final_stroke_color = effective
        .stroke_color
        .clone()
        .unwrap_or_else(|| ws_color.clone());

    obj.insert("color".to_string(), serde_json::Value::String(final_color));
    obj.insert(
        "strokeColor".to_string(),
        serde_json::Value::String(final_stroke_color),
    );
    obj.insert(
        "strokeWidth".to_string(),
        serde_json::Value::Number(serde_json::Number::from(effective.stroke_width)),
    );
    obj.insert(
        "typingSpeed".to_string(),
        serde_json::Value::Number(serde_json::Number::from(effective.typing_speed)),
    );
    obj.insert(
        "displayDuration".to_string(),
        serde_json::Value::Number(serde_json::Number::from(effective.display_duration)),
    );
    obj.insert(
        "fadeDuration".to_string(),
        serde_json::Value::Number(serde_json::Number::from(effective.fade_duration)),
    );
    obj.insert(
        "shakeAmplitude".to_string(),
        serde_json::Value::Number(
            serde_json::Number::from_f64(effective.shake_amplitude).unwrap_or_else(|| 0.into()),
        ),
    );
    obj.insert(
        "randomTilt".to_string(),
        serde_json::Value::Number(
            serde_json::Number::from_f64(effective.random_tilt).unwrap_or_else(|| 0.into()),
        ),
    );

    val
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(rust_analyzer)]
pub fn run() {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[cfg(not(rust_analyzer))]
pub fn run() {
    let context = tauri::generate_context!("tauri.conf.json");

    tauri::Builder::default()
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            
            // 启动SSE服务器
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let settings = load_general_settings(&app_handle).unwrap_or_default();
                if let Err(e) = start_or_restart_sse_server(app_handle, settings).await {
                    eprintln!("[SSE] 启动失败: {e}");
                }
            });

            // 加载房间样式配置
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let profile = load_style_profile(&app_handle);
                *STYLE_PROFILE.write().await = profile;
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_sse_server_cmd,
            get_general_settings,
            set_general_settings,
            get_current_room_context,
            get_style_profile,
            set_style_profile,
            // 兼容旧命令名（后续可移除）
            get_room_style_profile,
            set_room_style_profile,
            send_danmu,
            send_config,
            get_status,
            open_in_browser,
            toggle_always_on_top,
            connect_websocket,
            disconnect_websocket
        ])
        .run(context)
        .expect("error while running tauri application");
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneralSettings {
    pub sse_port: u16,
    pub sse_public: bool,
    pub sse_token: Option<String>,
    pub ws_debug: bool,
    pub default_reconnect_interval: u64,
    pub default_max_reconnect_attempts: u32,
    pub danmu_filter: bili_websocket_client::DanmuFilterConfig,
    #[serde(default)]
    pub render_settings: sse_server::RenderConfig,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            sse_port: 8081,
            sse_public: false,
            sse_token: None,
            ws_debug: false,
            default_reconnect_interval: 3000,
            default_max_reconnect_attempts: 5,
            danmu_filter: bili_websocket_client::DanmuFilterConfig::default(),
            render_settings: sse_server::RenderConfig::default(),
        }
    }
}

#[derive(Default)]
struct SseRuntime {
    state: Option<Arc<sse_server::AppState>>,
    shutdown: Option<tokio::sync::watch::Sender<bool>>,
    bind_addrs: Vec<SocketAddr>,
    join_handles: Vec<tauri::async_runtime::JoinHandle<()>>,
    settings: GeneralSettings,
}

static SSE_RUNTIME: Lazy<Arc<RwLock<SseRuntime>>> = Lazy::new(|| Arc::new(RwLock::new(SseRuntime::default())));

fn settings_path(app_handle: &tauri::AppHandle) -> Option<PathBuf> {
    let dir = app_handle.path().app_config_dir().ok()?;
    Some(dir.join("yjdanmu-settings.json"))
}

fn load_general_settings(app_handle: &tauri::AppHandle) -> Option<GeneralSettings> {
    let path = settings_path(app_handle)?;
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<GeneralSettings>(&bytes).ok()
}

fn save_general_settings(app_handle: &tauri::AppHandle, settings: &GeneralSettings) -> Result<(), String> {
    let path = settings_path(app_handle).ok_or_else(|| "无法获取配置目录".to_string())?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {e}"))?;
    }
    let bytes = serde_json::to_vec_pretty(settings).map_err(|e| format!("序列化设置失败: {e}"))?;
    fs::write(&path, bytes).map_err(|e| format!("写入设置失败: {e}"))?;
    Ok(())
}

pub async fn get_sse_state() -> Option<Arc<sse_server::AppState>> {
    SSE_RUNTIME.read().await.state.clone()
}

async fn stop_sse_server() {
    let (shutdown, join_handles) = {
        let mut rt = SSE_RUNTIME.write().await;
        let shutdown = rt.shutdown.take();
        let join_handles = std::mem::take(&mut rt.join_handles);
        rt.state = None;
        rt.bind_addrs.clear();
        (shutdown, join_handles)
    };

    if let Some(tx) = shutdown {
        let _ = tx.send(true);
    }
    for handle in join_handles {
        // 等待旧服务器真正退出，避免立即 bind 同端口时报 AddrInUse
        let _ = handle.await;
    }
}

async fn start_or_restart_sse_server(app_handle: tauri::AppHandle, settings: GeneralSettings) -> Result<String, String> {
    use tokio::net::TcpListener;
    use std::io::ErrorKind;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    let addr_v4 = SocketAddr::new(
        IpAddr::V4(if settings.sse_public { Ipv4Addr::UNSPECIFIED } else { Ipv4Addr::LOCALHOST }),
        settings.sse_port,
    );
    let addr_v6 = SocketAddr::new(
        IpAddr::V6(if settings.sse_public { Ipv6Addr::UNSPECIFIED } else { Ipv6Addr::LOCALHOST }),
        settings.sse_port,
    );
    let want_addrs = vec![addr_v4, addr_v6];

    // 如果 bind_addr 没变且服务已在运行：不重启（避免“自己占用自己”）
    {
        let rt = SSE_RUNTIME.read().await;
        if rt.bind_addrs == want_addrs {
            if let Some(state) = rt.state.clone() {
                // 热更新 token
                *state.auth.write().await = sse_server::AuthConfig {
                    token: settings.sse_token.clone(),
                };

                // 热更新 render 设置（无需重启）
                *state.render.write().await = settings.render_settings.clone();

                // 广播一次 config（让已打开的 preview 立即生效）
                let config = state.config.read().await.clone();
                let render = state.render.read().await.clone();
                sse_server::send_to_all_connections(
                    &state,
                    serde_json::json!({"type": "config", "config": config, "render": render}),
                )
                .await;
            }
        }
    }

    {
        let rt = SSE_RUNTIME.write().await;
        if rt.bind_addrs == want_addrs && rt.state.is_some() {
            // 应用 WS debug/过滤配置（不需要重启）
            drop(rt);
            bili_websocket_client::set_ws_debug_enabled(settings.ws_debug).await;
            bili_websocket_client::set_danmu_filter_config(settings.danmu_filter.clone()).await;

            // 更新 runtime 记录
            let mut rt2 = SSE_RUNTIME.write().await;
            rt2.settings = settings.clone();

            // 保存到磁盘
            let _ = save_general_settings(&app_handle, &settings);
            return Ok(format!("SSE服务器已在运行: http://{}（已应用设置）", addr_v4));
        }
    }

    // 需要真正重启：先停旧的并等待退出
    stop_sse_server().await;

    // 应用 WS debug/过滤配置
    bili_websocket_client::set_ws_debug_enabled(settings.ws_debug).await;
    bili_websocket_client::set_danmu_filter_config(settings.danmu_filter.clone()).await;

    let state = Arc::new(sse_server::AppState {
        sse_connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
        stats: Arc::new(RwLock::new(sse_server::Stats::default())),
        config: Arc::new(RwLock::new(sse_server::Config::default())),
        render: Arc::new(RwLock::new(settings.render_settings.clone())),
        auth: Arc::new(RwLock::new(sse_server::AuthConfig {
            token: settings.sse_token.clone(),
        })),
    });

    let listener_v4 = TcpListener::bind(addr_v4)
        .await
        .map_err(|e| {
            if e.kind() == ErrorKind::AddrInUse {
                format!(
                    "SSE 端口绑定失败：端口 {} 已被占用。请在【通用设置】里更换端口，或关闭占用该端口的程序。原始错误: {e}",
                    settings.sse_port
                )
            } else {
                format!("SSE 端口绑定失败: {e}")
            }
        })?;

    // IPv6 监听（用于让 localhost/::1 访问不再产生 IPv6->IPv4 回退等待）
    // 如果系统禁用 IPv6 或绑定失败，仍然只用 IPv4 工作。
    let listener_v6 = match TcpListener::bind(addr_v6).await {
        Ok(l) => Some(l),
        Err(e) => {
            eprintln!("[SSE] IPv6 监听绑定失败（将只启用 IPv4）: {e}");
            None
        }
    };

    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel::<bool>(false);

    let mut join_handles: Vec<tauri::async_runtime::JoinHandle<()>> = Vec::new();

    {
        let app = sse_server::create_app(state.clone());
        let mut rx = shutdown_rx.clone();
        join_handles.push(tauri::async_runtime::spawn(async move {
            let server = axum::serve(listener_v4, app).with_graceful_shutdown(async move {
                let _ = rx.changed().await;
            });
            let _ = server.await;
        }));
    }

    if let Some(listener_v6) = listener_v6 {
        let app = sse_server::create_app(state.clone());
        let mut rx = shutdown_rx.clone();
        join_handles.push(tauri::async_runtime::spawn(async move {
            let server = axum::serve(listener_v6, app).with_graceful_shutdown(async move {
                let _ = rx.changed().await;
            });
            let _ = server.await;
        }));
    }

    {
        let mut rt = SSE_RUNTIME.write().await;
        rt.state = Some(state);
        rt.shutdown = Some(shutdown_tx);
        rt.bind_addrs = want_addrs;
        rt.join_handles = join_handles;
        rt.settings = settings.clone();
    }

    // 保存到磁盘
    let _ = save_general_settings(&app_handle, &settings);
    Ok(format!("SSE服务器启动在 http://{}", addr_v4))
}

#[tauri::command]
async fn start_sse_server_cmd() -> Result<String, String> {
    let rt = SSE_RUNTIME.read().await;
    if let Some(addr) = rt.bind_addrs.first().copied() {
        Ok(format!("SSE服务器运行中: http://{}", addr))
    } else {
        Ok("SSE服务器未启动".to_string())
    }
}

#[tauri::command]
async fn get_general_settings(window: tauri::Window) -> Result<serde_json::Value, String> {
    let app_handle = window.app_handle();
    let disk = load_general_settings(&app_handle).unwrap_or_default();
    let rt = SSE_RUNTIME.read().await;
    Ok(serde_json::json!({
        "settings": disk,
        "runtimeBindAddr": rt.bind_addrs.first().map(|a| a.to_string()),
    }))
}

#[tauri::command]
async fn set_general_settings(window: tauri::Window, settings: GeneralSettings) -> Result<String, String> {
    let app_handle = window.app_handle();
    start_or_restart_sse_server(app_handle.clone(), settings).await
}

#[tauri::command]
async fn send_danmu(text: String, custom_data: Option<serde_json::Value>) -> Result<String, String> {
    if let Some(state) = get_sse_state().await {
        // 构建弹幕数据，确保基本字段存在
        let mut base_data = serde_json::json!({
            "type": "danmu",
            "text": text,
        });
        
        // 如果提供了自定义数据，合并到基础数据中
        if let Some(custom) = custom_data {
            if let serde_json::Value::Object(custom_map) = custom {
                if let serde_json::Value::Object(mut base_map) = base_data {
                    // 合并自定义属性到基础数据中
                    for (key, value) in custom_map.iter() {
                        base_map.insert(key.clone(), value.clone());
                    }
                    base_data = serde_json::Value::Object(base_map);
                }
            }
        }
        
        // 确保必要的时间戳字段存在
        if !base_data.get("time").is_some() {
            let time_val = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            if let serde_json::Value::Object(ref mut map) = base_data {
                map.insert("time".to_string(), serde_json::Value::Number(serde_json::Number::from(time_val)));
            }
        }
        
        if !base_data.get("timestamp").is_some() {
            let timestamp_val = chrono::Local::now().format("%H:%M:%S").to_string();
            if let serde_json::Value::Object(ref mut map) = base_data {
                map.insert("timestamp".to_string(), serde_json::Value::String(timestamp_val));
            }
        }
        
        // 解析弹幕数据
        let danmu_data: sse_server::DanmuData = serde_json::from_value(base_data)
            .map_err(|e| format!("解析弹幕数据失败: {}", e))?;
        
        // 发送弹幕
        let json_data = serde_json::to_value(danmu_data)
            .map_err(|e| format!("序列化弹幕数据失败: {}", e))?;
        
        sse_server::send_to_all_connections(&state, json_data).await;
        
        // 更新统计
        {
            let mut stats = state.stats.write().await;
            stats.danmu_count += 1;
            stats.last_activity = Some(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64);
        }
        
        Ok("弹幕发送成功".to_string())
    } else {
        Err("SSE服务器未启动".to_string())
    }
}

#[tauri::command]
async fn send_config(config: sse_server::Config) -> Result<String, String> {
    if let Some(state) = get_sse_state().await {
        // 更新全局配置
        *state.config.write().await = config.clone();
        
        // 发送配置更新到所有连接
        let render = state.render.read().await.clone();
        let config_msg = serde_json::json!({
            "type": "config",
            "config": config,
            "render": render
        });
        
        sse_server::send_to_all_connections(&state, config_msg).await;
        
        Ok("配置更新成功".to_string())
    } else {
        Err("SSE服务器未启动".to_string())
    }
}

#[tauri::command]
async fn get_status() -> Result<serde_json::Value, String> {
    if let Some(state) = get_sse_state().await {
        let connections = state.sse_connections.read().await.len();
        let stats = state.stats.read().await.clone();
        
        Ok(serde_json::json!({
            "connections": connections,
            "danmu_count": stats.danmu_count,
            "last_activity": stats.last_activity,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
            "development": false  // 默认不是开发模式
        }))
    } else {
        Err("SSE服务器未启动".to_string())
    }
}

#[tauri::command]
async fn open_in_browser(url: String) -> Result<String, String> {
    webbrowser::open(&url)
        .map_err(|e| format!("打开浏览器失败: {}", e))?;
    Ok("已在浏览器中打开".to_string())
}

#[tauri::command]
async fn toggle_always_on_top(window: tauri::Window) -> Result<bool, String> {
    let current = window.is_always_on_top().map_err(|e| format!("获取窗口状态失败: {}", e))?;
    window.set_always_on_top(!current).map_err(|e| format!("设置窗口置顶失败: {}", e))?;
    Ok(!current)
}

#[tauri::command]
async fn connect_websocket(
    window: tauri::Window,
    room_key: String,
    room_key_type: bili_websocket_client::RoomKeyType,
    reconnect_interval: u64,
    max_reconnect_attempts: u32,
    open_live_app_id: Option<i64>,
    open_live_access_key_id: Option<String>,
    open_live_access_key_secret: Option<String>,
) -> Result<String, String> {
    let app_handle = window.app_handle();

    // 连接前下发全局 base 样式，让 preview 立即切换默认样式
    {
        let profile = STYLE_PROFILE.read().await;
        let _ = broadcast_config_to_sse(profile.base.clone()).await;
    }

    bili_websocket_client::connect_websocket(
        app_handle.clone(),
        room_key,
        room_key_type,
        reconnect_interval,
        max_reconnect_attempts,
        open_live_app_id,
        open_live_access_key_id,
        open_live_access_key_secret,
    )
    .await
    .map(|_| "WebSocket连接成功".to_string())
}

#[tauri::command]
async fn disconnect_websocket() -> Result<String, String> {
    bili_websocket_client::disconnect_websocket().await.map(|_| "WebSocket断开成功".to_string())
}

#[tauri::command]
async fn get_current_room_context() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "roomKey": bili_websocket_client::current_room_key().await,
        "roomKeyType": bili_websocket_client::current_room_key_type().await,
    }))
}

#[tauri::command]
async fn get_style_profile(_window: tauri::Window) -> Result<serde_json::Value, String> {
    let profile = STYLE_PROFILE.read().await.clone();
    Ok(serde_json::json!({
        "profile": profile,
    }))
}

#[tauri::command]
async fn set_style_profile(window: tauri::Window, profile: StyleProfile) -> Result<String, String> {
    let app_handle = window.app_handle();
    {
        *STYLE_PROFILE.write().await = profile.clone();
        save_style_profile(&app_handle, &profile)?;
    }

    // 全局配置：立刻下发 base 配置
    let _ = broadcast_config_to_sse(profile.base.clone()).await;
    Ok("样式配置已保存".to_string())
}

#[tauri::command]
async fn get_room_style_profile(_window: tauri::Window, room_key: Option<String>) -> Result<serde_json::Value, String> {
    let _ = room_key;
    get_style_profile(_window).await
}

#[tauri::command]
async fn set_room_style_profile(window: tauri::Window, _room_key: String, profile: StyleProfile) -> Result<String, String> {
    set_style_profile(window, profile).await
}
