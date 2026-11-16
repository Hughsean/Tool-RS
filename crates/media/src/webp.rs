use std::io::Cursor;
use std::time::Duration;

use anyhow::{Context, Result};
use image_webp::{DecodingError, LoopCount as WebpLoopCount, WebPDecoder};

use crate::animation::{Animation, Frame, LoopCount};

/// 把 WebP 字节流解码成动画模型。
///
/// 动图的每一帧都由解码器合成成完整画布（含 disposal / blending 处理），
/// 所以上层不用关心子矩形和帧间叠加；静态 WebP 退化成只有一帧的动画。
pub fn decode(bytes: &[u8]) -> Result<Animation> {
    let mut decoder = WebPDecoder::new(Cursor::new(bytes)).context("failed to parse WebP data")?;
    let (width, height) = decoder.dimensions();
    let has_alpha = decoder.has_alpha();
    let loop_count = match decoder.loop_count() {
        WebpLoopCount::Forever => LoopCount::Infinite,
        WebpLoopCount::Times(times) => LoopCount::Times(times.get()),
    };
    let buffer_size = decoder
        .output_buffer_size()
        .context("WebP image dimensions are too large")?;

    let mut animation = Animation::new(width, height, loop_count);
    let mut buffer = vec![0u8; buffer_size];

    if decoder.is_animated() {
        loop {
            match decoder.read_frame(&mut buffer) {
                Ok(delay_ms) => animation.frames.push(Frame::new(
                    to_rgba(&buffer, has_alpha),
                    Duration::from_millis(u64::from(delay_ms)),
                )),
                Err(DecodingError::NoMoreFrames) => break,
                Err(err) => {
                    let index = animation.frames.len() + 1;
                    return Err(err).with_context(|| format!("failed to decode frame {index}"));
                }
            }
        }
    } else {
        decoder
            .read_image(&mut buffer)
            .context("failed to decode static WebP image")?;
        animation
            .frames
            .push(Frame::new(to_rgba(&buffer, has_alpha), Duration::ZERO));
    }

    animation.validate()?;
    Ok(animation)
}

/// 没有 alpha 通道时解码器输出 RGB8，这里统一补成 RGBA8，让模型只有一种像素布局。
fn to_rgba(buffer: &[u8], has_alpha: bool) -> Vec<u8> {
    if has_alpha {
        return buffer.to_vec();
    }
    let mut rgba = Vec::with_capacity(buffer.len() / 3 * 4);
    for pixel in buffer.chunks_exact(3) {
        rgba.extend_from_slice(pixel);
        rgba.push(u8::MAX);
    }
    rgba
}
