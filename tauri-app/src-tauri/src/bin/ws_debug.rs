use std::time::Duration;

use app_lib::bili_websocket_client::{self, RoomKeyType};

fn usage() {
    eprintln!(
        "Usage:\n  ws_debug roomid <ROOM_ID>\n  ws_debug authcode <AUTH_CODE>\n\nEnv:\n  BILI_WS_DEBUG=1   enable verbose ws logs\n  BILI_OPEN_LIVE_APP_ID / BILI_OPEN_LIVE_ACCESS_KEY_ID / BILI_OPEN_LIVE_ACCESS_KEY_SECRET (for authcode)\n"
    );
}

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() < 2 {
        usage();
        std::process::exit(2);
    }

    let mode = args.remove(0);
    let key = args.remove(0);

    let room_key_type = match mode.as_str() {
        "roomid" | "room_id" => RoomKeyType::ROOM_ID,
        "authcode" | "auth_code" => RoomKeyType::AUTH_CODE,
        _ => {
            usage();
            std::process::exit(2);
        }
    };

    // 只跑一段时间用于抓日志
    let reconnect_interval = 3000;
    let max_reconnect_attempts = 2;

    if let Err(e) = bili_websocket_client::connect_websocket_cli(
        key,
        room_key_type,
        reconnect_interval,
        max_reconnect_attempts,
        None,
        None,
        None,
    )
    .await
    {
        eprintln!("connect_websocket_cli failed: {e}");
        std::process::exit(1);
    }

    tokio::time::sleep(Duration::from_secs(30)).await;
    let _ = bili_websocket_client::disconnect_websocket().await;
}
