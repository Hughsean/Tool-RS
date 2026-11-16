use std::io::IsTerminal;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Parser;
use tool_common::{read_input_bytes, write_output_bytes};
use tool_media::gif::{self, GifOptions};
use tool_media::webp;
use tool_media::{Animation, LoopCount};

/// 把动图 WebP 转成动图 GIF。
///
/// 省略文件参数就走标准输入/输出，方便接管道：
///   webp2gif < in.webp > out.gif
#[derive(Debug, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// 输入 WebP 文件；省略或 `-` 表示 stdin
    input: Option<String>,

    /// 输出 GIF 文件；省略或 `-` 表示 stdout
    output: Option<String>,

    /// 固定帧率，覆盖每帧原始时长
    #[arg(long, value_name = "FPS", conflicts_with = "delay")]
    fps: Option<f64>,

    /// 固定每帧时长（毫秒），覆盖每帧原始时长
    #[arg(long, value_name = "MS")]
    delay: Option<u64>,

    /// 整体缩放比例，如 0.5 表示缩小一半
    #[arg(long, value_name = "FACTOR", conflicts_with = "width")]
    scale: Option<f64>,

    /// 缩放到指定宽度，高度按原比例
    #[arg(long, value_name = "PX")]
    width: Option<u32>,

    /// 循环次数，0 表示无限循环；省略则沿用源文件的循环设置
    #[arg(long = "loop", value_name = "N")]
    loop_count: Option<u16>,

    /// 调色板量化速度 1..=30，越大越快、画质越差
    #[arg(long, value_name = "1-30", default_value_t = 10, value_parser = clap::value_parser!(u8).range(1..=30))]
    speed: u8,

    /// 不向 stderr 打印转换统计
    #[arg(long)]
    quiet: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input = read_input_bytes(args.input.as_deref())?;
    let mut animation = webp::decode(&input)?;
    let source = Stats::of(&animation);

    apply_timing(&mut animation, &args)?;
    apply_scaling(&mut animation, &args)?;
    apply_loop(&mut animation, &args);

    let options = GifOptions {
        speed: args.speed,
        ..GifOptions::default()
    };
    let bytes = gif::encode(&animation, &options)?;

    if args.output.is_none() && std::io::stdout().is_terminal() && !args.quiet {
        eprintln!("warning: GIF 正在写入终端，建议用输出文件参数或重定向到文件");
    }
    write_output_bytes(args.output.as_deref(), &bytes)?;

    if !args.quiet {
        eprintln!("输入 {}", source.describe());
        eprintln!("输出 {}", Stats::of(&animation).describe());
        eprintln!("GIF 大小 {}", human_size(bytes.len()));
    }
    Ok(())
}

/// 用 `--fps` / `--delay` 覆盖每帧时长。
fn apply_timing(animation: &mut Animation, args: &Args) -> Result<()> {
    if let Some(fps) = args.fps {
        if !(fps.is_finite() && fps > 0.0) {
            bail!("--fps 必须是正数，当前 {fps}");
        }
        animation.set_delay(Duration::from_secs_f64(1.0 / fps));
    }
    if let Some(millis) = args.delay {
        if millis == 0 {
            bail!("--delay 必须大于 0");
        }
        animation.set_delay(Duration::from_millis(millis));
    }
    Ok(())
}

/// 用 `--scale` / `--width` 缩放所有帧。
fn apply_scaling(animation: &mut Animation, args: &Args) -> Result<()> {
    if let Some(factor) = args.scale {
        animation.scale(factor)?;
    }
    if let Some(width) = args.width {
        animation.resize_to_width(width)?;
    }
    Ok(())
}

/// 用 `--loop` 覆盖循环设置。
fn apply_loop(animation: &mut Animation, args: &Args) {
    if let Some(times) = args.loop_count {
        animation.loop_count = match times {
            0 => LoopCount::Infinite,
            times => LoopCount::Times(times),
        };
    }
}

/// 用于打印一行统计的快照。
struct Stats {
    width: u32,
    height: u32,
    frames: usize,
    duration: Duration,
    loop_count: LoopCount,
}

impl Stats {
    fn of(animation: &Animation) -> Self {
        Self {
            width: animation.width,
            height: animation.height,
            frames: animation.frame_count(),
            duration: animation.duration(),
            loop_count: animation.loop_count,
        }
    }

    fn describe(&self) -> String {
        let loop_text = match self.loop_count {
            LoopCount::Infinite => "无限循环".to_owned(),
            LoopCount::Times(times) => format!("循环 {times} 次"),
        };
        format!(
            "{}x{} {} 帧 {:.2}s {loop_text}",
            self.width,
            self.height,
            self.frames,
            self.duration.as_secs_f64()
        )
    }
}

fn human_size(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "KiB", "MiB", "GiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn clap_definition_is_valid() {
        Args::command().debug_assert();
    }

    #[test]
    fn human_size_uses_binary_units() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 KiB");
    }
}
