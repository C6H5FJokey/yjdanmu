use axum::{
    extract::{Query, State},
    response::Html,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use axum::http::StatusCode;
use futures::Stream;
use tower_http::services::ServeDir;
use tower_http::cors::CorsLayer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};
use uuid::Uuid;
use async_stream;

// 直接内置预览页，避免依赖运行时工作目录下的静态文件路径
static PREVIEW_HTML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../frontend/public/preview.html"));

async fn preview_html_handler() -> Html<&'static str> {
    Html(PREVIEW_HTML)
}

// 全局状态结构
#[derive(Clone)]
pub struct AppState {
    // SSE连接管理
    pub sse_connections: Arc<RwLock<HashMap<String, broadcast::Sender<serde_json::Value>>>>,
    // 统计信息
    pub stats: Arc<RwLock<Stats>>,
    // 配置
    pub config: Arc<RwLock<Config>>,

    // 认证/通用设置
    pub auth: Arc<RwLock<AuthConfig>>,
}

#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthConfig {
    pub token: Option<String>,
}

fn check_token(auth: &AuthConfig, query: &HashMap<String, String>) -> Result<(), (StatusCode, String)> {
    let Some(expected) = auth.token.as_deref() else {
        return Ok(());
    };
    let provided = query.get("token").map(|s| s.as_str()).unwrap_or("");
    if provided == expected {
        Ok(())
    } else {
        Err((StatusCode::UNAUTHORIZED, "Unauthorized".to_string()))
    }
}

#[derive(Default, Clone)]
pub struct Stats {
    pub connections: usize,
    pub danmu_count: u64,
    pub last_activity: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub font_size: u32,
    pub color: String,
    pub stroke_color: String,
    pub stroke_width: u32,
    pub typing_speed: u32,
    pub display_duration: u64,
    pub fade_duration: u64,
    pub shake_amplitude: f64,
    pub random_tilt: f64,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            font_size: 32,
            color: "#ffffff".to_string(),
            stroke_color: "#000000".to_string(),
            stroke_width: 2,
            typing_speed: 100,
            display_duration: 3000,
            fade_duration: 1000,
            shake_amplitude: 2.0,
            random_tilt: 10.0,
        }
    }
}

// 弹幕数据结构
#[derive(Serialize, Deserialize, Clone)]
pub struct DanmuData {
    #[serde(rename = "type")]
    pub danmu_type: String,
    pub text: String,
    #[serde(default = "default_user")]
    pub user: String,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default = "default_size", alias = "fontSize")]
    pub size: u32,
    #[serde(default = "default_time")]
    pub time: u64,
    #[serde(default = "default_timestamp")]
    pub timestamp: String,
    // 样式相关
    #[serde(default = "default_stroke_color", alias = "strokeColor")]
    pub stroke_color: String,
    #[serde(default = "default_stroke_width", alias = "strokeWidth")]
    pub stroke_width: u32,
    #[serde(default = "default_typing_speed", alias = "typingSpeed")]
    pub typing_speed: u32,
    #[serde(default = "default_display_duration", alias = "displayDuration")]
    pub display_duration: u64,
    #[serde(default = "default_fade_duration", alias = "fadeDuration")]
    pub fade_duration: u64,
    #[serde(default = "default_shake_amplitude", alias = "shakeAmplitude")]
    pub shake_amplitude: f64,
    #[serde(default = "default_random_tilt", alias = "randomTilt")]
    pub random_tilt: f64,
}

fn default_user() -> String {
    "匿名用户".to_string()
}

fn default_color() -> String {
    "#ffffff".to_string()
}

fn default_size() -> u32 {
    32
}

fn default_time() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn default_timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn default_stroke_color() -> String {
    "#000000".to_string()
}

fn default_stroke_width() -> u32 {
    2
}

fn default_typing_speed() -> u32 {
    100
}

fn default_display_duration() -> u64 {
    3000
}

fn default_fade_duration() -> u64 {
    1000
}

fn default_shake_amplitude() -> f64 {
    2.0
}

fn default_random_tilt() -> f64 {
    10.0
}

// Connection guard to ensure cleanup on drop
struct ConnectionGuard {
    connection_id: String,
    state: Arc<AppState>,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        // This runs when the stream is dropped, indicating client disconnection
        let state = self.state.clone();
        let connection_id = self.connection_id.clone();
        
        // Spawn a task to handle the cleanup asynchronously
        tauri::async_runtime::spawn(async move {
            {
                let mut connections = state.sse_connections.write().await;
                connections.remove(&connection_id);
                
                // 更新统计
                let mut stats = state.stats.write().await;
                stats.connections = connections.len();
            }
            println!("SSE连接断开: {}, 当前连接数: {}", connection_id, {
                state.sse_connections.read().await.len()
            });
        });
    }
}

