use std::{io::Read, sync::Arc, time::{Duration, SystemTime, UNIX_EPOCH}};

use brotli::Decompressor; // 新增
use flate2::read::ZlibDecoder;
use futures_util::{stream::StreamExt, SinkExt};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::{sync::RwLock, task::JoinHandle};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{self, Message},
};
use std::sync::atomic::{AtomicU8, Ordering};

use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DanmuFilterConfig {
    pub enabled: bool,
    pub keyword_blacklist: Vec<String>,
    pub min_len: Option<u32>,
    pub max_len: Option<u32>,
    // 仅显示佩戴本房间粉丝牌的弹幕（media_ruid 与主播 uid 匹配）
    pub only_fans_medal: bool,
    // 仅显示主播/本人弹幕（通过 face_url 匹配，免登录场景下依赖 face_url）
    pub only_streamer: bool,
    // 屏蔽主播/本人弹幕
    pub hide_streamer: bool,
}

impl Default for DanmuFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            keyword_blacklist: Vec::new(),
            min_len: None,
            max_len: None,
            only_fans_medal: false,
            only_streamer: false,
            hide_streamer: false,
        }
    }
}

// 0 = follow env, 1 = force false, 2 = force true
static WS_DEBUG_OVERRIDE: AtomicU8 = AtomicU8::new(0);
static DANMU_FILTER: Lazy<Arc<RwLock<DanmuFilterConfig>>> = Lazy::new(|| Arc::new(RwLock::new(DanmuFilterConfig::default())));

pub async fn set_ws_debug_enabled(enabled: bool) {
    WS_DEBUG_OVERRIDE.store(if enabled { 2 } else { 1 }, Ordering::Relaxed);
}

pub async fn set_danmu_filter_config(cfg: DanmuFilterConfig) {
    *DANMU_FILTER.write().await = cfg;
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn now_hms() -> String {
    // 与 sse_server.rs 的默认 timestamp 格式保持一致
    chrono::Local::now().format("%H:%M:%S").to_string()
}

fn hms_from_unix_ms(ms: u64) -> String {
    use chrono::TimeZone;

    // B 站 DANMU_MSG 的 timestamp 为 Unix 毫秒时间戳
    chrono::Local
        .timestamp_millis_opt(ms as i64)
        .single()
        .map(|dt| dt.format("%H:%M:%S").to_string())
        .unwrap_or_else(now_hms)
}

fn rgb_decimal_to_hex(v: u64) -> String {
    // B 站 DANMU_MSG 的颜色字段通常是十进制 RGB
    format!("#{:06x}", (v & 0x00ff_ffff))
}

fn make_sse_event(
    msg_type: &'static str,
    text: String,
    user: Option<String>,
    color: Option<String>,
    size: Option<u32>,
    face_url: Option<String>,
    media_ruid: Option<u32>,
    time_ms: Option<u64>,
) -> serde_json::Value {
    let timestamp = time_ms.unwrap_or_else(now_ms);
    let timestamp_text = hms_from_unix_ms(timestamp);
    serde_json::json!({
        "type": msg_type,
        "text": text,
        "user": user.unwrap_or_else(|| "匿名用户".to_string()),
        "color": color.unwrap_or_else(|| "#ffffff".to_string()),
        "size": size.unwrap_or(32),
        // 兼容字段：同样使用 Unix 毫秒时间戳
        "time": timestamp,
        // 对齐 blivedm/web.py: timestamp 是 Unix 毫秒时间戳
        "timestamp": timestamp,
        // 仅用于展示的人类可读时间（不要当作 timestamp 使用）
        "timestampText": timestamp_text,
        "face_url": face_url,
        "media_ruid": media_ruid,
        // 样式/过滤辅助字段（缺省由 forward_to_sse 兜底补齐）
        "hasOwnMedal": false,
        "isModerator": false,
        // 舰队等级：0非舰队, 1总督, 2提督, 3舰长（blivedm/web.py: privilege_type）
        "guardLevel": 0,
    })
}

fn make_sse_danmu(
    text: String,
    user: Option<String>,
    color: Option<String>,
    size: Option<u32>,
    face_url: Option<String>,
    media_ruid: Option<u32>,
    time_ms: Option<u64>,
) -> serde_json::Value {
    make_sse_event("danmu", text, user, color, size, face_url, media_ruid, time_ms)
}

fn parse_danmu_msg(root: &serde_json::Value) -> Option<serde_json::Value> {
    // 典型结构：{"cmd":"DANMU_MSG", "info": [meta, text, user, ...]}
    
    let text = root.pointer("/info/1")?.as_str()?.to_string();

    // web 协议：info[0][4] 是 Unix 毫秒时间戳（blivedm 的 DanmakuMessage.timestamp）
    let time_ms = root
        .pointer("/info/0/4")
        .and_then(|v| v.as_u64());

    let user = root
        .pointer("/info/2/1")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let face_url = root
        .pointer("/info/0/15/user/base/face")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    let media_ruid = root
        .pointer("/info/0/15/user/medal/ruid")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());

    let color = root
        .pointer("/info/0/3")
        .and_then(|v| v.as_u64())
        .map(rgb_decimal_to_hex);

    let size = root
        .pointer("/info/0/2")
        .and_then(|v| v.as_u64())
        .and_then(|v| u32::try_from(v).ok());

    // 尝试解析房管标记（不同地区/模式下字段可能缺失；缺失则按 false 处理）
    let is_moderator = root
        .pointer("/info/2/2")
        .and_then(|v| v.as_i64())
        .map(|v| v == 1)
        .unwrap_or(false);

    // 舰队类型（blivedm/web.py: privilege_type）
    let guard_level = root
        .pointer("/info/7")
        .and_then(|v| v.as_i64())
        .unwrap_or(0)
        .clamp(0, 3);

    let mut msg = make_sse_danmu(text, user, color, size, face_url, media_ruid, time_ms);
    if let Some(obj) = msg.as_object_mut() {
        obj.insert("isModerator".to_string(), serde_json::Value::Bool(is_moderator));
        obj.insert(
            "guardLevel".to_string(),
            serde_json::Value::Number(serde_json::Number::from(guard_level)),
        );
    }
    Some(msg)
}

