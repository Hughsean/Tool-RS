use anyhow::Result;
use clap::{Parser, ValueEnum};
use tool_common::read_input;

/// 在 camelCase / snake_case / kebab-case / SCREAMING-CASE 之间转换。
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// 目标命名风格
    #[arg(value_enum)]
    style: Style,

    /// 输入文件；省略或 `-` 表示 stdin
    file: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Style {
    Camel,
    Snake,
    Kebab,
    Screaming,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let input = read_input(args.file.as_deref())?;
    // 只转换第一行非空内容，避免把整段文本当成标识符。
    let source = input.lines().next().unwrap_or_default().trim();
    println!("{}", convert(source, args.style));
    Ok(())
}

fn convert(input: &str, style: Style) -> String {
    let words = split_words(input);
    match style {
        Style::Camel => {
            let mut out = String::new();
            for (i, word) in words.iter().enumerate() {
                if i == 0 {
                    out.push_str(word);
                } else {
                    push_capitalized(&mut out, word);
                }
            }
            out
        }
        Style::Snake => words.join("_"),
        Style::Kebab => words.join("-"),
        Style::Screaming => words.join("_").to_uppercase(),
    }
}

/// 按非字母数字字符和大小写边界切词，并统一转小写。
fn split_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut prev_lower = false;

    for ch in input.chars() {
        if !ch.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            prev_lower = false;
            continue;
        }
        if ch.is_uppercase() && prev_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        prev_lower = ch.is_lowercase() || ch.is_numeric();
        current.extend(ch.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn push_capitalized(out: &mut String, word: &str) {
    let mut chars = word.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
}
