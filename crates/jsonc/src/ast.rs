//! 保留了注释的 JSONC 语法树。
//!
//! 目标是把"格式细节"和"变换逻辑"隔开：解析时注释当作 trivia 挂在所属字段上，
//! 这样排序字段时注释会跟着一起走，而不是被丢掉或错位。

/// JSONC 值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Null,
    Bool(bool),
    /// 保留原始字面量（如 `1.50`、`1e3`），避免浮点精度损失。
    Number(String),
    /// 已解码的文本，打印时重新转义。
    String(String),
    Array(Container<Item>),
    Object(Container<Member>),
}

/// 数组 / 对象共用的容器：元素列表 + 收尾括号前的注释。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container<T> {
    pub items: Vec<T>,
    /// 收尾括号之前的注释，例如 `[\n  1\n  // 收尾说明\n]`。
    pub dangling: Vec<Comment>,
}

impl<T> Default for Container<T> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            dangling: Vec::new(),
        }
    }
}

/// 数组元素。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// 该元素上方的注释。
    pub leading: Vec<Comment>,
    /// 与上一个元素之间是否隔了空行（有注释时指注释块之前）。
    pub blank_before: bool,
    /// `leading` 的最后一个注释与值之间是否有空行（`leading` 为空时无意义）。
    pub blank_after_leading: bool,
    pub value: Value,
    /// 同一行末尾的注释（`1, // 说明`）。
    pub trailing: Option<Comment>,
}

/// 对象成员。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    /// 该成员上方的注释。
    pub leading: Vec<Comment>,
    /// 与上一个成员之间是否隔了空行（有注释时指注释块之前）。
    pub blank_before: bool,
    /// `leading` 的最后一个注释与键之间是否有空行（`leading` 为空时无意义）。
    pub blank_after_leading: bool,
    /// 已解码的键名，打印时重新转义。
    pub key: String,
    pub value: Value,
    /// 同一行末尾的注释（`"a": 1, // 说明`）。
    pub trailing: Option<Comment>,
}

/// 一条注释的原文及其空行信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    /// 含 `//` 或 `/* */` 的原文。
    pub text: String,
    /// 与上一行之间是否隔了空行。
    pub blank_before: bool,
}

impl Comment {
    /// 行尾注释：不涉及空行。
    pub fn inline(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            blank_before: false,
        }
    }
}

/// 整份文档：根值，加上它前后的注释。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// 根值上方的注释。
    pub leading: Vec<Comment>,
    pub value: Value,
    /// 与根值同一行末尾的注释。
    pub trailing: Option<Comment>,
    /// 根值之后的注释。
    pub dangling: Vec<Comment>,
}

impl Value {
    pub fn as_object(&self) -> Option<&Container<Member>> {
        match self {
            Value::Object(container) => Some(container),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Container<Item>> {
        match self {
            Value::Array(container) => Some(container),
            _ => None,
        }
    }
}
