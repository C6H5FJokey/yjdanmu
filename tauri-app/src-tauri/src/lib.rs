use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
use tauri::Manager;

mod sse_server;
pub mod bili_websocket_client;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
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
                start_sse_server(app_handle).await;
            });
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            start_sse_server_cmd,
            send_danmu,
            send_config,
            get_status,
            open_in_browser,
            toggle_always_on_top,
            connect_websocket,
            disconnect_websocket
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// SSE服务器状态
static mut SSE_SERVER_STATE: Option<Arc<sse_server::AppState>> = None;

async fn start_sse_server(_app_handle: tauri::AppHandle) {
    use tokio::net::TcpListener;
    
    let state = Arc::new(sse_server::AppState {
        sse_connections: Arc::new(RwLock::new(std::collections::HashMap::new())),
        stats: Arc::new(RwLock::new(sse_server::Stats::default())),
        config: Arc::new(RwLock::new(sse_server::Config::default())),
    });
    
    // 保存全局状态
    unsafe {
        SSE_SERVER_STATE = Some(state.clone());
    }
    
    let app = sse_server::create_app(state);
    
    let listener = TcpListener::bind("127.0.0.1:8081").await.unwrap();
    println!("SSE服务器启动在 http://127.0.0.1:8081");
    
    axum::serve(listener, app).await.unwrap();
}

#[tauri::command]
async fn start_sse_server_cmd() -> Result<String, String> {
    // 服务器在setup阶段已经启动，这里只是返回状态
    Ok("SSE服务器已启动".to_string())
}

#[tauri::command]
async fn send_danmu(text: String, custom_data: Option<serde_json::Value>) -> Result<String, String> {
    if let Some(state) = unsafe { SSE_SERVER_STATE.as_ref() } {
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
        
        // 在控制台打印发送的弹幕信息
        println!("[弹幕] {}", serde_json::to_string_pretty(&json_data).unwrap_or_default());
        sse_server::send_to_all_connections(state, json_data).await;
        
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
    if let Some(state) = unsafe { SSE_SERVER_STATE.as_ref() } {
        // 更新全局配置
        *state.config.write().await = config.clone();
        
        // 发送配置更新到所有连接
        let config_msg = serde_json::json!({
            "type": "config",
            "config": config
        });
        
        sse_server::send_to_all_connections(state, config_msg).await;
        
        Ok("配置更新成功".to_string())
    } else {
        Err("SSE服务器未启动".to_string())
    }
}

#[tauri::command]
async fn get_status() -> Result<serde_json::Value, String> {
    if let Some(state) = unsafe { SSE_SERVER_STATE.as_ref() } {
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
