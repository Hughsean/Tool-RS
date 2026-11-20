use std::io::IsTerminal;

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser};
use tool_common::{needs_explicit_input, read_input, write_output_bytes};
use tool_jsonc::{FormatOptions, format};

/// 格式化 JSONC（带注释的 JSON）：保留注释，对象字段按字母序排序。
///
/// 省略文件参数就走标准输入/输出，方便接管道：
///   jsonfmt < in.jsonc > out.json
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// 输入 JSONC 文件；省略或 `-` 表示 stdin
    input: Option<String>,

    /// 输出文件；省略或 `-` 表示 stdout
    output: Option<String>,

    /// 缩进空格数
    #[arg(long, value_name = "N", default_value_t = 2, value_parser = clap::value_parser!(u8).range(0..=16))]
    indent: u8,

    /// 保持字段原有顺序，不排序
    #[arg(long)]
    keep_order: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    if needs_explicit_input(args.input.as_deref(), std::io::stdin().is_terminal()) {
        show_usage();
    }

    let source = read_input(args.input.as_deref())?;
    if source.trim().is_empty() {
        bail!("输入为空：确认管道真的传了数据，或显式指定 JSONC 文件");
    }

    let options = FormatOptions {
        indent_width: usize::from(args.indent),
        sort_keys: !args.keep_order,
    };
    let formatted = format(&source, &options)?;
    write_output_bytes(args.output.as_deref(), formatted.as_bytes())?;
    Ok(())
}

/// 没有输入文件又处在终端时打印用法并退出，避免卡在 stdin 上。
fn show_usage() -> ! {
    let mut command = Args::command();
    eprint!("{}", command.render_help());
    eprintln!();
    eprintln!("未指定输入文件，且标准输入是终端。请显式传文件，或用管道喂数据：");
    eprintln!("  jsonfmt in.jsonc out.json");
    eprintln!("  jsonfmt < in.jsonc > out.json");
    // 与 clap 的用法错误保持一致的退出码。
    std::process::exit(2);
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn clap_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn rendered_help_mentions_usage() {
        let help = Args::command().render_help().to_string();
        assert!(help.contains("Usage:"));
        assert!(help.contains("--indent"));
    }
}
