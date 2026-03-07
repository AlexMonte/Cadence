use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CodeExpr {
    Literal(Value),
    Call {
        func: String,
        args: Vec<CodeExpr>,
    },
    Method {
        receiver: Box<CodeExpr>,
        method: String,
        args: Vec<CodeExpr>,
    },
    Ident(String),
    Raw(String),
}

impl CodeExpr {
    pub fn render(&self) -> String {
        match self {
            CodeExpr::Literal(value) => match value {
                Value::String(v) => format!("\"{}\"", v.replace('"', "\\\"")),
                other => other.to_string(),
            },
            CodeExpr::Call { func, args } => {
                let rendered = args
                    .iter()
                    .map(CodeExpr::render)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{func}({rendered})")
            }
            CodeExpr::Method {
                receiver,
                method,
                args,
            } => {
                let rendered = args
                    .iter()
                    .map(CodeExpr::render)
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}.{}({})", receiver.render(), method, rendered)
            }
            CodeExpr::Ident(name) => name.clone(),
            CodeExpr::Raw(raw) => raw.clone(),
        }
    }
}
