use anyhow::{Result, anyhow};

use crate::ast::{Comment, Container, Document, Item, Member, Value};

/// 解析 JSONC（允许 `//`、`/* */` 注释和尾逗号），保留注释与空行信息。
///
/// 开头的 UTF-8 BOM 会被忽略；解析失败时错误信息带行列号。
pub fn parse(source: &str) -> Result<Document> {
    let source = source.strip_prefix('\u{feff}').unwrap_or(source);
    let mut parser = Parser::new(source);

    let Trivia {
        comments: leading, ..
    } = parser.collect_trivia()?;
    let value = parser.parse_value()?;
    parser.skip_inline_ws();
    let trailing = parser.read_comment()?.map(Comment::inline);
    let Trivia {
        comments: dangling, ..
    } = parser.collect_trivia()?;
    if parser.pos < parser.source.len() {
        return Err(parser.error("根值之后还有多余内容"));
    }

    Ok(Document {
        leading,
        value,
        trailing,
        dangling,
    })
}

/// 把 trivia 拆成三份：注释、块前空行、注释块后的空行。
///
/// 第一段空行交给 `blank_before` 表达并从注释上清除，
/// 否则打印时同一处空行会被两个字段各写一次（曾导致不幂等）。
fn split_leading(mut trivia: Trivia) -> (Vec<Comment>, bool, bool) {
    let blank_after_leading = trivia.blank_before;
    match trivia.comments.first_mut() {
        Some(first) => {
            let blank_before = first.blank_before;
            first.blank_before = false;
            (trivia.comments, blank_before, blank_after_leading)
        }
        None => (trivia.comments, trivia.blank_before, false),
    }
}

/// 解析过程中的零散信息：注释（各自带空行标记）和"紧邻元素的空行"。
struct Trivia {
    comments: Vec<Comment>,
    blank_before: bool,
}

