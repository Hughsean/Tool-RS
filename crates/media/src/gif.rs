use std::time::Duration;

use anyhow::{Context, Result, bail};

use crate::animation::{Animation, LoopCount};

/// GIF 编码参数。
#[derive(Clone, Copy, Debug)]
pub struct GifOptions {
    /// 调色板量化速度 1..=30：越大越快、画质越差。
    pub speed: u8,
    /// 循环设置；`None` 表示沿用动画自身的设置。
    pub loop_count: Option<LoopCount>,
}

impl Default for GifOptions {
    fn default() -> Self {
        Self {
            speed: 10,
            loop_count: None,
        }
    }
}

/// 把动画模型编码成 GIF 字节流。
///
/// 颜色量化交给 `gif` crate 的 NeuQuant（每帧独立 256 色调色板）；
/// 透明度只能压成 1-bit，alpha 为 0 的像素视为透明。
pub fn encode(animation: &Animation, options: &GifOptions) -> Result<Vec<u8>> {
    animation.validate()?;
    if !(1..=30).contains(&options.speed) {
        bail!(
            "quantizer speed must be within 1..=30, got {}",
            options.speed
        );
    }
    let width = u16::try_from(animation.width).context("GIF width limit is 65535")?;
    let height = u16::try_from(animation.height).context("GIF height limit is 65535")?;

    let repeat = match options.loop_count.unwrap_or(animation.loop_count) {
        LoopCount::Infinite => ::gif::Repeat::Infinite,
        // NETSCAPE 扩展记录的是"额外重复次数"，所以总播放次数要减一；
        // `Finite(0)` 会被 `gif` 直接跳过不写，正好等于只播放一遍。
        LoopCount::Times(times) => ::gif::Repeat::Finite(times.saturating_sub(1)),
    };

    let mut output = Vec::new();
    {
        let mut encoder = ::gif::Encoder::new(&mut output, width, height, &[])
            .context("failed to create GIF encoder")?;
        encoder
            .set_repeat(repeat)
            .context("failed to write GIF loop info")?;

        let single_frame = animation.frame_count() == 1;
        for frame in &animation.frames {
            // NeuQuant 会就地改写像素，所以每帧都需要一份独立拷贝。
            let mut rgba = frame.rgba.clone();
            let mut gif_frame =
                ::gif::Frame::from_rgba_speed(width, height, &mut rgba, i32::from(options.speed));
            gif_frame.delay = if single_frame {
                0
            } else {
                centiseconds(frame.delay)
            };
            encoder
                .write_frame(&gif_frame)
                .context("failed to write GIF frame")?;
        }
    }
    Ok(output)
}

/// GIF 以百分之一秒记录帧延迟。多数播放器会把小于 2cs 的值兜底成 10cs，
/// 因此这里四舍五入后夹到 `[2, 65535]`，避免动画因为量化后变成 0 而变慢。
fn centiseconds(delay: Duration) -> u16 {
    let value = (delay.as_millis() + 5) / 10;
    value.clamp(2, u128::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use super::*;
    use crate::animation::Frame;

    fn animation(frame_count: u32, delay: Duration, loop_count: LoopCount) -> Animation {
        let mut animation = Animation::new(4, 2, loop_count);
        for index in 0..frame_count {
            let tone = (index % 2) as u8 * 200;
            let rgba = vec![tone, 255 - tone, 128, 255].repeat(4 * 2);
            animation.frames.push(Frame::new(rgba, delay));
        }
        animation
    }

    fn decode_frame_meta(bytes: &[u8]) -> (Vec<u16>, ::gif::Repeat) {
        let mut options = ::gif::DecodeOptions::new();
        options.set_color_output(::gif::ColorOutput::RGBA);
        let mut decoder = options.read_info(Cursor::new(bytes.to_vec())).unwrap();
        let repeat = decoder.repeat();
        let mut delays = Vec::new();
        while let Some(frame) = decoder.read_next_frame().unwrap() {
            delays.push(frame.delay);
        }
        (delays, repeat)
    }

    #[test]
    fn round_trip_keeps_frame_count_and_delays() {
        let source = animation(3, Duration::from_millis(120), LoopCount::Infinite);
        let bytes = encode(&source, &GifOptions::default()).unwrap();
        let (delays, repeat) = decode_frame_meta(&bytes);
        assert_eq!(delays, vec![12, 12, 12]);
        assert_eq!(repeat, ::gif::Repeat::Infinite);
    }

    #[test]
    fn loop_override_and_finite_playback() {
        let source = animation(2, Duration::from_millis(50), LoopCount::Infinite);
        let options = GifOptions {
            loop_count: Some(LoopCount::Times(3)),
            ..GifOptions::default()
        };
        let (_, repeat) = decode_frame_meta(&encode(&source, &options).unwrap());
        assert_eq!(repeat, ::gif::Repeat::Finite(2));

        let options = GifOptions {
            loop_count: Some(LoopCount::Times(1)),
            ..GifOptions::default()
        };
        let (_, repeat) = decode_frame_meta(&encode(&source, &options).unwrap());
        assert_eq!(repeat, ::gif::Repeat::Finite(0));
    }

    #[test]
    fn short_delays_are_clamped_to_two_centiseconds() {
        let source = animation(2, Duration::from_millis(3), LoopCount::Infinite);
        let (delays, _) = decode_frame_meta(&encode(&source, &GifOptions::default()).unwrap());
        assert_eq!(delays, vec![2, 2]);
    }

    #[test]
    fn rejects_out_of_range_speed() {
        let source = animation(1, Duration::from_millis(100), LoopCount::Infinite);
        let options = GifOptions {
            speed: 31,
            ..GifOptions::default()
        };
        assert!(encode(&source, &options).is_err());
    }
}
