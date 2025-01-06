use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    accept_async, connect_async,
    tungstenite::Message,
    WebSocketStream, MaybeTlsStream,
};
use std::net::SocketAddr;
use serde::{Serialize, Deserialize};
use tokio::sync::broadcast;

// 定义消息类型
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub kind: String,
    pub timestamp: u64,
    #[serde(default)]  // 添加来源标记
    pub from_server: bool,
}

// WebSocket 客户端
pub async fn start_ws_client(url: &str) -> Result<broadcast::Sender<FileChange>, Box<dyn std::error::Error>> {
    let (ws_stream, _) = connect_async(url).await?;
    let (tx, _) = broadcast::channel::<FileChange>(100);
    let tx_clone = tx.clone();
    
    // 分离读写流
    let (mut write, read) = ws_stream.split();
    
    // 创建一个新的接收器来监听文件变更
    let mut rx = tx.subscribe();
    
    // 处理发送文件变更到 WebSocket 服务器
    tokio::spawn(async move {
        while let Ok(change) = rx.recv().await {
            // 只转发本地产生的变更
            if !change.from_server {
                let message = serde_json::to_string(&change).unwrap();
                if let Err(e) = write.send(Message::Text(message)).await {
                    println!("发送到 WebSocket 失败: {}", e);
                    break;
                }
            }
        }
    });

    // 处理从 WebSocket 服务器接收的消息
    tokio::spawn(async move {
        handle_client_connection(read, tx_clone).await;
    });

    Ok(tx)
}

async fn handle_client_connection(
    mut read: futures::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    tx: broadcast::Sender<FileChange>
) {
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(mut change) = serde_json::from_str::<FileChange>(&text) {
                    // 标记这是从服务器收到的消息
                    change.from_server = true;
                    println!("收到服务器消息: {:?}", change);
                    if tx.send(change).is_err() {
                        println!("无法发送消息到通道");
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                println!("WebSocket 错误: {}", e);
                break;
            }
            _ => {}
        }
    }
}

// WebSocket 服务器
pub async fn start_ws_server(addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let socket = TcpListener::bind(addr).await?;
    println!("WebSocket 服务器运行在: {}", addr);
    
    let (broadcast_tx, _) = broadcast::channel::<FileChange>(100);
    let broadcast_tx = std::sync::Arc::new(broadcast_tx);

    while let Ok((stream, peer)) = socket.accept().await {
        println!("接受新的连接: {}", peer);
        let tx = broadcast_tx.clone();
        tokio::spawn(async move {
            handle_server_connection(stream, peer, tx).await;
        });
    }

    Ok(())
}

async fn handle_server_connection(
    stream: TcpStream,
    peer: SocketAddr,
    broadcast_tx: std::sync::Arc<broadcast::Sender<FileChange>>
) {
    let ws_stream = match accept_async(stream).await {
        Ok(ws_stream) => ws_stream,
        Err(e) => {
            println!("处理连接错误 {}: {}", peer, e);
            return;
        }
    };

    let (mut write, mut read) = ws_stream.split();
    let mut rx = broadcast_tx.subscribe();

    // 转发消息到当前客户端
    let forward_task = tokio::spawn(async move {
        while let Ok(change) = rx.recv().await {
            let message = serde_json::to_string(&change).unwrap();
            if let Err(e) = write.send(Message::Text(message)).await {
                println!("发送消息失败: {}", e);
                break;
            }
        }
    });

    // 处理来自客户端的消息
    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(change) = serde_json::from_str::<FileChange>(&text) {
                    println!("服务器收到文件变更: {:?}", change);
                    // 广播给其他客户端
                    if broadcast_tx.send(change).is_err() {
                        println!("无法广播消息");
                    }
                }
            }
            Ok(Message::Close(_)) => break,
            Err(e) => {
                println!("错误: {}", e);
                break;
            }
            _ => {}
        }
    }

    forward_task.abort();
    println!("连接关闭: {}", peer);
}


