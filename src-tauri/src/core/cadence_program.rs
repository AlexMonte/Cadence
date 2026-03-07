use tile_graph::code_expr::CodeExpr;

use crate::commands::TerminalStrategy;
use crate::core::tricks::{CompiledTrick, trick_param_defaults};

#[derive(Debug, Clone)]
pub struct CadenceProgram {
    pub setup: Vec<CadenceSetupStmt>,
    pub declarations: Vec<CadenceDecl>,
    pub runtime: Option<CadenceRuntimeBlock>,
}

#[derive(Debug, Clone)]
pub enum CadenceSetupStmt {
    Expr { expr: CodeExpr, await_: bool },
}

#[derive(Debug, Clone)]
pub struct CadenceDecl {
    pub name: String,
    pub params: Vec<CadenceParam>,
    pub body: CodeExpr,
}

#[derive(Debug, Clone)]
pub struct CadenceParam {
    pub name: String,
    pub default: Option<CodeExpr>,
}

#[derive(Debug, Clone)]
pub struct CadenceRuntimeBlock {
    pub terminals: Vec<CodeExpr>,
    pub strategy: TerminalStrategy,
}

impl CadenceProgram {
    pub fn render(&self) -> Option<String> {
        let mut sections = Vec::<String>::new();

        for stmt in &self.setup {
            sections.push(stmt.render());
        }

        for decl in &self.declarations {
            sections.push(decl.render());
        }

        if let Some(runtime) = self.runtime.as_ref() {
            let rendered = runtime.render();
            if !rendered.is_empty() {
                sections.push(rendered);
            }
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n"))
        }
    }

    pub fn declaration_code(&self) -> Vec<String> {
        self.declarations.iter().map(CadenceDecl::render).collect()
    }
}

impl CadenceSetupStmt {
    pub fn render(&self) -> String {
        match self {
            CadenceSetupStmt::Expr { expr, await_ } => {
                if *await_ {
                    format!("await {}", expr.render())
                } else {
                    expr.render()
                }
            }
        }
    }
}

impl CadenceDecl {
    pub fn render(&self) -> String {
        let params = self
            .params
            .iter()
            .map(CadenceParam::render)
            .collect::<Vec<_>>()
            .join(", ");
        format!("const {} = ({}) => {}", self.name, params, self.body.render())
    }
}

impl CadenceParam {
    fn render(&self) -> String {
        if let Some(default) = self.default.as_ref() {
            format!("{} = {}", self.name, default.render())
        } else {
            self.name.clone()
        }
    }
}

impl CadenceRuntimeBlock {
    pub fn render(&self) -> String {
        let renderer = self.strategy.as_renderer();
        renderer.render(self.terminals.as_slice())
    }
}

pub fn declaration_from_compiled_trick(compiled: &CompiledTrick) -> CadenceDecl {
    let defaults = trick_param_defaults(&compiled.signature);
    CadenceDecl {
        name: compiled.binding_name.clone(),
        params: compiled
            .signature
            .inputs
            .iter()
            .zip(defaults)
            .map(|(input, default)| CadenceParam {
                name: input.param_name(),
                default,
            })
            .collect(),
        body: compiled.body.clone(),
    }
}
