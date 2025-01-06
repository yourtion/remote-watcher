mod websocket;

use crate::websocket::{FileChange, start_ws_server, start_ws_client};
use notify::{recommended_watcher, RecursiveMode, Watcher, Event};
use std::path::Path;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;

async fn watch<P: AsRef<Path>>(
    path: P,
    tx: broadcast::Sender<FileChange>
) -> notify::Result<()> {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(notify_tx)?;
    
    let mut event_cache: HashMap<String, (Event, Instant)> = HashMap::new();
    const AGGREGATION_DELAY: Duration = Duration::from_secs(2);
    const CHECK_INTERVAL: Duration = Duration::from_millis(500);

    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    let mut last_check = Instant::now();

    loop {
        match notify_rx.recv_timeout(CHECK_INTERVAL) {
            Ok(Ok(event)) => {
                for path in event.paths.iter() {
                    let path_str = path.to_string_lossy().to_string();
                    println!("检测到文件变更: {}", path_str);
                    event_cache.insert(path_str, (event.clone(), Instant::now()));
                }
            }
            Ok(Err(e)) => println!("监控错误: {:?}", e),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            }
            Err(e) => {
                println!("接收错误: {:?}", e);
                break;
            }
        }

        let now = Instant::now();
        if now.duration_since(last_check) >= CHECK_INTERVAL {
            last_check = now;

            let mature_events: Vec<FileChange> = event_cache
                .iter()
                .filter(|(_, (_, timestamp))| now.duration_since(*timestamp) >= AGGREGATION_DELAY)
                .map(|(path, (event, _))| FileChange {
                    path: path.clone(),
                    kind: format!("{:?}", event.kind),
                    timestamp: now.elapsed().as_secs(),
                    from_server: false,
                })
                .collect();

            for change in mature_events {
                println!("发送文件变更通知: {:?}", change);
                if tx.send(change.clone()).is_err() {
                    println!("无法发送变更通知");
                }
                event_cache.remove(&change.path);
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 这里假设通过命令行参数获取角色和路径
    let role = std::env::args().nth(1).unwrap_or_else(|| "client".to_string());
    let path = std::env::args().nth(2).unwrap_or_else(|| ".".to_string());

    match role.as_str() {
        "server" => {
            println!("启动服务器模式");
            start_ws_server("127.0.0.1:8080").await?;
        }
        "client" => {
            println!("启动客户端模式");
            let tx = start_ws_client("ws://127.0.0.1:8080").await?;
            watch(path, tx).await?;
        }
        _ => {
            println!("无效的角色，请使用 'server' 或 'client'");
        }
    }

    Ok(())
}