struct Parser<'a> {
    source: &'a str,
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(source: &'a str) -> Self {
        Self { source, pos: 0 }
    }

    fn peek(&self) -> Option<u8> {
        self.source.as_bytes().get(self.pos).copied()
    }

    fn starts_with(&self, pattern: &str) -> bool {
        self.source[self.pos..].starts_with(pattern)
    }

    fn bump(&mut self) {
        debug_assert!(self.peek().is_some_and(|byte| byte.is_ascii()));
        self.pos += 1;
    }

    /// 只跳过空格、制表符和 `\r`，不跨行 —— 用来区分"行尾注释"和"下一行注释"。
    fn skip_inline_ws(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\t' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn next_char(&mut self) -> Option<char> {
        let ch = self.source[self.pos..].chars().next()?;
        self.pos += ch.len_utf8();
        Some(ch)
    }

    /// 构造带"第 x 行第 y 列"的错误。
    fn error(&self, message: impl Into<String>) -> anyhow::Error {
        let consumed = &self.source[..self.pos.min(self.source.len())];
        let line = consumed.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let line_start = consumed.rfind('\n').map_or(0, |index| index + 1);
        let column = consumed[line_start..].chars().count() + 1;
        anyhow!("{}（第 {line} 行第 {column} 列）", message.into())
    }

    /// 收集空白和注释，直到下一个有意义的字符。
    fn collect_trivia(&mut self) -> Result<Trivia> {
        let mut comments: Vec<Comment> = Vec::new();
        let mut newlines = 0usize;
        loop {
            match self.peek() {
                Some(b' ') | Some(b'\t') | Some(b'\r') => self.pos += 1,
                Some(b'\n') => {
                    self.pos += 1;
                    newlines += 1;
                }
                Some(b'/') if self.starts_with("//") || self.starts_with("/*") => {
                    let text = self
                        .read_comment()?
                        .expect("just checked that a comment starts here");
                    comments.push(Comment {
                        text,
                        blank_before: newlines >= 2,
                    });
                    newlines = 0;
                }
                _ => break,
            }
        }
        Ok(Trivia {
            comments,
            blank_before: newlines >= 2,
        })
    }

    /// 当前位置若是注释就整段读出（不含换行），否则返回 `None` 且不移动位置。
    fn read_comment(&mut self) -> Result<Option<String>> {
        if self.starts_with("//") {
            let start = self.pos;
            self.pos += 2;
            while let Some(byte) = self.peek() {
                if byte == b'\n' {
                    break;
                }
                self.pos += 1;
            }
            Ok(Some(self.source[start..self.pos].trim_end().to_string()))
        } else if self.starts_with("/*") {
            let start = self.pos;
            match self.source[self.pos + 2..].find("*/") {
                Some(offset) => {
                    self.pos += 2 + offset + 2;
                    Ok(Some(self.source[start..self.pos].to_string()))
                }
                None => Err(self.error("块注释没有闭合")),
            }
        } else {
            Ok(None)
        }
    }

    fn parse_value(&mut self) -> Result<Value> {
        match self.peek() {
            Some(b'{') => self.parse_object(),
            Some(b'[') => self.parse_array(),
            Some(b'"') => Ok(Value::String(self.parse_string()?)),
            Some(b't') => {
                self.expect_literal("true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.expect_literal("false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.expect_literal("null")?;
                Ok(Value::Null)
            }
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(_) => {
                let ch = self.source[self.pos..].chars().next().unwrap_or('\0');
                Err(self.error(format!("意外的字符 `{ch}`")))
            }
            None => Err(self.error("输入意外结束")),
        }
    }

    fn parse_object(&mut self) -> Result<Value> {
        self.bump(); // '{'
        let mut members: Vec<Member> = Vec::new();
        let mut trivia = self.collect_trivia()?;
        let mut needs_separator = false;
        loop {
            if self.peek() == Some(b'}') {
                self.bump();
                return Ok(Value::Object(Container {
                    items: members,
                    dangling: trivia.comments,
                }));
            }
            if needs_separator {
                // 逗号可以写在注释或换行之后（前导逗号风格）。
                if self.peek() != Some(b',') {
                    return Err(self.error("对象成员之间缺少逗号"));
                }
                self.bump();
                needs_separator = false;
                trivia = self.collect_trivia()?;
                continue;
            }
            if self.peek() != Some(b'"') {
                return Err(self.error("对象的键必须是字符串"));
            }
            let key = self.parse_string()?;
            self.skip_inline_ws();
            if self.peek() != Some(b':') {
                return Err(self.error("键后面缺少冒号"));
            }
            self.bump(); // ':'
            // 冒号和值之间的注释（`"a": // 说明\n 1`）并到成员的 leading 里，
            // 位置会挪到键上方，但内容不丢。
            let between = self.collect_trivia()?;
            let value = self.parse_value()?;
            let (has_comma, trailing) = self.finish_entry()?;

            let (comments, blank_before, blank_after_leading) = split_leading(trivia);
            let mut leading = comments;
            leading.extend(between.comments);
            members.push(Member {
                leading,
                blank_before,
                blank_after_leading,
                key,
                value,
                trailing,
            });

            trivia = self.collect_trivia()?;
            needs_separator = !has_comma;
        }
    }

    fn parse_array(&mut self) -> Result<Value> {
        self.bump(); // '['
        let mut items: Vec<Item> = Vec::new();
        let mut trivia = self.collect_trivia()?;
        let mut needs_separator = false;
        loop {
            if self.peek() == Some(b']') {
                self.bump();
                return Ok(Value::Array(Container {
                    items,
                    dangling: trivia.comments,
                }));
            }
            if needs_separator {
                // 逗号可以写在注释或换行之后（前导逗号风格）。
                if self.peek() != Some(b',') {
                    return Err(self.error("数组元素之间缺少逗号"));
                }
                self.bump();
                needs_separator = false;
                trivia = self.collect_trivia()?;
                continue;
            }

            let value = self.parse_value()?;
            let (has_comma, trailing) = self.finish_entry()?;
            let (leading, blank_before, blank_after_leading) = split_leading(trivia);
            items.push(Item {
                leading,
                blank_before,
                blank_after_leading,
                value,
                trailing,
            });

            trivia = self.collect_trivia()?;
            needs_separator = !has_comma;
        }
    }

    /// 读掉可选逗号和可选行尾注释，报告是否出现过逗号。
    ///
    /// 同一行可能出现多个注释（`1 /* a */ /* b */,`），也可能夹在逗号两侧，
    /// 这些注释会被合并成一条行尾注释（原文用空格拼接），保证不丢内容。
    fn finish_entry(&mut self) -> Result<(bool, Option<Comment>)> {
        let mut has_comma = false;
        let mut trailing: Option<Comment> = None;

        loop {
            self.skip_inline_ws();
            if !has_comma && self.peek() == Some(b',') {
                self.bump();
                has_comma = true;
                continue;
            }
            match self.read_comment()? {
                Some(text) => match &mut trailing {
                    Some(existing) => {
                        existing.text.push(' ');
                        existing.text.push_str(&text);
                    }
                    None => trailing = Some(Comment::inline(text)),
                },
                None => break,
            }
        }

        Ok((has_comma, trailing))
    }

    fn parse_string(&mut self) -> Result<String> {
        debug_assert_eq!(self.peek(), Some(b'"'));
        self.bump();
        let mut text = String::new();
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("字符串没有闭合"));
            };
            match byte {
                b'"' => {
                    self.bump();
                    return Ok(text);
                }
                b'\\' => {
                    self.bump();
                    let Some(escape) = self.peek() else {
                        return Err(self.error("转义序列不完整"));
                    };
                    self.bump();
                    match escape {
                        b'"' => text.push('"'),
                        b'\\' => text.push('\\'),
                        b'/' => text.push('/'),
                        b'b' => text.push('\u{8}'),
                        b'f' => text.push('\u{c}'),
                        b'n' => text.push('\n'),
                        b'r' => text.push('\r'),
                        b't' => text.push('\t'),
                        b'u' => {
                            let ch = self.parse_unicode_escape()?;
                            text.push(ch);
                        }
                        other => {
                            return Err(self.error(format!("无效的转义序列 `\\{}`", other as char)));
                        }
                    }
                }
                0x00..=0x1f => return Err(self.error("字符串里不能直接出现控制字符")),
                _ => {
                    let ch = self
                        .next_char()
                        .ok_or_else(|| self.error("字符串没有闭合"))?;
                    text.push(ch);
                }
            }
        }
    }

    fn parse_hex4(&mut self) -> Result<u32> {
        let mut value = 0u32;
        for _ in 0..4 {
            let Some(byte) = self.peek() else {
                return Err(self.error("`\\u` 转义不完整"));
            };
            let Some(digit) = (byte as char).to_digit(16) else {
                return Err(self.error("`\\u` 转义需要 4 位十六进制"));
            };
            value = value * 16 + digit;
            self.bump();
        }
        Ok(value)
    }

    fn parse_unicode_escape(&mut self) -> Result<char> {
        let first = self.parse_hex4()?;
        let code = if (0xd800..=0xdbff).contains(&first) {
            if !self.starts_with("\\u") {
                return Err(self.error("高位代理项后面缺少 `\\u` 低位代理"));
            }
            self.pos += 2;
            let second = self.parse_hex4()?;
            if !(0xdc00..=0xdfff).contains(&second) {
                return Err(self.error("低位代理项无效"));
            }
            0x10000 + ((first - 0xd800) << 10) + (second - 0xdc00)
        } else if (0xdc00..=0xdfff).contains(&first) {
            return Err(self.error("出现了孤立的低位代理项"));
        } else {
            first
        };
        char::from_u32(code).ok_or_else(|| self.error("无效的 Unicode 码点"))
    }

    fn parse_number(&mut self) -> Result<Value> {
        let start = self.pos;
        while let Some(byte) = self.peek() {
            if matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let raw = &self.source[start..self.pos];
        if !is_valid_number(raw) {
            return Err(self.error(format!("无效的数字 `{raw}`")));
        }
        Ok(Value::Number(raw.to_string()))
    }

    fn expect_literal(&mut self, literal: &str) -> Result<()> {
        if self.starts_with(literal) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(self.error(format!("期望字面量 `{literal}`")))
        }
    }
}

/// JSON 数字文法：`-?(0|[1-9]\d*)(\.\d+)?([eE][+-]?\d+)?`
fn is_valid_number(raw: &str) -> bool {
    let bytes = raw.as_bytes();
    let mut index = 0;

    if bytes.get(index).copied() == Some(b'-') {
        index += 1;
    }

    match bytes.get(index).copied() {
        Some(b'0') => index += 1,
        Some(byte) if byte.is_ascii_digit() => {
            while bytes
                .get(index)
                .copied()
                .is_some_and(|byte| byte.is_ascii_digit())
            {
                index += 1;
            }
        }
        _ => return false,
    }

    if bytes.get(index).copied() == Some(b'.') {
        index += 1;
        if !bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            return false;
        }
        while bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
    }

    if matches!(bytes.get(index).copied(), Some(b'e') | Some(b'E')) {
        index += 1;
        if matches!(bytes.get(index).copied(), Some(b'+') | Some(b'-')) {
            index += 1;
        }
        if !bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            return false;
        }
        while bytes
            .get(index)
            .copied()
            .is_some_and(|byte| byte.is_ascii_digit())
        {
            index += 1;
        }
    }

    index == bytes.len()
}
