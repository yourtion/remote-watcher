use std::fs;
use std::path::Path;
use similar::{ChangeTag, TextDiff};

pub fn calculate_diff(path: &Path, old_content: Option<&str>) -> std::io::Result<String> {
    // 读取当前文件内容
    let new_content = fs::read_to_string(path)?;
    
    // 如果没有旧内容，返回完整的新内容
    let old_content = old_content.unwrap_or("");
    
    // 计算差异
    let diff = TextDiff::from_lines(old_content, &new_content);
    
    // 构建差异字符串
    let mut diff_text = String::new();
    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        diff_text.push_str(&format!("{}{}", sign, change));
    }
    
    Ok(diff_text)
} 