// SSE连接端点
pub async fn sse_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HashMap<String, String>>,
) -> axum::response::Response {
    use axum::response::sse::{Event, KeepAlive, Sse};

    // 可选 token 鉴权
    {
        let auth = state.auth.read().await.clone();
        if let Err((code, msg)) = check_token(&auth, &query) {
            return (code, msg).into_response();
        }
    }

    let connection_id = Uuid::new_v4().to_string();
    let (sender, mut receiver) = broadcast::channel::<serde_json::Value>(100);
    
    // 添加连接到全局管理器
    {
        let mut connections = state.sse_connections.write().await;
        connections.insert(connection_id.clone(), sender.clone());
        
        // 更新统计
        let mut stats = state.stats.write().await;
        stats.connections = connections.len();
        stats.last_activity = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
    }
    
    println!("SSE连接建立: {}, 当前连接数: {}", connection_id, {
        state.sse_connections.read().await.len()
    });
    
    // 发送初始配置 (removed connection confirmation message)
    let config = state.config.read().await.clone();
    let config_msg = serde_json::json!({
        "type": "config",
        "config": config
    });
    
    let _ = sender.send(config_msg);

    // Create a connection guard that will clean up when dropped
    let guard = ConnectionGuard {
        connection_id: connection_id.clone(),
        state: state.clone(),
    };

    // 创建SSE流，处理连接断开
    let sse_stream: std::pin::Pin<Box<dyn Stream<Item = Result<Event, std::convert::Infallible>> + Send>> = Box::pin(async_stream::stream! {
        // Move the guard into the stream closure so it's dropped when the stream ends
        let _guard = guard;
        
        loop {
            tokio::select! {
                // 监听广播消息
                result = receiver.recv() => {
                    match result {
                        Ok(value) => {
                            yield Ok(Event::default().json_data(value).unwrap_or_else(|_| Event::default().data("Error serializing data")));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // 发送者已被关闭
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            // 消息滞后，继续接收
                            continue;
                        }
                    }
                }
                // 检查连接是否仍然活跃（通过尝试接收 a message）
                _ = tokio::time::sleep(std::time::Duration::from_secs(30)) => {
                    // 发送心跳消息
                    let _ = sender.send(serde_json::json!({"type": "ping", "timestamp": chrono::Local::now().timestamp()}));
                }
            }
        }
        
        // The stream is ending, the guard will be dropped and cleanup will happen automatically
    });

    // 返回SSE响应
    Sse::new(sse_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

// 发送弹幕端点
pub async fn send_danmu_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HashMap<String, String>>,
    Json(mut danmu_data): Json<DanmuData>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // 可选 token 鉴权
    {
        let auth = state.auth.read().await.clone();
        check_token(&auth, &query)?;
    }

    // 验证必要字段
    if danmu_data.text.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "缺少text字段".to_string()));
    }
    
    // 设置时间戳
    danmu_data.time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    danmu_data.timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

    // 发送到所有连接
    send_to_all_connections(&state, serde_json::to_value(danmu_data).unwrap()).await;
    
    // 更新统计
    {
        let mut stats = state.stats.write().await;
        stats.danmu_count += 1;
        stats.last_activity = Some(std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64);
    }
    
    Ok(Json(serde_json::json!({
        "success": true,
        "message": "弹幕发送成功"
    })))
}

// 获取状态端点
pub async fn status_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HashMap<String, String>>,
) -> Json<serde_json::Value> {
    // 可选 token 鉴权（状态接口失败时直接返回空状态，避免把 handler 改成 Result）
    {
        let auth = state.auth.read().await.clone();
        if check_token(&auth, &query).is_err() {
            return Json(serde_json::json!({"error": "unauthorized"}));
        }
    }

    let connections = state.sse_connections.read().await.len();
    let stats = state.stats.read().await.clone();
    
    Json(serde_json::json!({
        "connections": connections,
        "danmu_count": stats.danmu_count,
        "last_activity": stats.last_activity,
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }))
}

// 更新配置端点
pub async fn update_config_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HashMap<String, String>>,
    Json(config_data): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (axum::http::StatusCode, String)> {
    // 可选 token 鉴权
    {
        let auth = state.auth.read().await.clone();
        check_token(&auth, &query)?;
    }

    if let Some(config_obj) = config_data.get("config") {
        if let Ok(new_config) = serde_json::from_value::<Config>(config_obj.clone()) {
            // 更新全局配置
            *state.config.write().await = new_config.clone();
            
            // 发送配置更新到所有连接
            let config_msg = serde_json::json!({
                "type": "config",
                "config": new_config
            });
            
            send_to_all_connections(&state, config_msg).await;
            
            return Ok(Json(serde_json::json!({
                "success": true,
                "message": "配置更新成功"
            })));
        } else {
            return Err((axum::http::StatusCode::BAD_REQUEST, "配置格式错误".to_string()));
        }
    }
    
    Err((axum::http::StatusCode::BAD_REQUEST, "缺少config字段".to_string()))
}

// 内部函数：发送消息到所有连接
pub async fn send_to_all_connections(state: &Arc<AppState>, msg: serde_json::Value) {
    println!("[弹幕] {}", serde_json::to_string_pretty(&msg).unwrap_or_default());
    let connections = state.sse_connections.read().await;
    let connection_ids: Vec<String> = connections.keys().cloned().collect();
    
    for id in connection_ids {
        if let Some(sender) = connections.get(&id) {
            let _ = sender.send(msg.clone());
        }
    }
}

// 创建Axum应用
pub fn create_app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/preview.html", get(preview_html_handler))
        .route("/api/sse", get(sse_handler))
        .route("/api/send-danmu", post(send_danmu_handler))
        .route("/api/status", get(status_handler))
        .route("/api/config", post(update_config_handler))
        .fallback_service(ServeDir::new("../frontend/public"))
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods(tower_http::cors::Any)
                .allow_headers(tower_http::cors::Any)
        )
        .with_state(state)
}