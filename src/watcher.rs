use notify::{recommended_watcher, RecursiveMode, Watcher, Event};
use std::path::Path;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use std::fs;
use crate::websocket::FileChange;
use crate::diff::calculate_diff;

pub async fn start_file_watcher<P: AsRef<Path>>(
    path: P,
    tx: broadcast::Sender<FileChange>
) -> notify::Result<()> {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut watcher = recommended_watcher(notify_tx)?;
    
    let mut event_cache: HashMap<String, (Event, Instant)> = HashMap::new();
    let mut content_cache: HashMap<String, String> = HashMap::new();
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
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
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
                .filter_map(|(path, (event, _))| {
                    let path = Path::new(path);
                    
                    // 计算文件差异
                    let content_diff = match calculate_diff(
                        path,
                        content_cache.get(path.to_string_lossy().as_ref())
                            .map(String::as_str)
                    ) {
                        Ok(diff) => {
                            // 更新缓存
                            if let Ok(new_content) = fs::read_to_string(path) {
                                content_cache.insert(path.to_string_lossy().to_string(), new_content);
                            }
                            Some(diff)
                        }
                        Err(e) => {
                            println!("计算差异失败: {}", e);
                            None
                        }
                    };

                    Some(FileChange {
                        path: path.to_string_lossy().to_string(),
                        kind: format!("{:?}", event.kind),
                        timestamp: now.elapsed().as_secs(),
                        content_diff,
                        from_server: false,
                    })
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