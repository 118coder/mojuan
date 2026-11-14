//! 代码高亮：把代码切成「语义 token」，颜色由 UI 主题决定。
//!
//! 这样做的好处是同一份解析结果可以同时适配暗色 / 亮色主题，
//! 而且配色由我们统一设计，不受 syntect 内置主题限制。

use std::sync::OnceLock;

use syntect::parsing::{ParseState, ScopeStack, SyntaxSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenStyle {
    Plain,
    Comment,
    Keyword,
    Str,
    Number,
    Constant,
    Function,
    Type,
    Tag,
    Attribute,
    Variable,
    Operator,
    Punct,
    Invalid,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub text: String,
    pub style: TokenStyle,
}

fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// 高亮整段代码，返回按行切好的 token
pub fn highlight(code: &str, lang: &str) -> Vec<Vec<Token>> {
    let ss = syntax_set();
    let lang = lang.trim();
    let syntax = if lang.is_empty() {
        None
    } else {
        ss.find_syntax_by_token(lang)
            .or_else(|| ss.find_syntax_by_extension(lang))
    };
    let syntax = match syntax {
        Some(s) => s,
        None => return plain_lines(code),
    };

    let mut state = ParseState::new(syntax);
    let mut stack = ScopeStack::new();
    let mut out: Vec<Vec<Token>> = Vec::new();

    for line in code.split_inclusive('\n') {
        let ops = match state.parse_line(line, ss) {
            Ok(ops) => ops,
            Err(_) => {
                out.push(vec![Token {
                    text: line.to_string(),
                    style: TokenStyle::Plain,
                }]);
                continue;
            }
        };

        let mut tokens: Vec<Token> = Vec::new();
        let mut prev = 0usize;
        for (idx, op) in ops {
            if idx > prev {
                push_token(&mut tokens, &line[prev..idx], classify(&stack));
            }
            let _ = stack.apply(&op);
            prev = idx.max(prev);
        }
        if prev < line.len() {
            push_token(&mut tokens, &line[prev..], classify(&stack));
        }
        out.push(merge(tokens));
    }

    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

fn plain_lines(code: &str) -> Vec<Vec<Token>> {
    code.split_inclusive('\n')
        .map(|l| {
            vec![Token {
                text: l.to_string(),
                style: TokenStyle::Plain,
            }]
        })
        .collect()
}

fn push_token(tokens: &mut Vec<Token>, text: &str, style: TokenStyle) {
    if text.is_empty() {
        return;
    }
    tokens.push(Token {
        text: text.to_string(),
        style,
    });
}

fn merge(tokens: Vec<Token>) -> Vec<Token> {
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    for t in tokens {
        match out.last_mut() {
            Some(last) if last.style == t.style => last.text.push_str(&t.text),
            _ => out.push(t),
        }
    }
    out
}

fn classify(stack: &ScopeStack) -> TokenStyle {
    for scope in stack.as_slice().iter().rev() {
        let s = scope.to_string();
        let st = classify_one(&s);
        if st != TokenStyle::Plain {
            return st;
        }
    }
    TokenStyle::Plain
}

fn classify_one(s: &str) -> TokenStyle {
    // 越具体的规则越靠前
    if s.contains("comment") {
        return TokenStyle::Comment;
    }
    if s.contains("invalid") {
        return TokenStyle::Invalid;
    }
    if s.contains("string") || s.contains("constant.character") {
        return TokenStyle::Str;
    }
    if s.contains("constant.numeric") {
        return TokenStyle::Number;
    }
    if s.contains("constant.language") {
        return TokenStyle::Constant;
    }
    if s.contains("keyword.operator") {
        return TokenStyle::Operator;
    }
    if s.contains("keyword") || s.contains("storage") {
        return TokenStyle::Keyword;
    }
    if s.contains("entity.name.function") || s.contains("support.function") {
        return TokenStyle::Function;
    }
    if s.contains("entity.name.type")
        || s.contains("entity.name.class")
        || s.contains("entity.name.struct")
        || s.contains("entity.name.enum")
        || s.contains("entity.name.namespace")
        || s.contains("support.type")
        || s.contains("support.class")
        || s.contains("entity.other.inherited-class")
    {
        return TokenStyle::Type;
    }
    if s.contains("entity.name.tag") {
        return TokenStyle::Tag;
    }
    if s.contains("entity.other.attribute-name") {
        return TokenStyle::Attribute;
    }
    if s.contains("constant") {
        return TokenStyle::Constant;
    }
    if s.contains("variable") || s.contains("variable.parameter") {
        return TokenStyle::Variable;
    }
    if s.contains("punctuation") || s.contains("meta.brace") {
        return TokenStyle::Punct;
    }
    TokenStyle::Plain
}

/// 代码块角标显示的语言名
pub fn display_lang(lang: &str) -> String {
    let l = lang.trim().to_ascii_lowercase();
    let name = match l.as_str() {
        "" => "TEXT",
        "rs" => "RUST",
        "py" => "PYTHON",
        "js" | "javascript" => "JAVASCRIPT",
        "jsx" => "JSX",
        "ts" | "typescript" => "TYPESCRIPT",
        "tsx" => "TSX",
        "sh" | "bash" | "shell" | "zsh" => "SHELL",
        "ps1" | "powershell" => "POWERSHELL",
        "cpp" | "c++" | "cxx" => "C++",
        "cs" | "csharp" => "C#",
        "yml" | "yaml" => "YAML",
        "md" | "markdown" => "MARKDOWN",
        "html" | "htm" => "HTML",
        "css" => "CSS",
        "json" | "jsonc" => "JSON",
        "toml" | "ini" => "TOML",
        "sql" => "SQL",
        "go" | "golang" => "GO",
        "java" => "JAVA",
        "kt" | "kotlin" => "KOTLIN",
        "text" | "plain" | "txt" => "TEXT",
        other => return other.to_ascii_uppercase(),
    };
    name.to_string()
}
