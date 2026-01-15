use std::{
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
                let _ = start_or_restart_sse_server(app_handle, settings).await;
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_sse_server_cmd,
            get_general_settings,
            set_general_settings,
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
        }
    }
}

#[derive(Default)]
struct SseRuntime {
    state: Option<Arc<sse_server::AppState>>,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
    bind_addr: Option<SocketAddr>,
    join_handle: Option<tauri::async_runtime::JoinHandle<()>>,
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
    let mut rt = SSE_RUNTIME.write().await;
    if let Some(tx) = rt.shutdown.take() {
        let _ = tx.send(());
    }
    rt.join_handle = None;
    rt.state = None;
    rt.bind_addr = None;
}

async fn start_or_restart_sse_server(app_handle: tauri::AppHandle, settings: GeneralSettings) -> Result<String, String> {
    use tokio::net::TcpListener;

    // 应用 WS debug/过滤配置
    bili_websocket_client::set_ws_debug_enabled(settings.ws_debug).await;
    bili_websocket_client::set_danmu_filter_config(settings.danmu_filter.clone()).await;

    // 先停旧的
    stop_sse_server().await;

    let bind_ip = if settings.sse_public { "0.0.0.0" } else { "127.0.0.1" };
    let bind_addr: SocketAddr = format!("{bind_ip}:{}", settings.sse_port)
        .parse()
        .map_err(|e| format!("绑定地址解析失败: {e}"))?;

    let state = Arc::new(sse_server::AppState {
        sse_connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
        stats: Arc::new(RwLock::new(sse_server::Stats::default())),
        config: Arc::new(RwLock::new(sse_server::Config::default())),
        auth: Arc::new(RwLock::new(sse_server::AuthConfig {
            token: settings.sse_token.clone(),
        })),
    });

    let app = sse_server::create_app(state.clone());
    let listener = TcpListener::bind(bind_addr)
        .await
        .map_err(|e| format!("SSE 端口绑定失败: {e}"))?;

    let (tx, rx) = tokio::sync::oneshot::channel::<()>();

    let join_handle = tauri::async_runtime::spawn(async move {
        let server = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            });
        let _ = server.await;
    });

    {
        let mut rt = SSE_RUNTIME.write().await;
        rt.state = Some(state);
        rt.shutdown = Some(tx);
        rt.bind_addr = Some(bind_addr);
        rt.join_handle = Some(join_handle);
        rt.settings = settings.clone();
    }

    // 保存到磁盘
    let _ = save_general_settings(&app_handle, &settings);
    Ok(format!("SSE服务器启动在 http://{}", bind_addr))
}

#[tauri::command]
async fn start_sse_server_cmd() -> Result<String, String> {
    let rt = SSE_RUNTIME.read().await;
    if let Some(addr) = rt.bind_addr {
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
        "runtimeBindAddr": rt.bind_addr.map(|a| a.to_string()),
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
        let config_msg = serde_json::json!({
            "type": "config",
            "config": config
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
