//! 媒体格式的通用抽象层。
//!
//! 分三层，新增格式时只往下加模块，CLI 侧不感知像素布局和帧合成细节：
//!
//! 1. 模型：[`Animation`] / [`Frame`] / [`LoopCount`]，与容器格式无关，
//!    是所有媒体工具之间的交换格式（RGBA8 全画布帧 + 每帧时长）；
//! 2. 解码器：把某种容器格式还原成 [`Animation`]，如 [`webp::decode`]；
//! 3. 编码器：把 [`Animation`] 写成某种容器格式，如 [`gif::encode`]。
//!
//! 模型故意不暴露 `image` / `image-webp` / `gif` 的类型，
//! 这样换掉底层库不会波及使用方。

mod animation;
pub mod gif;
pub mod webp;

pub use animation::{Animation, Frame, LoopCount};
