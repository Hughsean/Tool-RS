//! 保留注释的 JSONC 解析与格式化。
//!
//! 用法是"解析成 AST → 变换 → 重新打印"，而不是字符串替换：
//!
//! ```no_run
//! let formatted = tool_jsonc::format(r#"{"b": 1, "a": 2}"#, &Default::default())?;
//! # Ok::<(), anyhow::Error>(())
//! ```
//!
//! 特性：
//! - 支持 `//`、`/* */` 注释、尾逗号、UTF-8 BOM；
//! - 注释跟着所属字段走，排序不会把它留在原地；
//! - 对象字段按键名字母序（忽略大小写）排序，可用 [`FormatOptions::sort_keys`] 关闭；
//! - 数字保留原始字面量，不做浮点转换；
//! - 输出不带尾逗号，所以无注释时结果仍是合法 JSON。

mod ast;
mod parser;
mod printer;

pub use ast::{Comment, Container, Document, Item, Member, Value};
pub use parser::parse;
pub use printer::{FormatOptions, print};

/// 一步到位：解析 + 打印。返回的文本一定以换行结尾。
pub fn format(source: &str, options: &FormatOptions) -> anyhow::Result<String> {
    let document = parse(source)?;
    Ok(print(&document, options))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formatting(source: &str) -> String {
        format(source, &FormatOptions::default()).expect("源代码应当能解析")
    }

    #[test]
    fn sorts_keys_case_insensitively() {
        let output = formatting(r#"{"b": 1, "A": 2, "a": 3}"#);
        assert_eq!(output, "{\n  \"A\": 2,\n  \"a\": 3,\n  \"b\": 1\n}\n");
    }

    #[test]
    fn sorts_nested_objects_and_objects_in_arrays() {
        let output = formatting(r#"{"z": {"y": 1, "x": 2}, "a": [{"q": 1, "p": 2}]}"#);
        let position = |needle: &str| {
            output
                .find(needle)
                .unwrap_or_else(|| panic!("输出里应该有 {needle}：\n{output}"))
        };
        assert!(position("\"p\"") < position("\"q\""));
        assert!(position("\"x\"") < position("\"y\""));
        assert!(position("\"a\"") < position("\"z\""));
    }

    #[test]
    fn keep_order_flag_disables_sorting() {
        let options = FormatOptions {
            sort_keys: false,
            ..FormatOptions::default()
        };
        let output = format(r#"{"b": 1, "a": 2}"#, &options).unwrap();
        assert_eq!(output, "{\n  \"b\": 1,\n  \"a\": 2\n}\n");
    }

    #[test]
    fn honours_indent_width() {
        let options = FormatOptions {
            indent_width: 4,
            ..FormatOptions::default()
        };
        assert_eq!(
            format(r#"{"a": {"b": 1}}"#, &options).unwrap(),
            "{\n    \"a\": {\n        \"b\": 1\n    }\n}\n"
        );
    }

    #[test]
    fn comments_follow_the_member_they_belong_to() {
        let source = "{\n  // 关于 b\n  \"b\": 1,\n  \"a\": 2 // 关于 a\n}\n";
        assert_eq!(
            formatting(source),
            "{\n  \"a\": 2, // 关于 a\n  // 关于 b\n  \"b\": 1\n}\n"
        );
    }

    #[test]
    fn keeps_dangling_comment_before_closing_brace() {
        let source = "{\n  \"a\": 1\n  // 结束\n}\n";
        assert_eq!(formatting(source), "{\n  \"a\": 1\n  // 结束\n}\n");
    }

    #[test]
    fn drops_trailing_commas() {
        assert_eq!(formatting(r#"{"a": 1,}"#), "{\n  \"a\": 1\n}\n");
        assert_eq!(formatting("[1, 2,]"), "[1, 2]\n");
    }

    #[test]
    fn keeps_short_arrays_inline() {
        assert_eq!(
            formatting(r#"{"tags": ["a", "b"], "empty": []}"#),
            "{\n  \"empty\": [],\n  \"tags\": [\"a\", \"b\"]\n}\n"
        );
    }

    #[test]
    fn expands_arrays_that_carry_comments() {
        assert_eq!(formatting("[1, // 一\n 2]"), "[\n  1, // 一\n  2\n]\n");
    }

    #[test]
    fn decodes_and_reencodes_string_escapes() {
        assert_eq!(formatting(r#""a\u00e9\n""#), "\"aé\\n\"\n");
        assert_eq!(formatting(r#""\ud83d\ude00""#), "\"😀\"\n");
    }

    #[test]
    fn keeps_number_literals_verbatim() {
        assert_eq!(
            formatting("[1.50, 1e3, -0, 2E-2]"),
            "[1.50, 1e3, -0, 2E-2]\n"
        );
    }

    #[test]
    fn handles_empty_containers_and_bom() {
        assert_eq!(
            formatting("\u{feff}{\"a\": {}, \"b\": []}"),
            "{\n  \"a\": {},\n  \"b\": []\n}\n"
        );
    }

    #[test]
    fn preserves_blank_lines_attached_to_members() {
        let source = "{\n  \"b\": 1,\n\n  \"c\": 2\n}\n";
        assert_eq!(formatting(source), "{\n  \"b\": 1,\n\n  \"c\": 2\n}\n");
    }

    #[test]
    fn blank_lines_around_a_comment_block_are_not_doubled() {
        // 回归测试：注释块前后各有一个空行时，两者曾写进同一个字段，
        // 导致第二遍格式化会少掉一个空行（不幂等）。
        let source = "{\n  \"b\": 1,\n\n  // 注释块\n  // 第二行\n\n  \"a\": 2\n}\n";
        let once = formatting(source);
        // 注释块属于 "a"，排序后 "a" 在前，块前的空行被"块首"规则抑制；
        // 关键是块后的空行保留且不重复。
        assert_eq!(
            once,
            "{\n  // 注释块\n  // 第二行\n\n  \"a\": 2,\n  \"b\": 1\n}\n"
        );
        assert_eq!(format(&once, &FormatOptions::default()).unwrap(), once);
    }

    #[test]
    fn formatting_is_idempotent_on_a_busy_document() {
        let source = r#"{
  // 头部
  "zeta": [1, 2, 3],

  /* 块注释 */
  "Alpha": {
    "y": null
    // 收尾
  },
  "beta": [
    // 数组注释
    "b",
    "a",
  ],
  "url": "https://example.com/a?b=1" // 行尾
}
"#;
        let once = formatting(source);
        let twice = format(&once, &FormatOptions::default()).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn formatting_is_idempotent_on_edge_cases() {
        let cases = [
            "{}",
            "[]",
            "{\n  // 只有注释\n}",
            "[\n  // 只有注释\n]",
            "{\n  \"a\": 1\n\n  // 结尾注释\n}",
            "{\n  /* 多行\n     块注释 */\n  \"a\": 1\n}",
            "[\n  1 /* a */ /* b */,\n  2\n]",
            "[\n  1 /* 前导逗号 */\n  , 2\n]",
            "{\n  \"a\": 1 /* 注释 */\n  , \"b\": 2\n}",
            "[\n  1,\n]",
            "{\"a\": {\"b\": {\"c\": [1, [2, 3]]}}}",
            "{\"a\": 1, \"a\": 2}",
            "\u{feff}{\"b\": 1, \"a\": 2}",
            "{\"a\":1}\r\n",
            "{\n\n  \"a\": 1,\n\n\n  \"b\": 2\n}",
            "[\n  \"https://x/a?b=1\",\n  \"a\"\n]",
            "{\n  \"a\": \"\\u00e9\\ud83d\\ude00\"\n}",
            "{\n  \"b\": 1,\n\n  // 注释块\n\n  \"a\": 2\n}",
        ];

        for source in cases {
            let once = format(source, &FormatOptions::default())
                .unwrap_or_else(|error| panic!("输入应当能解析：{source:?}：{error}"));
            let twice = format(&once, &FormatOptions::default())
                .unwrap_or_else(|error| panic!("第一遍输出应当能再解析：{source:?}：{error}"));
            assert_eq!(once, twice, "不幂等：{source:?}");
        }
    }

    #[test]
    fn zero_indent_is_idempotent() {
        let options = FormatOptions {
            indent_width: 0,
            ..FormatOptions::default()
        };
        let once = format(r#"{"b": {"y": 1, "x": 2}, "a": [1, 2]}"#, &options).unwrap();
        let twice = format(&once, &options).unwrap();
        assert_eq!(once, twice);
    }

    #[test]
    fn reports_line_and_column_for_syntax_errors() {
        let error = format("{\n  \"a\": 1\n  \"b\": 2\n}", &FormatOptions::default()).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("第 3 行"), "{message}");
        assert!(message.contains("缺少逗号"), "{message}");
    }

    #[test]
    fn rejects_broken_input() {
        let cases = [
            (r#"{"a"#, "字符串没有闭合"),
            (r#"{"a" 1}"#, "缺少冒号"),
            ("1 2", "多余内容"),
            ("[01]", "无效的数字"),
            ("/* 没闭合", "块注释没有闭合"),
            (r#""\ud800""#, "低位代理"),
        ];
        for (source, expected) in cases {
            let error = format(source, &FormatOptions::default())
                .expect_err(&format!("{source} 应当解析失败"));
            assert!(
                error.to_string().contains(expected),
                "{source} 的错误信息应包含 {expected}，实际：{error}"
            );
        }
    }
}
