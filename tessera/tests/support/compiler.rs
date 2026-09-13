use tessera::prelude::*;

pub fn authored(program: &TesseraProgram) -> AuthoredTesseraProgram {
    let placements = program
        .root_nodes
        .keys()
        .enumerate()
        .map(|(index, node)| (node.clone(), RootPlacement::unit(index as i32, 0)))
        .collect();
    AuthoredTesseraProgram {
        root_surface: RootSurface {
            nodes: program.root_nodes.clone(),
            placements,
            explicit_relations: program.relations.clone(),
            bindings: Default::default(),
        },
        containers: program.containers.clone(),
    }
}
