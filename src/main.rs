mod websocket;
mod watcher;

use crate::websocket::{start_ws_server, start_ws_client, ServerConfig};
use crate::watcher::start_file_watcher;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        println!("用法:");
        println!("  服务端: {} server [目标目录]", args[0]);
        println!("  客户端: {} client <监控路径>", args[0]);
        return Ok(());
    }

    match args[1].as_str() {
        "server" => {
            let config = ServerConfig {
                addr: "127.0.0.1:8080".to_string(),
                target_dir: args.get(2)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| PathBuf::from(".")),
            };
            
            println!("启动服务器模式");
            println!("目标目录: {}", config.target_dir.display());
            start_ws_server(config).await?;
        }
        "client" => {
            if args.len() < 3 {
                println!("错误: 客户端模式需要指定监控路径");
                return Ok(());
            }
            let path = &args[2];
            println!("启动客户端模式，监控路径: {}", path);
            let tx = start_ws_client("ws://127.0.0.1:8080").await?;
            start_file_watcher(path, tx).await?;
        }
        _ => {
            println!("无效的模式，请使用 'server' 或 'client'");
        }
    }

    Ok(())
}
