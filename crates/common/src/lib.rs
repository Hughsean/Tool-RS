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
