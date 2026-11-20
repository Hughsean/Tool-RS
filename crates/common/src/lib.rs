//! 多个工具共用的垫子代码放这里：标准输入输出约定、格式化输出之类。
//!
//! 只有被两个以上工具用到的逻辑才升到这个 crate，
//! 单个工具自己用的逻辑留在自己的 `main.rs` / 模块里。

use std::io::{Read, Write};

use anyhow::{Context, Result};

/// 统一读入文本：`None` 或 `-` 表示 stdin，否则当作文件路径。
pub fn read_input(path: Option<&str>) -> Result<String> {
    let bytes = read_input_bytes(path)?;
    String::from_utf8(bytes).context("input is not valid UTF-8")
}

/// 统一读入字节：`None` 或 `-` 表示 stdin，否则当作文件路径。
///
/// 处理图片、音视频这类二进制输入的工具应该用它，而不是上面的文本版本。
pub fn read_input_bytes(path: Option<&str>) -> Result<Vec<u8>> {
    match path {
        None | Some("-") => {
            let mut buf = Vec::new();
            std::io::stdin()
                .read_to_end(&mut buf)
                .context("failed to read stdin")?;
            Ok(buf)
        }
        Some(path) => std::fs::read(path).with_context(|| format!("failed to read file `{path}`")),
    }
}

/// 统一写出字节：`None` 或 `-` 表示 stdout，否则写文件。
pub fn write_output_bytes(path: Option<&str>, bytes: &[u8]) -> Result<()> {
    match path {
        None | Some("-") => {
            let mut stdout = std::io::stdout().lock();
            stdout
                .write_all(bytes)
                .context("failed to write to stdout")?;
            stdout.flush().context("failed to flush stdout")?;
            Ok(())
        }
        Some(path) => {
            std::fs::write(path, bytes).with_context(|| format!("failed to write file `{path}`"))
        }
    }
}

/// 没给输入文件（或显式给了 `-`）时，数据只能来自 stdin。
pub fn reads_stdin(input: Option<&str>) -> bool {
    matches!(input, None | Some("-"))
}

/// 需要调用方改成"打印用法"的情形：没有文件输入，而 stdin 是交互终端。
///
/// 这时 `read_to_end` 会一直等人输入（表现为"卡住"），所以工具应该
/// 提前打印用法并以用法错误的退出码退出。
pub fn needs_explicit_input(input: Option<&str>, stdin_is_terminal: bool) -> bool {
    reads_stdin(input) && stdin_is_terminal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdin_is_used_only_when_input_is_missing_or_dash() {
        assert!(reads_stdin(None));
        assert!(reads_stdin(Some("-")));
        assert!(!reads_stdin(Some("a.jsonc")));
    }

    #[test]
    fn needs_explicit_input_only_for_terminal_without_file() {
        // 没有输入文件又处在终端：必须拦截，否则会卡在 stdin 上。
        assert!(needs_explicit_input(None, true));
        assert!(needs_explicit_input(Some("-"), true));
        // 有文件参数：正常读取文件。
        assert!(!needs_explicit_input(Some("a.jsonc"), true));
        // stdin 是管道/重定向：照旧从 stdin 读。
        assert!(!needs_explicit_input(None, false));
        assert!(!needs_explicit_input(Some("-"), false));
    }
}
