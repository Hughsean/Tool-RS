use crate::ast::{Comment, Container, Document, Item, Member, Value};

/// 打印选项。
#[derive(Debug, Clone, Copy)]
pub struct FormatOptions {
    /// 每层缩进的空格数。
    pub indent_width: usize,
    /// 对象字段按键名字母序排序（忽略大小写）。
    pub sort_keys: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            indent_width: 2,
            sort_keys: true,
        }
    }
}

/// 把语法树打印成 JSONC 文本，保证以换行结尾。
pub fn print(document: &Document, options: &FormatOptions) -> String {
    let mut printer = Printer {
        options,
        out: String::new(),
    };
    printer.document(document);
    printer.out
}

/// 单行最大宽度，短数组尽量内联。
const MAX_INLINE_WIDTH: usize = 76;

struct Printer<'a> {
    options: &'a FormatOptions,
    out: String,
}

impl Printer<'_> {
    fn document(&mut self, document: &Document) {
        self.leading(&document.leading, 0, true);
        self.value(&document.value, 0);
        if let Some(comment) = &document.trailing {
            self.out.push(' ');
            self.out.push_str(&comment.text);
        }
        self.out.push('\n');
        self.dangling(&document.dangling, 0);
    }

    fn value(&mut self, value: &Value, level: usize) {
        match value {
            Value::Null => self.out.push_str("null"),
            Value::Bool(flag) => self.out.push_str(if *flag { "true" } else { "false" }),
            Value::Number(raw) => self.out.push_str(raw),
            Value::String(text) => write_quoted(&mut self.out, text),
            Value::Array(container) => self.array(container, level),
            Value::Object(container) => self.object(container, level),
        }
    }

    fn array(&mut self, container: &Container<Item>, level: usize) {
        if container.items.is_empty() && container.dangling.is_empty() {
            self.out.push_str("[]");
            return;
        }
        if let Some(parts) = inline_parts(container) {
            let inline = format!("[{}]", parts.join(", "));
            if self.column_width(level) + inline.chars().count() <= MAX_INLINE_WIDTH {
                self.out.push_str(&inline);
                return;
            }
        }

        self.out.push_str("[\n");
        let last = container.items.len().saturating_sub(1);
        for (index, item) in container.items.iter().enumerate() {
            if index > 0 && item.blank_before {
                self.out.push('\n');
            }
            self.leading(&item.leading, level + 1, index == 0);
            self.indent(level + 1);
            self.value(&item.value, level + 1);
            if index < last {
                self.out.push(',');
            }
            self.inline_comment(&item.trailing);
            self.out.push('\n');
        }
        self.dangling(&container.dangling, level + 1);
        self.indent(level);
        self.out.push(']');
    }

    fn object(&mut self, container: &Container<Member>, level: usize) {
        if container.items.is_empty() && container.dangling.is_empty() {
            self.out.push_str("{}");
            return;
        }

        let mut members: Vec<&Member> = container.items.iter().collect();
        if self.options.sort_keys {
            // 先按小写比较（忽略大小写），同前缀再按原始字节序，保证稳定且可预测。
            members.sort_by_cached_key(|member| (member.key.to_lowercase(), member.key.clone()));
        }

        self.out.push_str("{\n");
        let last = members.len().saturating_sub(1);
        for (index, member) in members.iter().enumerate() {
            if index > 0 && member.blank_before {
                self.out.push('\n');
            }
            self.leading(&member.leading, level + 1, index == 0);
            self.indent(level + 1);
            write_quoted(&mut self.out, &member.key);
            self.out.push_str(": ");
            self.value(&member.value, level + 1);
            if index < last {
                self.out.push(',');
            }
            self.inline_comment(&member.trailing);
            self.out.push('\n');
        }
        self.dangling(&container.dangling, level + 1);
        self.indent(level);
        self.out.push('}');
    }

    fn leading(&mut self, comments: &[Comment], level: usize, first_in_block: bool) {
        for (index, comment) in comments.iter().enumerate() {
            if comment.blank_before && !(first_in_block && index == 0) {
                self.out.push('\n');
            }
            self.indent(level);
            self.out.push_str(&comment.text);
            self.out.push('\n');
        }
    }

    fn dangling(&mut self, comments: &[Comment], level: usize) {
        for (index, comment) in comments.iter().enumerate() {
            if comment.blank_before && index > 0 {
                self.out.push('\n');
            }
            self.indent(level);
            self.out.push_str(&comment.text);
            self.out.push('\n');
        }
    }

    fn inline_comment(&mut self, comment: &Option<Comment>) {
        if let Some(comment) = comment {
            self.out.push(' ');
            self.out.push_str(&comment.text);
        }
    }

    fn indent(&mut self, level: usize) {
        for _ in 0..level * self.options.indent_width {
            self.out.push(' ');
        }
    }

    /// 当前层缩进折算成的列数，用于判断要不要内联数组。
    fn column_width(&self, level: usize) -> usize {
        level * self.options.indent_width
    }
}

/// 数组能否压成一行：没有注释、没有空行、元素都是标量。
fn inline_parts(container: &Container<Item>) -> Option<Vec<String>> {
    if !container.dangling.is_empty() {
        return None;
    }
    let mut parts = Vec::with_capacity(container.items.len());
    for (index, item) in container.items.iter().enumerate() {
        if !item.leading.is_empty() || item.trailing.is_some() {
            return None;
        }
        if index > 0 && item.blank_before {
            return None;
        }
        parts.push(inline_value(&item.value)?);
    }
    Some(parts)
}

fn inline_value(value: &Value) -> Option<String> {
    let mut text = String::new();
    match value {
        Value::Null => text.push_str("null"),
        Value::Bool(flag) => text.push_str(if *flag { "true" } else { "false" }),
        Value::Number(raw) => text.push_str(raw),
        Value::String(string) => write_quoted(&mut text, string),
        Value::Array(_) | Value::Object(_) => return None,
    }
    Some(text)
}

fn write_quoted(out: &mut String, text: &str) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            ch if (ch as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", ch as u32));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}
