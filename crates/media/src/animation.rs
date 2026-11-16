use std::time::Duration;

use anyhow::{Context, Result, bail};

/// 动画中的一帧：整幅画布的 RGBA8 像素 + 该帧的显示时长。
///
/// "整幅画布"是刻意的约定：所有解码器都必须完成帧间合成（disposal / blending）
/// 再交给上层，这样编码器和变换代码都不用关心子矩形和差分帧。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// 整幅画布的 RGBA8 像素，长度必须是 `width * height * 4`。
    pub rgba: Vec<u8>,
    /// 该帧显示时长。
    pub delay: Duration,
}

impl Frame {
    pub fn new(rgba: Vec<u8>, delay: Duration) -> Self {
        Self { rgba, delay }
    }
}

/// 动画的循环设置。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopCount {
    /// 无限循环。
    Infinite,
    /// 总共播放的次数（含第一遍）。
    Times(u16),
}

/// 与容器格式无关的动图模型：解码器的输出和编码器的输入都是它。
#[derive(Clone, Debug)]
pub struct Animation {
    pub width: u32,
    pub height: u32,
    pub loop_count: LoopCount,
    pub frames: Vec<Frame>,
}

impl Animation {
    pub fn new(width: u32, height: u32, loop_count: LoopCount) -> Self {
        Self {
            width,
            height,
            loop_count,
            frames: Vec::new(),
        }
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// 播放一遍的总时长。
    pub fn duration(&self) -> Duration {
        self.frames.iter().map(|frame| frame.delay).sum()
    }

    /// 把所有帧的显示时长统一改成 `delay`（例如实现 `--fps` / `--delay` 覆盖）。
    pub fn set_delay(&mut self, delay: Duration) {
        for frame in &mut self.frames {
            frame.delay = delay;
        }
    }

    /// 按比例缩放所有帧，宽高各自四舍五入且至少 1 像素。
    pub fn scale(&mut self, factor: f64) -> Result<()> {
        if !(factor.is_finite() && factor > 0.0) {
            bail!("scale factor must be a positive number, got {factor}");
        }
        let width = ((self.width as f64 * factor).round() as u32).max(1);
        let height = ((self.height as f64 * factor).round() as u32).max(1);
        self.resize(width, height)
    }

    /// 缩放到指定宽度，高度按原比例取整。
    pub fn resize_to_width(&mut self, width: u32) -> Result<()> {
        if width == 0 {
            bail!("target width must be greater than 0");
        }
        if self.width == 0 {
            bail!("animation has zero width");
        }
        let height =
            ((self.height as f64 * width as f64 / self.width as f64).round() as u32).max(1);
        self.resize(width, height)
    }

    /// 检查模型自洽性（尺寸非零、每帧像素长度正确）。编码器入口会调用。
    pub fn validate(&self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            bail!(
                "frame size must not be zero (got {}x{})",
                self.width,
                self.height
            );
        }
        if self.frames.is_empty() {
            bail!("animation has no frames");
        }
        let expected = self.width as usize * self.height as usize * 4;
        for (index, frame) in self.frames.iter().enumerate() {
            if frame.rgba.len() != expected {
                bail!(
                    "frame {} has {} bytes, expected {} for {}x{} RGBA",
                    index + 1,
                    frame.rgba.len(),
                    expected,
                    self.width,
                    self.height
                );
            }
        }
        Ok(())
    }

    fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if width == self.width && height == self.height {
            return Ok(());
        }
        for (index, frame) in self.frames.iter_mut().enumerate() {
            let source = image::RgbaImage::from_raw(
                self.width,
                self.height,
                std::mem::take(&mut frame.rgba),
            )
            .with_context(|| {
                format!(
                    "frame {} does not match the {}x{} canvas",
                    index + 1,
                    self.width,
                    self.height
                )
            })?;
            let resized = image::imageops::resize(
                &source,
                width,
                height,
                image::imageops::FilterType::Lanczos3,
            );
            frame.rgba = resized.into_raw();
        }
        self.width = width;
        self.height = height;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(width: u32, height: u32, color: [u8; 4], delay: Duration) -> Frame {
        let rgba = color
            .iter()
            .copied()
            .cycle()
            .take(width as usize * height as usize * 4)
            .collect();
        Frame::new(rgba, delay)
    }

    #[test]
    fn duration_sums_frame_delays() {
        let mut animation = Animation::new(2, 2, LoopCount::Infinite);
        animation
            .frames
            .push(solid(2, 2, [255, 0, 0, 255], Duration::from_millis(70)));
        animation
            .frames
            .push(solid(2, 2, [0, 255, 0, 255], Duration::from_millis(30)));
        assert_eq!(animation.duration(), Duration::from_millis(100));
    }

    #[test]
    fn scale_keeps_pixel_buffer_consistent() {
        let mut animation = Animation::new(8, 4, LoopCount::Infinite);
        animation
            .frames
            .push(solid(8, 4, [1, 2, 3, 255], Duration::from_millis(50)));
        animation.scale(0.5).unwrap();
        assert_eq!((animation.width, animation.height), (4, 2));
        assert_eq!(animation.frames[0].rgba.len(), 4 * 2 * 4);
        animation.validate().unwrap();
    }

    #[test]
    fn reject_negative_scale() {
        let animation = Animation::new(4, 4, LoopCount::Infinite);
        assert!(animation.clone().scale(-1.0).is_err());
    }
}
