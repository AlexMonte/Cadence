//! Lightweight intermediate representation for the final emitted Strudel program.

use tessera::subgraph::{CompiledSubgraph, default_expr_for_input};
use tessera::{Backend, Expr, JsBackend};

#[derive(Debug, Clone)]
/// Final Cadence program split into setup, declarations, and runtime sections.
pub struct CadenceProgram {
    pub setup: Vec<CadenceSetupStmt>,
    pub declarations: Vec<CadenceDecl>,
    pub runtime: Option<CadenceRuntimeBlock>,
}

#[derive(Debug, Clone)]
/// One statement that should run before the runtime graph output.
pub enum CadenceSetupStmt {
    Expr { expr: Expr, await_: bool },
}

#[derive(Debug, Clone)]
/// Rendered trick declaration ready to become a `const name = (...) => ...`.
pub struct CadenceDecl {
    pub name: String,
    pub params: Vec<CadenceParam>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
/// Function parameter metadata for a generated trick declaration.
pub struct CadenceParam {
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone)]
/// Runtime output section returned by the compiled main graph.
pub struct CadenceRuntimeBlock {
    pub rendered_sections: Vec<String>,
    pub voice_count: usize,
}

impl CadenceProgram {
    /// Render the full program into a single source string.
    pub fn render(&self) -> Option<String> {
        let mut sections = Vec::<String>::new();

        for stmt in &self.setup {
            sections.push(stmt.render());
        }

        for decl in &self.declarations {
            sections.push(decl.render());
        }

        if let Some(runtime) = self.runtime.as_ref() {
            for rendered in &runtime.rendered_sections {
                if !rendered.trim().is_empty() {
                    sections.push(rendered.clone());
                }
            }
        }

        if sections.is_empty() {
            None
        } else {
            Some(sections.join("\n"))
        }
    }

    /// Render only the reusable trick declarations.
    pub fn declaration_code(&self) -> Vec<String> {
        self.declarations.iter().map(CadenceDecl::render).collect()
    }
}

impl CadenceSetupStmt {
    /// Render one setup statement, optionally prefixed with `await`.
    pub fn render(&self) -> String {
        match self {
            CadenceSetupStmt::Expr { expr, await_ } => {
                if *await_ {
                    format!("await {}", JsBackend.render(expr))
                } else {
                    JsBackend.render(expr)
                }
            }
        }
    }
}

impl CadenceDecl {
    /// Render a generated trick declaration.
    pub fn render(&self) -> String {
        if self.params.is_empty() {
            // Zero-input tricks are value declarations: const scala = 'C:minor'
            format!(
                "const {} = {}",
                self.name.to_lowercase(),
                JsBackend.render(&self.body)
            )
        } else {
            let params = self
                .params
                .iter()
                .map(CadenceParam::render)
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "const {} = ({}) => {}",
                self.name.to_lowercase(),
                params,
                JsBackend.render(&self.body)
            )
        }
    }
}

impl CadenceParam {
    fn render(&self) -> String {
        if let Some(default) = self.default.as_ref() {
            format!("{} = {}", self.name, JsBackend.render(default))
        } else {
            self.name.clone()
        }
    }
}

/// Convert a compiled Tessera subgraph into a Cadence declaration.
pub fn declaration_from_compiled_subgraph(compiled: &CompiledSubgraph) -> CadenceDecl {
    CadenceDecl {
        name: compiled.binding_name.clone(),
        params: compiled
            .signature
            .inputs
            .iter()
            .map(|input| CadenceParam {
                name: input.param_name(),
                default: default_expr_for_input(input),
            })
            .collect(),
        body: compiled.body.clone(),
    }
}
