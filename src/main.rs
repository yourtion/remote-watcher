mod websocket;
mod watcher;

use crate::websocket::{start_ws_server, start_ws_client, ServerConfig};
use crate::watcher::start_file_watcher;
use std::path::{Path, PathBuf};

// 验证路径是否为有效的相对路径且在当前目录下
fn validate_watch_path(path: &str) -> Result<PathBuf, &'static str> {
    let path = Path::new(path);
    
    // 检查是否为绝对路径
    if path.is_absolute() {
        return Err("监控路径必须是相对路径");
    }
    
    // 检查路径组件，确保没有 '..' 
    if path.components().any(|c| c == std::path::Component::ParentDir) {
        return Err("不允许访问上级目录");
    }
    
    // 检查路径是否存在且是目录
    let canonical_path = path.canonicalize()
        .map_err(|_| "无法解析路径")?;
    
    if !canonical_path.is_dir() {
        return Err("指定的路径不是目录");
    }
    
    // 确保路径在当前目录下
    let current_dir = std::env::current_dir()
        .map_err(|_| "无法获取当前目录")?
        .canonicalize()
        .map_err(|_| "无法解析当前目录")?;
        
    if !canonical_path.starts_with(current_dir) {
        return Err("监控路径必须在当前目录下");
    }
    
    Ok(PathBuf::from(path))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        println!("用法:");
        println!("  服务端: {} server [目标目录]", args[0]);
        println!("  客户端: {} client <监控路径>", args[0]);
        println!("\n监控路径必须是当前目录下的相对路径，例如:");
        println!("  {} client ./logs", args[0]);
        println!("  {} client src/test", args[0]);
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
            
            // 验证监控路径
            let path = match validate_watch_path(&args[2]) {
                Ok(path) => path,
                Err(e) => {
                    println!("错误: {}", e);
                    println!("监控路径必须是当前目录下的相对路径，例如:");
                    println!("  {} client ./logs", args[0]);
                    println!("  {} client src/test", args[0]);
                    return Ok(());
                }
            };
            
            println!("启动客户端模式，监控路径: {}", path.display());
            let tx = start_ws_client("ws://127.0.0.1:8080").await?;
            start_file_watcher(path, tx).await?;
        }
        _ => {
            println!("无效的模式，请使用 'server' 或 'client'");
        }
    }

    Ok(())
}
