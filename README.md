# Tool-RS

一堆小工具的 Rust 合集。顶层是 Cargo workspace，**一个工具一个 crate**，
互不干扰地加、删、改。

## 目录结构

```
.
├── Cargo.toml              # 只声明 workspace（members / 统一版本 / 统一依赖）
├── .cargo/config.toml      # cargo 别名，缩短日常命令
├── crates/
│   ├── common/             # 共享库 tool-common：stdin/stdout 约定、通用 IO
│   ├── media/              # 共享库 tool-media：动图模型 + webp/gif 编解码
│   ├── case/               # 工具：命名风格转换
│   ├── count/              # 工具：行/单词/字符/字节统计
│   └── webp2gif/           # 工具：动图 WebP -> 动图 GIF
```

约定：`crates/<名字>/` 的包名就是工具名，也就是可执行文件名。

## 运行某个工具

```sh
cargo t case -- --help          # = cargo run -p case -- --help
cargo t count -- README.md
cargo t count -- -              # 从 stdin 读
echo "Hello World" | cargo t case snake -

cargo t webp2gif -- in.webp out.gif
cargo t webp2gif -- in.webp out.gif --width 320 --fps 12 --loop 0
cargo t webp2gif < in.webp > out.gif          # 省略参数即 stdin/stdout
```

```sh
cargo b case                    # 只构建这个工具
cargo br case                   # release 构建
cargo build                     # 构建全部工具
```

## 加一个新工具

1. 建目录和清单（手写比 `cargo new` 更省事，不必额外指定 workspace）：

   ```
   crates/<tool>/Cargo.toml
   crates/<tool>/src/main.rs
   ```

   ```toml
   [package]
   name = "<tool>"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   publish = false

   [dependencies]
   # 版本统一在顶层 [workspace.dependencies] 里维护
   anyhow.workspace = true
   clap.workspace = true
   tool-common = { path = "../common" }   # 需要共享代码时才加
   ```

2. `cargo t <tool>` 直接跑，无需改动顶层 `Cargo.toml`（`members = ["crates/*"]` 自动收录）。

3. 需要新依赖时，先在顶层 `[workspace.dependencies]` 加版本，再在工具里以
   `dep.workspace = true` 引用，避免各工具版本漂移。

## 媒体工具的分层

`crates/media` 把"媒体格式"和"像素/帧细节"隔离在 CLI 之外，共三层：

1. **模型**：`Animation { width, height, loop_count, frames }` +
   `Frame { rgba, delay }`。帧永远是**合成好的整幅画布 RGBA8**，与容器格式无关。
2. **解码器**：`tool_media::webp::decode(&[u8]) -> Result<Animation>`。
   动图的 disposal/blending 由解码器内部处理，上层拿不到差分帧。
3. **编码器**：`tool_media::gif::encode(&Animation, &GifOptions) -> Result<Vec<u8>>`。
   调色板量化交给 `gif` crate（NeuQuant），透明度压成 1-bit。

这样加新格式（apng / webm / 图片序列）时只加一个 `media::<format>` 模块，
`webp2gif` 这类 CLI 只负责参数解析和 IO，不碰像素布局。

几条已经踩平的坑，写在新解码器里要留意：

- `image` 的 WebP 动图封装读不到循环次数，所以解码走 `image-webp`（它暴露
  `loop_count()`），`image` 只用来做 `imageops::resize`。
- GIF 的帧延迟是百分之一秒，且多数播放器把 `< 2cs` 兜底成 `10cs`，
  编码时四舍五入后夹到 `[2, 65535]`，否则快动画会莫名其妙变慢。
- GIF 的 NETSCAPE 扩展记录的是"额外重复次数"，`Times(n)` 要减一；
  `Finite(0)` 会被 `gif` 直接省略扩展，正好等于只播放一遍。

## 设计要点

- **工具之间不互相依赖**。共享逻辑只往上抽到 `crates/common` 或按领域拆成
  `crates/<domain>`（如 `media`），不要出现 `tool-a` 依赖 `tool-b` 的情况。
- **顶层 Cargo.toml 不放 `[package]`**，这样根本不存在"根 bin"这种半工具，
  也就没有 `cargo run` 该跑谁的歧义。
- **依赖随工具走**：`case` 装了 clap 不影响只依赖 std 的工具，CI 也可以只构建
  改动的 crate（`cargo build -p <tool>`）。
- 需要"一键装到 PATH"时用 `cargo install --path crates/<tool>`，
  所以每个 crate 都是独立可安装单元。

## 后续可选

- `crates/xtask`：写 `cargo xtask new <tool>` 自动生成工具骨架，省掉手写 Cargo.toml。
- 想在 `cargo run`（不带参数）时有统一入口，就再加一个 `crates/toolbox` 的
  多子命令 bin 作为 hub，让各工具同时暴露成 lib；纯 bins 方案下则不需要。
