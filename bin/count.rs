use anyhow::Result;
use clap::Parser;
use tool_common::read_input;

/// 统计行数、单词数、字符数、字节数。
#[derive(Debug, Parser)]
#[command(version, about)]
struct Args {
    /// 输入文件；省略或 `-` 表示 stdin
    file: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let input = read_input(args.file.as_deref())?;

    println!("lines\twords\tchars\tbytes");
    println!(
        "{}\t{}\t{}\t{}",
        input.lines().count(),
        input.split_whitespace().count(),
        input.chars().count(),
        input.len()
    );
    Ok(())
}
