use notify::{recommended_watcher, RecursiveMode, Watcher, Event};
use std::path::Path;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, WebSocketStream};
use futures::prelude::*;
use serde::{Serialize, Deserialize};

// 定义要传输的消息结构
#[derive(Serialize, Deserialize)]
struct FileChange {
    path: String,
    kind: String,
    timestamp: u64,
}

// 修改 watch 函数以支持事件聚合
async fn watch<P: AsRef<Path>>(path: P, ws_url: &str) -> notify::Result<()> {
    let (tx, rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(tx)?;
    
    // 事件聚合缓存
    let mut event_cache: HashMap<String, (Event, Instant)> = HashMap::new();
    const AGGREGATION_DELAY: Duration = Duration::from_secs(2);

    // 连接 WebSocket
    let (ws_stream, _) = connect_async(ws_url).await.expect("Failed to connect");
    let (mut write, _read) = ws_stream.split();

    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    loop {
        if let Ok(event) = rx.try_recv() {
            match event {
                Ok(event) => {
                    // 将事件添加到缓存
                    for path in event.paths.iter() {
                        let path_str = path.to_string_lossy().to_string();
                        event_cache.insert(path_str, (event.clone(), Instant::now()));
                    }
                }
                Err(e) => println!("watch error: {:?}", e),
            }
        }

        // 检查并发送聚合后的事件
        let now = Instant::now();
        let mature_events: Vec<FileChange> = event_cache
            .iter()
            .filter(|(_, (_, timestamp))| now.duration_since(*timestamp) >= AGGREGATION_DELAY)
            .map(|(path, (event, _))| FileChange {
                path: path.clone(),
                kind: format!("{:?}", event.kind),
                timestamp: now.elapsed().as_secs(),
            })
            .collect();

        // 发送成熟的事件并从缓存中移除
        for change in mature_events {
            let msg = serde_json::to_string(&change).unwrap();
            write.send(msg.into()).await.expect("Failed to send message");
            event_cache.remove(&change.path);
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

// 修改 main 函数以支持异步
#[tokio::main]
async fn main() {
    let role = std::env::args()
        .nth(1)
        .expect("Argument 1 needs to be a server/clent");
    let path = std::env::args()
        .nth(2)
        .expect("Argument 2 needs to be a path");
    if role == "server" {
        if !std::path::Path::new(&path).exists() {
            println!("路径 '{}' 不存在", path);
            return;
        }
        println!("watching {}", path);
        let ws_url = "ws://localhost:8080";
        if let Err(e) = watch(path, ws_url).await {
            println!("error: {:?}", e)
        }
    } else {
        println!("client {}", path);
    }
}