fn parse_open_live_danmaku(root: &serde_json::Value) -> Option<serde_json::Value> {
    // 常见结构：{"cmd":"OPEN_LIVE_DANMAKU","data":{"uname":"xx","msg":"yy", ...}}
    let data = root.get("data")?;
    let text = data.get("msg")?.as_str()?.to_string();
    let user = data
        .get("uname")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    Some(make_sse_event("danmu", text, user, None, None, None, None, None))
}

fn parse_open_live_gift(root: &serde_json::Value) -> Option<serde_json::Value> {
    // 常见结构：{"cmd":"OPEN_LIVE_GIFT","data":{"uname":"xx","gift_name":"xx","gift_num":1}}
    let data = root.get("data")?;
    let user = data
        .get("uname")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let gift_name = data.get("gift_name").and_then(|v| v.as_str()).unwrap_or("礼物");
    let gift_num = data.get("gift_num").and_then(|v| v.as_u64()).unwrap_or(1);
    let text = format!("送出 {gift_name} x{gift_num}");
    Some(make_sse_event("gift", text, user, None, None, None, None, None))
}

fn parse_open_live_super_chat(root: &serde_json::Value) -> Option<serde_json::Value> {
    // 常见结构：{"cmd":"OPEN_LIVE_SUPER_CHAT","data":{"uname":"xx","message":"yy"}}
    let data = root.get("data")?;
    let user = data
        .get("uname")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let message = data
        .get("message")
        .or_else(|| data.get("msg"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let text = if message.is_empty() {
        "醒目留言".to_string()
    } else {
        format!("醒目留言：{message}")
    };
    Some(make_sse_event("superChat", text, user, None, None, None, None, None))
}

const BILI_BROADCAST_WS: &str = "wss://broadcastlv.chat.bilibili.com/sub";

const ROOM_INIT_URL: &str = "https://api.live.bilibili.com/room/v1/Room/get_info";
const BUVID_INIT_URL: &str = "https://www.bilibili.com/";
const WBI_INIT_URL: &str = "https://api.bilibili.com/x/web-interface/nav";
const DANMAKU_SERVER_CONF_URL: &str = "https://api.live.bilibili.com/xlive/web-room/v1/index/getDanmuInfo";

const HEADER_LEN: u16 = 16;
// 与 blivedm 的 ws_base.py 一致：客户端发包 ver 固定为 1（不是压缩协议版本）
const PROTO_VER_CLIENT: u16 = 1;
const PROTO_VER_ZLIB: u16 = 2;
const PROTO_VER_BROTLI: u16 = 3;
const OP_HEARTBEAT: u32 = 2;
const OP_HEARTBEAT_REPLY: u32 = 3;
const OP_MESSAGE: u32 = 5;
const OP_AUTH: u32 = 7;
const OP_AUTH_REPLY: u32 = 8;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RoomKeyType {
    RoomId,   // 公屏房间直连
    AuthCode, // 开放平台用户码
}

#[derive(Debug, Default)]
struct WsState {
    handle: Option<JoinHandle<()>>,
    should_stop: bool,
    app_handle: Option<AppHandle>,

    // 当前连接的房间标识（用于按房间样式配置）
    current_room_key: Option<String>,
    current_room_key_type: Option<RoomKeyType>,

    // roomid 模式下用于弹幕过滤（免登录）：主播 uid 与 face_url
    room_owner_uid: Option<i64>,
    room_owner_face_url: Option<String>,
}

static WS_STATE: Lazy<Arc<RwLock<WsState>>> = Lazy::new(|| Arc::new(RwLock::new(WsState::default())));

const BILI_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/102.0.0.0 Safari/537.36";

#[derive(Clone, Debug)]
struct DanmakuHost {
    host: String,
    wss_port: u16,
}

#[derive(Clone, Debug)]
struct RoomIdConnInfo {
    room_id: i64,
    ws_url: String,
    token: Option<String>,
    buvid: Option<String>,

    owner_uid: Option<i64>,
    owner_face_url: Option<String>,
}

async fn resolve_room_info(tmp_room_id: i64) -> (i64, Option<i64>) {
    #[derive(Deserialize)]
    struct Resp {
        code: i64,
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        room_id: i64,
        uid: Option<i64>,
    }

    let client = reqwest::Client::new();
    let url = format!("{ROOM_INIT_URL}?room_id={tmp_room_id}");
    let res = client.get(url).header("User-Agent", BILI_USER_AGENT).send().await;

    match res {
        Ok(r) => {
            if !r.status().is_success() {
                if ws_debug_enabled() {
                    eprintln!(
                        "[WebSocket][DEBUG] resolve_room_id http status={} (fallback tmp={tmp_room_id})",
                        r.status().as_u16()
                    );
                }
                return (tmp_room_id, None);
            }

            match r.json::<Resp>().await {
            Ok(parsed) => {
                if parsed.code == 0 {
                    if let Some(d) = parsed.data {
                        if ws_debug_enabled() {
                            eprintln!("[WebSocket][DEBUG] resolve_room_id tmp={tmp_room_id} real={} uid={:?}", d.room_id, d.uid);
                        }
                        return (d.room_id, d.uid);
                    }
                }
                (tmp_room_id, None)
            }
            Err(_) => (tmp_room_id, None),
            }
        }
        Err(e) => {
            if ws_debug_enabled() {
                eprintln!("[WebSocket][DEBUG] resolve_room_id request failed: {e:?} (fallback tmp={tmp_room_id})");
            }
            (tmp_room_id, None)
        }
    }
}

async fn fetch_face_url_by_mid(mid: i64) -> Option<String> {
    #[derive(Deserialize)]
    struct Resp {
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        face: Option<String>,
    }

    let client = reqwest::Client::new();
    let url = format!("https://api.bilibili.com/x/web-interface/card?mid={mid}");
    let res = client
        .get(url)
        .header("User-Agent", BILI_USER_AGENT)
        .send()
        .await
        .ok()?;
    let parsed: Resp = res.json().await.ok()?;
    parsed.data.and_then(|d| d.face)
}

async fn fetch_buvid() -> Option<String> {
    let client = reqwest::Client::new();
    let res = client
        .get(BUVID_INIT_URL)
        .header("User-Agent", BILI_USER_AGENT)
        .send()
        .await
        .map_err(|e| {
            if ws_debug_enabled() {
                eprintln!("[WebSocket][DEBUG] fetch_buvid request failed: {e:?}");
            }
            e
        })
        .ok()?;

    if ws_debug_enabled() && !res.status().is_success() {
        eprintln!(
            "[WebSocket][DEBUG] fetch_buvid http status={} (buvid may be empty)",
            res.status().as_u16()
        );
    }

    for value in res.headers().get_all("set-cookie") {
        if let Ok(s) = value.to_str() {
            // 只需要 buvid3（Python 版也是用它）
            if let Some(rest) = s.strip_prefix("buvid3=") {
                let buvid = rest.split(';').next().unwrap_or("");
                if !buvid.is_empty() {
                    if ws_debug_enabled() {
                        eprintln!("[WebSocket][DEBUG] buvid3={buvid}");
                    }
                    return Some(buvid.to_string());
                }
            }
        }
    }
    None
}

async fn fetch_wbi_key() -> Result<String, String> {
    #[derive(Deserialize)]
    struct Resp {
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        wbi_img: WbiImg,
    }
    #[derive(Deserialize)]
    struct WbiImg {
        img_url: String,
        sub_url: String,
    }

    // 与 Python 版一致的索引表
    const WBI_KEY_INDEX_TABLE: [usize; 32] = [
        46, 47, 18, 2, 53, 8, 23, 32, 15, 50, 10, 31, 58, 3, 45, 35, 27, 43, 5, 49, 33, 9,
        42, 19, 29, 28, 14, 39, 12, 38, 41, 13,
    ];

    let client = reqwest::Client::new();
    let res = client
        .get(WBI_INIT_URL)
        .header("User-Agent", BILI_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("wbi 请求失败: {e:?}"))?;

    if !res.status().is_success() {
        return Err(format!(
            "wbi 请求失败: status={}",
            res.status().as_u16()
        ));
    }

    let parsed: Resp = res
        .json()
        .await
        .map_err(|e| format!("wbi 解析失败: {e:?}"))?;
    let wbi_img = parsed
        .data
        .ok_or_else(|| "wbi data 缺失".to_string())?
        .wbi_img;

    fn extract_key(url: &str) -> String {
        let file = url.rsplit('/').next().unwrap_or("");
        file.split('.').next().unwrap_or("").to_string()
    }

    let img_key = extract_key(&wbi_img.img_url);
    let sub_key = extract_key(&wbi_img.sub_url);
    let shuffled = img_key + &sub_key;

    let mut out = String::new();
    for idx in WBI_KEY_INDEX_TABLE {
        if idx < shuffled.len() {
            out.push(shuffled.as_bytes()[idx] as char);
        }
    }
    if ws_debug_enabled() {
        eprintln!("[WebSocket][DEBUG] wbi_key={out}");
    }
    Ok(out)
}

fn wbi_add_sign(mut params: Vec<(String, String)>, wbi_key: &str) -> Vec<(String, String)> {
    use url::form_urlencoded;

    let wts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .to_string();

    let mut params_to_sign = params.clone();
    params_to_sign.push(("wts".to_string(), wts.clone()));

    params_to_sign.sort_by(|a, b| a.0.cmp(&b.0));
    for (_, v) in params_to_sign.iter_mut() {
        *v = v
            .chars()
            .filter(|ch| !matches!(ch, '!' | '\'' | '(' | ')' | '*'))
            .collect();
    }

    let mut ser = form_urlencoded::Serializer::new(String::new());
    for (k, v) in &params_to_sign {
        ser.append_pair(k, v);
    }
    let str_to_sign = ser.finish() + wbi_key;
    let w_rid = format!("{:x}", md5::compute(str_to_sign.as_bytes()));

    params.push(("wts".to_string(), wts));
    params.push(("w_rid".to_string(), w_rid));
    params
}

async fn fetch_danmaku_server(room_id: i64, wbi_key: &str) -> Result<(Vec<DanmakuHost>, Option<String>), String> {
    #[derive(Deserialize)]
    struct Resp {
        code: i64,
        message: Option<String>,
        data: Option<Data>,
    }
    #[derive(Deserialize)]
    struct Data {
        host_list: Vec<Host>,
        token: Option<String>,
    }
    #[derive(Deserialize)]
    struct Host {
        host: String,
        wss_port: u16,
    }

    use url::form_urlencoded;

    let client = reqwest::Client::new();
    let params = vec![
        ("id".to_string(), room_id.to_string()),
        ("type".to_string(), "0".to_string()),
    ];
    let signed = wbi_add_sign(params, wbi_key);

    let query = {
        let mut ser = form_urlencoded::Serializer::new(String::new());
        for (k, v) in &signed {
            ser.append_pair(k, v);
        }
        ser.finish()
    };
    let url = format!("{DANMAKU_SERVER_CONF_URL}?{query}");

    let res = client
        .get(url)
        .header("User-Agent", BILI_USER_AGENT)
        .send()
        .await
        .map_err(|e| format!("getDanmuInfo 请求失败: {e:?}"))?;

    if !res.status().is_success() {
        return Err(format!(
            "getDanmuInfo 请求失败: status={} ",
            res.status().as_u16()
        ));
    }

    let parsed: Resp = res
        .json()
        .await
        .map_err(|e| format!("getDanmuInfo 解析失败: {e:?}"))?;
    if parsed.code != 0 {
        return Err(format!(
            "getDanmuInfo code={} message={}",
            parsed.code,
            parsed.message.unwrap_or_default()
        ));
    }
    let data = parsed.data.ok_or_else(|| "getDanmuInfo data 缺失".to_string())?;
    let hosts = data
        .host_list
        .into_iter()
        .map(|h| DanmakuHost {
            host: h.host,
            wss_port: h.wss_port,
        })
        .collect::<Vec<_>>();

    if ws_debug_enabled() {
        eprintln!("[WebSocket][DEBUG] getDanmuInfo host_list={} token={}", hosts.len(), if data.token.as_deref().unwrap_or("").is_empty() {"N"} else {"Y"});
    }

    Ok((hosts, data.token))
}

async fn prepare_roomid_conn(tmp_room_id: i64) -> RoomIdConnInfo {
    let (room_id, owner_uid) = resolve_room_info(tmp_room_id).await;
    let buvid = fetch_buvid().await;

    let owner_face_url = match owner_uid {
        Some(uid) => fetch_face_url_by_mid(uid).await,
        std::option::Option::None => std::option::Option::None,
    };

    // 获取弹幕服务器列表与 token（失败则回退到默认 broadcastlv）
    let (ws_url, token) = match fetch_wbi_key().await {
        Ok(wbi_key) => match fetch_danmaku_server(room_id, &wbi_key).await {
            Ok((hosts, token)) if !hosts.is_empty() => {
                // python 版是 host_list[retry % len]
                let h = &hosts[0];
                (format!("wss://{}:{}/sub", h.host, h.wss_port), token)
            }
            Ok(_) => (BILI_BROADCAST_WS.to_string(), None),
            Err(e) => {
                if ws_debug_enabled() {
                    eprintln!("[WebSocket][DEBUG] getDanmuInfo failed: {e}, fallback to {BILI_BROADCAST_WS}");
                }
                (BILI_BROADCAST_WS.to_string(), None)
            }
        },
        Err(e) => {
            if ws_debug_enabled() {
                eprintln!("[WebSocket][DEBUG] fetch_wbi_key failed: {e}, fallback to {BILI_BROADCAST_WS}");
            }
            (BILI_BROADCAST_WS.to_string(), None)
        }
    };

    RoomIdConnInfo {
        room_id,
        ws_url,
        token,
        buvid,

        owner_uid,
        owner_face_url,
    }
}

fn ws_debug_enabled() -> bool {
    match WS_DEBUG_OVERRIDE.load(Ordering::Relaxed) {
        1 => return false,
        2 => return true,
        _ => {}
    }
    match std::env::var("BILI_WS_DEBUG") {
        Ok(v) => matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"),
        Err(_) => false,
    }
}

fn get_text_len(text: &str) -> u32 {
    text.chars().count() as u32
}

async fn should_forward_danmu(msg: &serde_json::Value) -> bool {
    let cfg = DANMU_FILTER.read().await.clone();
    if !cfg.enabled {
        return true;
    }

    let text = msg.get("text").and_then(|v| v.as_str()).unwrap_or("");
    let text_len = get_text_len(text);

    if let Some(min_len) = cfg.min_len {
        if text_len < min_len {
            return false;
        }
    }
    if let Some(max_len) = cfg.max_len {
        if text_len > max_len {
            return false;
        }
    }

    if !cfg.keyword_blacklist.is_empty() {
        for kw in &cfg.keyword_blacklist {
            let kw = kw.trim();
            if !kw.is_empty() && text.contains(kw) {
                return false;
            }
        }
    }

    let (owner_uid, owner_face) = {
        let guard = WS_STATE.read().await;
        (guard.room_owner_uid, guard.room_owner_face_url.clone())
    };

    if cfg.only_fans_medal {
        let Some(owner_uid) = owner_uid else {
            return false;
        };
        let owner_uid_str = owner_uid.to_string();
        let media_ruid = msg.get("media_ruid").and_then(|v| v.as_str()).unwrap_or("");
        if media_ruid != owner_uid_str {
            return false;
        }
    }

    if cfg.only_streamer || cfg.hide_streamer {
        let Some(owner_face) = owner_face else {
            // 无法获取主播 face_url 时，为避免误判：only_streamer 直接过滤掉；hide_streamer 则不处理
            return !cfg.only_streamer;
        };
        let face_url = msg.get("face_url").and_then(|v| v.as_str()).unwrap_or("");
        let is_streamer = !face_url.is_empty() && face_url == owner_face;
        if cfg.only_streamer && !is_streamer {
            return false;
        }
        if cfg.hide_streamer && is_streamer {
            return false;
        }
    }

    true
}

fn debug_dump_packet(prefix: &str, buf: impl AsRef<[u8]>) {
    if !ws_debug_enabled() {
        return;
    }
    let buf = buf.as_ref();
    if buf.len() < HEADER_LEN as usize {
        eprintln!("[WebSocket][DEBUG] {prefix}: len={} (< header)", buf.len());
        return;
    }
    let packet_len = u32::from_be_bytes(buf[0..4].try_into().unwrap());
    let header_len = u16::from_be_bytes(buf[4..6].try_into().unwrap());
    let ver = u16::from_be_bytes(buf[6..8].try_into().unwrap());
    let op = u32::from_be_bytes(buf[8..12].try_into().unwrap());
    let seq = u32::from_be_bytes(buf[12..16].try_into().unwrap());
    let preview_len = buf.len().min(64);
    eprintln!(
        "[WebSocket][DEBUG] {prefix}: packet_len={packet_len} header_len={header_len} ver={ver} op={op} seq={seq} raw_len={} head={:02x?}",
        buf.len(),
        &buf[..preview_len]
    );
}

async fn connect_with_ua(url: &str) -> Result<(tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>, tungstenite::handshake::client::Response), tungstenite::Error> {
    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    use tungstenite::http::header::{HeaderValue, ORIGIN, USER_AGENT};

    let mut req = url.into_client_request()?;
    // python 版固定设置 UA（web token 也会签 UA），这里保持一致
    req.headers_mut()
        .insert(USER_AGENT, HeaderValue::from_static(BILI_USER_AGENT));
    // 对部分地区/网络策略，Origin 缺失可能导致服务端直接 reset
    req.headers_mut()
        .insert(ORIGIN, HeaderValue::from_static("https://live.bilibili.com"));
    if ws_debug_enabled() {
        eprintln!("[WebSocket][DEBUG] ws_connect url={url}");
        eprintln!("[WebSocket][DEBUG] request headers={:?}", req.headers());
    }
    let (ws, resp) = connect_async(req).await?;
    if ws_debug_enabled() {
        eprintln!("[WebSocket][DEBUG] handshake status={}", resp.status());
        eprintln!("[WebSocket][DEBUG] response headers={:?}", resp.headers());
    }
    Ok((ws, resp))
}


pub async fn connect_websocket(
    app_handle: AppHandle,
    room_key: String,
    room_key_type: RoomKeyType,
    reconnect_interval: u64,
    max_reconnect_attempts: u32,
    open_live_app_id: Option<i64>,
    open_live_access_key_id: Option<String>,
    open_live_access_key_secret: Option<String>,
) -> Result<(), String> {
    {
        // 关键点：确保 should_stop 被重置。
        // 之前 disconnect 使用 abort，run_ws_loop 的 finally 不会执行，should_stop 会一直为 true，
        // 导致下一次 connect 后任务立刻退出且不再发事件，前端就“看起来连不上”。
        let mut guard = WS_STATE.write().await;

        if let Some(h) = guard.handle.as_ref() {
            if h.is_finished() {
                guard.handle = None;
            }
        }
        if guard.handle.is_some() {
            return Err("已有连接".to_string());
        }

        guard.should_stop = false;
        guard.app_handle = Some(app_handle.clone());
        guard.current_room_key = Some(room_key.clone());
        guard.current_room_key_type = Some(room_key_type);
    }

    emit_status("connecting", "连接中...").await;

    let handle = tokio::spawn(run_ws_loop(
        room_key,
        room_key_type,
        reconnect_interval,
        max_reconnect_attempts,
        open_live_app_id,
        open_live_access_key_id,
        open_live_access_key_secret,
    ));

    {
        let mut guard = WS_STATE.write().await;
        guard.handle = Some(handle);
    }
    Ok(())
}

pub async fn disconnect_websocket() -> Result<(), String> {
    let handle = {
        let mut guard = WS_STATE.write().await;
        guard.should_stop = true;
        guard.handle.take()
    };
    if let Some(h) = handle {
        h.abort();
        let _ = h.await;
    }

    {
        // abort 会跳过 run_ws_loop 的清理代码，因此这里必须兜底恢复状态
        let mut guard = WS_STATE.write().await;
        guard.should_stop = false;
        guard.handle = None;
        guard.room_owner_uid = None;
        guard.room_owner_face_url = None;
    }

    emit_status("disconnected", "已断开连接").await;
    Ok(())
}

async fn run_ws_loop(
    room_key: String,
    room_key_type: RoomKeyType,
    reconnect_interval: u64,
    max_reconnect_attempts: u32,
    open_live_app_id: Option<i64>,
    open_live_access_key_id: Option<String>,
    open_live_access_key_secret: Option<String>,
) {
    let mut attempts = 0;
    let reconnect_dur = Duration::from_millis(reconnect_interval.max(1000));

    while attempts <= max_reconnect_attempts {
        {
            let guard = WS_STATE.read().await;
            if guard.should_stop {
                break;
            }
        }

        let ws_result = match room_key_type {
            RoomKeyType::RoomId => {
                let tmp_room_id: i64 = match room_key.parse() {
                    Ok(id) => id,
                    Err(_) => {
                        eprintln!("[WebSocket] room_id 不是数字: {}", room_key);
                        emit_status("error", "房间号不是数字").await;
                        return;
                    }
                };
                let info = prepare_roomid_conn(tmp_room_id).await;
                let ws_url = info.ws_url.clone();
                connect_with_ua(&ws_url)
                    .await
                    .map(|(ws, resp)| (ws, resp, Some(info), None))
            }
            RoomKeyType::AuthCode => match start_game_and_get_ws(
                &room_key,
                open_live_app_id,
                open_live_access_key_id.clone(),
                open_live_access_key_secret.clone(),
            )
            .await {
                Ok(info) => connect_with_ua(&info.ws_url)
                    .await
                    .map(|(ws, resp)| (ws, resp, None, Some(info.auth_body))),
                Err(e) => {
                    eprintln!("[WebSocket] start_game 失败: {e}");
                    emit_status("error", &format!("鉴权失败: {}", e)).await;
                    return;
                }
            },
        };

        match ws_result {
            Ok((ws_stream, _resp, maybe_room_info, maybe_auth_body)) => {
                println!("[WebSocket] 连接成功");
                emit_status("connected", "连接成功").await;
                attempts = 0;
                let (mut write, mut read) = ws_stream.split();

                // roomid 模式下保存主播 uid/face_url，用于后续弹幕过滤
                if let RoomKeyType::RoomId = room_key_type {
                    if let Some(info) = maybe_room_info.as_ref() {
                        let mut guard = WS_STATE.write().await;
                        guard.room_owner_uid = info.owner_uid;
                        guard.room_owner_face_url = info.owner_face_url.clone();
                    }
                }

                // 鉴权
                match room_key_type {
                    RoomKeyType::RoomId => {
                        let info = maybe_room_info.expect("房间连接信息缺失");
                        let mut auth_body = serde_json::json!({
                            "uid": 0,
                            "roomid": info.room_id,
                            "protover": 3,
                            "platform": "web",
                            "type": 2,
                            "buvid": info.buvid.clone().unwrap_or_default(),
                        });
                        if let Some(token) = info.token.clone() {
                            if let Some(obj) = auth_body.as_object_mut() {
                                obj.insert("key".to_string(), serde_json::Value::String(token));
                            }
                        }
                        if ws_debug_enabled() {
                            eprintln!("[WebSocket][DEBUG] auth json={}", auth_body);
                        }
                        // 注意：header 的 ver 必须为 1；protover=3 走在 JSON body 里
                        let msg = make_packet(auth_body.to_string().into_bytes(), OP_AUTH, PROTO_VER_CLIENT);
                        if let Message::Binary(b) = &msg {
                            debug_dump_packet("send auth(roomid)", b);
                        }
                        let _ = write.send(msg).await;
                    }
                    RoomKeyType::AuthCode => {
                        if let Some(auth_body) = maybe_auth_body.clone() {
                            let msg = make_packet(auth_body.into_bytes(), OP_AUTH, PROTO_VER_CLIENT);
                            if let Message::Binary(b) = &msg {
                                debug_dump_packet("send auth(authcode)", b);
                            }
                            let _ = write.send(msg).await;
                        }
                    }
                }

                let mut heartbeat = tokio::time::interval(Duration::from_secs(20));

                loop {
                    tokio::select! {
                        _ = heartbeat.tick() => {
                            let msg = make_packet(Vec::new(), OP_HEARTBEAT, PROTO_VER_CLIENT);
                            if let Message::Binary(b) = &msg {
                                debug_dump_packet("send heartbeat", b);
                            }
                            if let Err(e) = write.send(msg).await {
                                eprintln!("[WebSocket] 心跳失败: {e}");
                                emit_status("disconnected", "心跳失败").await;
                                break;
                            }
                        }
                        msg = read.next() => {
                            match msg {
                                Some(Ok(Message::Binary(bin))) => {
                                    debug_dump_packet("recv frame", &bin);
                                    handle_packet(bin.as_ref()).await
                                }
                                Some(Ok(Message::Text(txt))) => { eprintln!("[WebSocket] 收到文本帧（异常）: {txt}"); }
                                Some(Ok(Message::Close(frame))) => { 
                                    eprintln!("[WebSocket] 服务端关闭: {:?}", frame); 
                                    emit_status("disconnected", "服务端关闭").await;
                                    break; 
                                }
                                Some(Ok(Message::Ping(_))) => {}
                                Some(Ok(Message::Pong(_))) => {}
                                Some(Ok(Message::Frame(_))) => {}
                                Some(Err(e)) => { 
                                    eprintln!("[WebSocket] 读取错误: {e}"); 
                                    emit_status("disconnected", &format!("读取错误: {}", e)).await;
                                    break; 
                                }
                                std::option::Option::None => { 
                                    println!("[WebSocket] 连接结束"); 
                                    emit_status("disconnected", "连接结束").await;
                                    break; 
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                attempts += 1;
                emit_status("reconnecting", &format!("重连中 {}/{}", attempts, max_reconnect_attempts)).await;
            }
        }

        if attempts > max_reconnect_attempts {
            eprintln!("[WebSocket] 达到最大重连次数，停止");
            break;
        }
        tokio::time::sleep(reconnect_dur).await;
        println!("[WebSocket] 重连中 {attempts}/{max_reconnect_attempts} …");
    }

    {
        let mut guard = WS_STATE.write().await;
        guard.should_stop = false;
        guard.handle = None;
    }
    println!("[WebSocket] 任务结束");
}

fn make_packet(body: Vec<u8>, op: u32, proto_ver: u16) -> Message {
    let packet_len = HEADER_LEN as u32 + body.len() as u32;
    let mut buf = Vec::with_capacity(packet_len as usize);
    buf.extend_from_slice(&packet_len.to_be_bytes());
    buf.extend_from_slice(&HEADER_LEN.to_be_bytes());
    buf.extend_from_slice(&proto_ver.to_be_bytes());
    buf.extend_from_slice(&op.to_be_bytes());
    buf.extend_from_slice(&1u32.to_be_bytes()); // seq
    buf.extend_from_slice(&body);
    Message::Binary(buf.into())
}

async fn handle_packet(data: &[u8]) {
    let mut packets = vec![data.to_vec()];

    while let Some(current_data) = packets.pop() {
        let mut offset = 0;
        while offset + HEADER_LEN as usize <= current_data.len() {
            let packet_len = u32::from_be_bytes(current_data[offset..offset + 4].try_into().unwrap()) as usize;
            let header_len = u16::from_be_bytes(current_data[offset + 4..offset + 6].try_into().unwrap()) as usize;
            let proto_ver = u16::from_be_bytes(current_data[offset + 6..offset + 8].try_into().unwrap());
            let op = u32::from_be_bytes(current_data[offset + 8..offset + 12].try_into().unwrap());

            if offset + packet_len > current_data.len() {
                break;
            }
            let body = &current_data[offset + header_len..offset + packet_len];

            match proto_ver {
                PROTO_VER_ZLIB => {
                    let mut decoder = ZlibDecoder::new(body);
                    let mut decompressed = Vec::new();
                    match decoder.read_to_end(&mut decompressed) {
                        Ok(_) => {
                            if ws_debug_enabled() {
                                eprintln!("[WebSocket][DEBUG] zlib decompressed len={} from len={}", decompressed.len(), body.len());
                            }
                            packets.push(decompressed);
                        }
                        Err(e) => {
                            eprintln!("[WebSocket][DEBUG] zlib decompress failed: {e}");
                        }
                    }
                }
                PROTO_VER_BROTLI => {
                    let mut decoder = Decompressor::new(body, 4096);
                    let mut decompressed = Vec::new();
                    match decoder.read_to_end(&mut decompressed) {
                        Ok(_) => {
                            if ws_debug_enabled() {
                                eprintln!("[WebSocket][DEBUG] brotli decompressed len={} from len={}", decompressed.len(), body.len());
                            }
                            packets.push(decompressed);
                        }
                        Err(e) => {
                            eprintln!("[WebSocket][DEBUG] brotli decompress failed: {e}");
                        }
                    }
                }
                _ => match op {
                    OP_HEARTBEAT_REPLY => {
                        if body.len() >= 4 {
                            let popularity = u32::from_be_bytes(body[0..4].try_into().unwrap());
                            println!("[WebSocket] 心跳回复 人气={}", popularity);
                        }
                    }
                    OP_AUTH_REPLY => {
                        match serde_json::from_slice::<serde_json::Value>(body) {
                            Ok(v) => {
                                eprintln!("[WebSocket] 鉴权回复: {}", v);
                                if v.get("code").and_then(|c| c.as_i64()).unwrap_or(0) != 0 {
                                    emit_status("disconnected", &format!("鉴权失败 code={}", v["code"])).await;
                                    break;
                                }
                            }
                            Err(_) => println!("[WebSocket] 鉴权成功"),
                        }
                    }
                    OP_MESSAGE => {
                        if let Ok(text) = std::str::from_utf8(body) {
                            handle_command_text(text).await;
                        }
                    }
                    _ => {}
                },
            }
            offset += packet_len;
        }
    }
}

async fn handle_command_text(text: &str) {
    if let Ok(val) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(cmd) = val.get("cmd").and_then(|c| c.as_str()) {
            let cmd = cmd.split(':').next().unwrap_or(cmd);
            // 公屏 DANMU_MSG，开放平台 OPEN_LIVE_* 事件
            match cmd {
                "DANMU_MSG" => {
                    // println!("[WebSocket] 弹幕原始信息 {}", serde_json::to_string_pretty(&val).unwrap_or_default());
                    if let Some(msg) = parse_danmu_msg(&val) {
                        println!("[WebSocket] 弹幕解析结果 {}", serde_json::to_string_pretty(&msg).unwrap_or_default());
                        if should_forward_danmu(&msg).await {
                            forward_to_sse(msg).await;
                        }
                    }
                }
                "OPEN_LIVE_DANMAKU" => {
                    if let Some(msg) = parse_open_live_danmaku(&val) {
                        forward_to_sse(msg).await;
                    }
                }
                "OPEN_LIVE_GIFT" => {
                    if let Some(msg) = parse_open_live_gift(&val) {
                        forward_to_sse(msg).await;
                    }
                }
                "OPEN_LIVE_SUPER_CHAT" => {
                    if let Some(msg) = parse_open_live_super_chat(&val) {
                        forward_to_sse(msg).await;
                    }
                }
                _ => {}
            }
        }
    } else {
        eprintln!("[WebSocket] JSON 解析失败: {text}");
    }
}

async fn forward_to_sse(val: serde_json::Value) {
    if let Some(state) = crate::get_sse_state().await {
        // 兜底补齐 hasOwnMedal / isModerator / guardLevel
        let mut val = val;
        if let Some(obj) = val.as_object_mut() {
            if obj.get("isModerator").and_then(|v| v.as_bool()).is_none() {
                obj.insert("isModerator".to_string(), serde_json::Value::Bool(false));
            }

            if obj.get("hasOwnMedal").and_then(|v| v.as_bool()).is_none() {
                obj.insert("hasOwnMedal".to_string(), serde_json::Value::Bool(false));
            }

            if obj.get("guardLevel").and_then(|v| v.as_i64()).is_none() {
                obj.insert("guardLevel".to_string(), serde_json::Value::Number(0.into()));
            }

            // 通过 media_ruid 与主播 uid 匹配判断“佩戴本房间粉丝牌”
            let media_ruid = obj.get("media_ruid").and_then(|v| {
                if let Some(u) = v.as_u64() {
                    Some(u.to_string())
                } else {
                    v.as_str().map(|s| s.to_string())
                }
            });

            if let Some(media_ruid) = media_ruid {
                let owner_uid = WS_STATE.read().await.room_owner_uid;
                if let Some(owner_uid) = owner_uid {
                    if media_ruid == owner_uid.to_string() {
                        obj.insert("hasOwnMedal".to_string(), serde_json::Value::Bool(true));
                    }
                }
            }
        }

        let val = crate::apply_style_to_sse_message(val).await;
        crate::sse_server::send_to_all_connections(&state, val).await;
    }
}

pub async fn current_room_key() -> Option<String> {
    WS_STATE.read().await.current_room_key.clone()
}

pub async fn current_room_key_type() -> Option<String> {
    WS_STATE
        .read()
        .await
        .current_room_key_type
        .map(|t| match t {
            RoomKeyType::RoomId => "RoomId".to_string(),
            RoomKeyType::AuthCode => "AuthCode".to_string(),
        })
}

async fn emit_status(status: &str, message: &str) {
    let guard = WS_STATE.read().await;
    if let Some(app_handle) = &guard.app_handle {
        let _ = app_handle.emit("websocket-status", serde_json::json!({
            "status": status,
            "message": message
        }));
    }
}

/* ---------------- 开放平台 REST ---------------- */

#[derive(Deserialize)]
struct StartGameResp {
    data: StartGameData,
}
#[derive(Deserialize)]
#[allow(dead_code)]
struct StartGameData {
    game_info: GameInfo,
    websocket_info: WebsocketInfo,
}
#[derive(Deserialize)]
#[allow(dead_code)]
struct GameInfo {
    game_id: String,
}
#[derive(Deserialize)]
struct WebsocketInfo {
    wss_link: Vec<String>,
    auth_body: String,
}

struct WsConnInfo {
    ws_url: String,
    auth_body: String,
}

async fn start_game_and_get_ws(
    auth_code: &str,
    open_live_app_id: Option<i64>,
    open_live_access_key_id: Option<String>,
    open_live_access_key_secret: Option<String>,
) -> Result<WsConnInfo, String> {
    // 依照 blivedm/clients/open_live.py _request_open_live
    let app_id: i64 = match open_live_app_id {
        Some(v) => v,
        std::option::Option::None => std::env::var("BILI_OPEN_LIVE_APP_ID")
            .map_err(|_| "缺少 BILI_OPEN_LIVE_APP_ID")?
            .parse()
            .map_err(|_| "BILI_OPEN_LIVE_APP_ID 不是数字")?,
    };
    let access_key_id = match open_live_access_key_id {
        Some(v) => v,
        std::option::Option::None => std::env::var("BILI_OPEN_LIVE_ACCESS_KEY_ID")
            .map_err(|_| "缺少 BILI_OPEN_LIVE_ACCESS_KEY_ID")?,
    };
    let access_key_secret = match open_live_access_key_secret {
        Some(v) => v,
        std::option::Option::None => std::env::var("BILI_OPEN_LIVE_ACCESS_KEY_SECRET")
            .map_err(|_| "缺少 BILI_OPEN_LIVE_ACCESS_KEY_SECRET")?,
    };

    #[derive(Serialize)]
    struct Body<'a> {
        code: &'a str,
        app_id: i64,
    }
    let body = Body { code: auth_code, app_id };
    let body_json = serde_json::to_string(&body).unwrap();
    let body_bytes = body_json.as_bytes();

    // 构造头部并签名（与 Python 版一致）
    let content_md5 = format!("{:x}", md5::compute(body_bytes));
    // python 版是 uuid.uuid4().hex（无短横线）
    let signature_nonce = uuid::Uuid::new_v4().simple().to_string();
    let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs().to_string();

    // 注意顺序固定
    let str_to_sign = [
        ("x-bili-accesskeyid", access_key_id.as_str()),
        ("x-bili-content-md5", content_md5.as_str()),
        ("x-bili-signature-method", "HMAC-SHA256"),
        ("x-bili-signature-nonce", signature_nonce.as_str()),
        ("x-bili-signature-version", "1.0"),
        ("x-bili-timestamp", timestamp.as_str()),
    ]
    .iter()
    .map(|(k, v)| format!("{k}:{v}"))
    .collect::<Vec<_>>()
    .join("\n");

    let signature = hmac_sha256::HMAC::mac(str_to_sign.as_bytes(), access_key_secret.as_bytes());
    let signature_hex = hex::encode(signature);

    let client = reqwest::Client::new();
    let res = client
        .post("https://live-open.biliapi.com/v2/app/start")
        .header("x-bili-accesskeyid", &access_key_id)
        .header("x-bili-content-md5", &content_md5)
        .header("x-bili-signature-method", "HMAC-SHA256")
        .header("x-bili-signature-nonce", &signature_nonce)
        .header("x-bili-signature-version", "1.0")
        .header("x-bili-timestamp", &timestamp)
        .header("Authorization", signature_hex)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json")
        .body(body_json)
        .send()
        .await
        .map_err(|e| format!("start 请求失败: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("start 返回状态码 {}", res.status()));
    }
    let parsed: StartGameResp = res.json().await.map_err(|e| format!("start 解析失败: {e}"))?;
    let ws_url = parsed
        .data
        .websocket_info
        .wss_link
        .get(0)
        .cloned()
        .ok_or_else(|| "start 返回的 wss_link 为空".to_string())?;
    Ok(WsConnInfo {
        ws_url,
        auth_body: parsed.data.websocket_info.auth_body,
    })
}

/// CLI/测试场景使用：不依赖 Tauri AppHandle，也不发送前端事件。
pub async fn connect_websocket_cli(
    room_key: String,
    room_key_type: RoomKeyType,
    reconnect_interval: u64,
    max_reconnect_attempts: u32,
    open_live_app_id: Option<i64>,
    open_live_access_key_id: Option<String>,
    open_live_access_key_secret: Option<String>,
) -> Result<(), String> {
    {
        let guard = WS_STATE.read().await;
        if guard.handle.is_some() {
            return Err("已有连接".to_string());
        }
    }

    {
        let mut guard = WS_STATE.write().await;
        guard.app_handle = None;
        guard.should_stop = false;
    }

    let handle = tokio::spawn(run_ws_loop(
        room_key,
        room_key_type,
        reconnect_interval,
        max_reconnect_attempts,
        open_live_app_id,
        open_live_access_key_id,
        open_live_access_key_secret,
    ));

    {
        let mut guard = WS_STATE.write().await;
        guard.handle = Some(handle);
    }

    Ok(())
}