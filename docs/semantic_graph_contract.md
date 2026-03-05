# Semantic Graph Contract (Refactor)

## Modes
- `code_authority`: source text edits are canonical; graph is projected from IR.
- `graph_authority`: graph operations are canonical; source is rendered from IR + source-preserving patching.

## Pipeline
1. `raw source`
2. shorthand desugar (`$:` -> `stack(...)`) with origin mapping
3. JS AST parse (SWC)
4. symbol extraction + semantic typing
5. typed IR (`IrStatement`)
6. graph projection (`top` + `inner` layers)

## Type Lattice
- `pattern`
- `pattern_fn`
- `number`
- `string`
- `boolean`
- `event_like`
- `collection`
- `symbol_ref`
- `unknown`

## Invariants
- Unknown is explicit and never coerced to pattern.
- Opaque valid JS statements remain byte-preserved.
- Dependency edges are `symbol_ref` typed.
- UI color decisions are derived from semantic type keys only.

## Commands
- `graph_scope_snapshot`
- `graph_scope_set_mode`
- `graph_scope_preview_source`
- `graph_scope_apply_source`
- `graph_scope_preview_ops`
- `graph_scope_apply_ops`

## Graph Ops Payload
- `statement_create { after_statement_id?: string, raw: string }`
- `statement_update_raw { statement_id: string, raw: string }`
- `statement_delete { statement_id: string }`
- `statement_move { statement_id: string, after_statement_id?: string }`

## Mode Gating
- `code_authority`: `graph_scope_apply_source` allowed, `graph_scope_apply_ops` rejected with `mode_mismatch`.
- `graph_authority`: `graph_scope_apply_ops` allowed, `graph_scope_apply_source` rejected with `mode_mismatch`.
- `graph_scope_preview_source` and `graph_scope_preview_ops` are allowed in both modes.
