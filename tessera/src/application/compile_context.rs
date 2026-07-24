use crate::domain::pattern_ir::PatternProvenance;
use crate::domain::{ContainerId, NodeId};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProvenanceFrame {
    pub node: Option<NodeId>,
    pub container: Option<ContainerId>,
    pub stack_index: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CompileContext {
    pub cycle_index: usize,
    pub provenance_stack: Vec<ProvenanceFrame>,
}

impl CompileContext {
    pub fn new(cycle_index: usize) -> Self {
        Self {
            cycle_index,
            provenance_stack: Vec::new(),
        }
    }

    pub fn current_provenance(&self) -> Option<PatternProvenance> {
        self.provenance_stack.last().map(|frame| PatternProvenance {
            node: frame.node.clone(),
            container: frame.container.clone(),
            stack_index: frame.stack_index,
        })
    }

    pub(crate) fn with_provenance_frame<R>(
        &mut self,
        frame: ProvenanceFrame,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.provenance_stack.push(frame);
        let result = f(self);
        self.provenance_stack.pop();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ContainerId;

    #[test]
    fn provenance_frame_pops_after_error() {
        let mut ctx = CompileContext::default();
        let result = ctx.with_provenance_frame(
            ProvenanceFrame {
                node: None,
                container: Some(ContainerId::new("child")),
                stack_index: None,
            },
            |ctx| {
                assert_eq!(ctx.provenance_stack.len(), 1);
                Err::<(), ()>(())
            },
        );
        assert!(result.is_err());
        assert!(ctx.provenance_stack.is_empty());
    }
}
