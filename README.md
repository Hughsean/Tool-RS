# Tool-RS

一堆小工具的 Rust 合集。**一个根包，多个 bin**：每个工具的源码是 `bin/` 下的一个文件，
共享逻辑下沉成独立 crate。

## 目录结构

```
.
├── Cargo.toml              # 根包 + workspace：[[bin]] 清单、统一版本/依赖、default-run
├── .cargo/config.toml      # cargo 别名，缩短日常命令
├── bin/
│   ├── case.rs             # 工具：命名风格转换
│   ├── count.rs            # 工具：行/单词/字符/字节统计
│   ├── jsonfmt.rs          # 工具：JSONC 格式化 + 字段排序
│   └── webp2gif.rs         # 工具：动图 WebP -> 动图 GIF
└── crates/
    ├── common/             # 共享库 tool-common：stdin/stdout 约定、通用 IO
    ├── jsonc/              # 共享库 tool-jsonc：保留注释的 JSONC AST + 格式化
    └── media/              # 共享库 tool-media：动图模型 + webp/gif 编解码
```

约定：`bin/<tool>.rs` 的文件名就是 bin 名，两者在 `[[bin]]` 里显式声明。

## 运行某个工具

```sh
cargo run                                  # 跑 default-run（当前是 webp2gif）
cargo t webp2gif -- in.webp out.gif        # = cargo run --bin webp2gif -- in.webp out.gif
cargo t case snake -
cargo t count -- README.md
cargo t webp2gif -- in.webp out.gif --width 320 --fps 12 --loop 0
echo "Hello World" | cargo t case snake -
cargo t webp2gif < in.webp > out.gif       # 省略参数即 stdin/stdout

cargo t jsonfmt -- in.jsonc out.json            # 默认 2 空格缩进 + 字段字母序
echo '{"b":1,"a":2}' | cargo t jsonfmt         # 管道
cargo t jsonfmt -- in.jsonc --indent 4 --keep-order
```

```sh
cargo b case                               # 只构建这个工具
cargo br webp2gif                          # release 构建
cargo build --bins                         # 构建全部工具
cargo install --path .                     # 一次装全部工具进 PATH
```

`default-run` 决定裸 `cargo run` 跑谁，改根 `Cargo.toml` 里那一行即可。

### stdin 的行为约定

- `webp2gif` / `jsonfmt` 没给输入文件、而 stdin 是**交互终端**时，会直接打印用法并
  以退出码 2 退出，不会傻等输入（否则表现就是“卡住”）。判断逻辑在
  `tool_common::needs_explicit_input`，两个工具共用。
- stdin 是管道但没数据时，报“输入为空”而不是难懂的解析错误。
- 终端判断靠 `std::io::IsTerminal`。在 mintty（Git Bash 自带终端）这类环境下，
  原生 Windows 程序看到的 stdin 是管道而非终端，拿不到 TTY 信号，
  Windows Terminal / Zed 内置终端（ConPTY）则正常。

## 加一个新工具

1. 新建 `bin/<tool>.rs`：

   ```rust
   use clap::Parser;

   /// 一句话说明这个工具干什么。
   #[derive(Debug, Parser)]
   #[command(version, about)]
   struct Args {
       /// 输入文件；省略或 `-` 表示 stdin
       input: Option<String>,
   }

   fn main() -> anyhow::Result<()> {
       let args = Args::parse();
       let text = tool_common::read_input(args.input.as_deref())?;
       println!("{text}");
       Ok(())
   }
   ```

2. 在根 `Cargo.toml` 加四行：

   ```toml
   [[bin]]
   name = "<tool>"
   path = "bin/<tool>.rs"
   ```

3. `cargo t <tool> -- --help` 直接跑。

工具长到单个文件放不下时，改成目录形式，`[[bin]]` 指向入口文件即可：

```toml
[[bin]]
name = "foo"
path = "bin/foo/main.rs"      # bin/foo/main.rs 里 `mod parser;` → bin/foo/parser.rs
```

需要新依赖时用 `cargo add <dep>`（加在根包上，全工具共享）；
希望集中管理版本，就写进 `[workspace.dependencies]` 再以 `dep.workspace = true` 引用。

## 依赖是包级别的（重要）

单包多 bin 的代价：**根包的所有依赖，每个 bin 都会参与编译**。
实测过：`gif` 只有在 `webp2gif` 里用到，但 `cargo build --bin case` 依然会编译 `gif`
和它的 `color_quant`/`weezl`。`image` + `image-webp` + `gif` 这套是编译时间的大头，
以后加依赖前先想一下：这是不是只服务某一个工具？

如果哪天某个工具的依赖栈和别的工具差别很大（比如要 `rayon` 或某个库的另一个大版本），
两个出口：

- **feature 门控**：`dep = { version = "...", optional = true }`，`[[bin]]` 上写
  `required-features = ["heavy"]`，日常命令带 `--features heavy`；
- **退回独立 crate**：把它拆成 `crates/<tool>`，`cargo run -p <tool>`，
  享受依赖隔离，代价是多一个 Cargo.toml。

## JSONC 工具的分层

`crates/jsonc` 把"带注释的配置"当成和媒体一样的结构化数据来处理，不靠字符串替换：

1. **AST**：`Value` / `Container<T>` / `Member` / `Item` / `Comment`。
   注释是挂在字段上的 trivia（`leading` / `trailing` / 容器级 `dangling`），
   所以**排序字段时注释跟着走**，不会停在原地或丢掉。
2. **解析器**：`tool_jsonc::parse(&str) -> Result<Document>`。支持 `//`、`/* */`、
   尾逗号、UTF-8 BOM；数字保留原始字面量（`2.50` 不会变成 `2.5`）；
   错误信息带行列号。
3. **打印器**：`tool_jsonc::print(&Document, &FormatOptions) -> String`。
   对象字段按 `(小写键名, 原键名)` 排序，即忽略大小写的字母序；
   `sort_keys: false` 关掉排序；数组在"无注释 + 全是标量 + 不超宽"时内联成一行。

两个约定：

- **输出不带尾逗号**，所以无注释时结果仍是合法 JSON，可以直接当 JSON 用；
- **幂等**：格式化两次结果完全一致（端到端验证过 `diff` 为空）。

已知的位置取舍：写在冒号后、换行前的注释（`"a": // 说明\n 1`）会被归到
该成员的 `leading`，打印到键的上方；跨行块注释保留原缩进，不重新对齐。

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

- **共享逻辑下沉成 crate**，不要出现 `bin/a.rs` 依赖 `bin/b.rs`。
  按领域拆：通用 IO 进 `common`，媒体相关进 `media`。
- **共享库互相独立**：`tool-media` 不依赖 `tool-common`，一个库只做一件事。
- **版本集中在 `[workspace.dependencies]`**，避免各 crate 漂移出不同的 `clap`/`anyhow`。
- 想让裸 `cargo run` 有个统一入口（比如 `cargo run -- <tool> <args>`），
  可以再加一个 `bin/toolbox.rs` 做子命令分发；现在靠 `default-run` + 别名已经够用。
