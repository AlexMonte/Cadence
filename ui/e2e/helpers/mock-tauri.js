(() => {
  const GRID_COLS = 9;
  const GRID_ROWS = 9;
  let edgeSeq = 1;

  const patternSchema = () => ({
    kind: "custom",
    port_type: "pattern",
    value_kind: "text",
    can_inline: false,
    inline_mode: "raw",
    default: null,
    min: null,
    max: null,
  });

  const numberSchema = () => ({
    kind: "number",
    default: 1,
    min: null,
    max: null,
    can_inline: true,
  });

  const textSchema = (defaultValue = "") => ({
    kind: "text",
    default: defaultValue,
    can_inline: true,
  });

  const boolSchema = (defaultValue = false) => ({
    kind: "bool",
    default: defaultValue,
    can_inline: true,
  });

  const enumSchema = (options, defaultValue) => ({
    kind: "enum",
    options,
    default: defaultValue,
    can_inline: true,
  });

  const jsonSchema = () => ({
    kind: "custom",
    port_type: "any",
    value_kind: "json",
    default: null,
    can_inline: true,
    inline_mode: "literal",
    min: null,
    max: null,
  });

  const builtins = [
    {
      id: "strudel.sound",
      label: "s",
      category: "generator",
      params: [
        { id: "pattern", label: "pattern", side: "west", schema: patternSchema(), required: false },
        { id: "value", label: "value", side: "south", schema: textSchema("bd"), required: false },
      ],
      output_type: "pattern",
      output_side: "east",
      description: "Create a sample pattern via s().",
    },
    {
      id: "strudel.fast",
      label: "fast",
      category: "transform",
      params: [
        { id: "pattern", label: "pattern", side: "west", schema: patternSchema(), required: true },
        { id: "factor", label: "factor", side: "south", schema: numberSchema(), required: false },
      ],
      output_type: "pattern",
      output_side: "east",
      description: "Speed up a pattern by a factor.",
    },
    {
      id: "strudel.output",
      label: "play",
      category: "output",
      params: [{ id: "pattern", label: "pattern", side: "west", schema: patternSchema(), required: true }],
      output_type: null,
      output_side: null,
      description: "Terminal output node.",
    },
  ];

  const trickInputPiece = (slot) => ({
    id: `cadence.trick_input_${slot}`,
    label: `arg${slot}`,
    category: "trick",
    params: [
      { id: "label", label: "label", side: "south", schema: textSchema(`input ${slot}`), required: false },
      {
        id: "port_type",
        label: "type",
        side: "south",
        schema: enumSchema(["pattern", "number", "text", "rhythm", "bool", "trigger", "signal"], "pattern"),
        required: false,
      },
      { id: "required", label: "required", side: "south", schema: boolSchema(true), required: false },
      { id: "is_receiver", label: "receiver", side: "south", schema: boolSchema(false), required: false },
      { id: "default_value", label: "default", side: "south", schema: jsonSchema(), required: false },
    ],
    output_type: "any",
    output_side: "east",
    description: "Trick boundary input.",
  });

  const trickOutputPiece = () => ({
    id: "cadence.trick_output",
    label: "return",
    category: "output",
    params: [{ id: "pattern", label: "pattern", side: "west", schema: patternSchema(), required: true }],
    output_type: null,
    output_side: null,
    description: "Trick boundary output.",
  });

  const project = {
    name: "Untitled",
    schema_version: 3,
    dirty: false,
    path: null,
  };

  const initStage = {
    cps_expr: null,
    sample_loads: [],
    tricks: [],
  };
  const OPEN_PATH = "/mock/projects/loaded.json";

  const runtimeGraph = {
    name: "runtime",
    nodes: [],
    edges: [],
    cols: GRID_COLS,
    rows: GRID_ROWS,
  };

  const deepClone = (value) => JSON.parse(JSON.stringify(value));

  const equalsPos = (a, b) => a && b && a.col === b.col && a.row === b.row;
  const sideFromToNode = (toNode, fromNode) => {
    if (fromNode.col === toNode.col + 1 && fromNode.row === toNode.row) return "east";
    if (fromNode.col === toNode.col - 1 && fromNode.row === toNode.row) return "west";
    if (fromNode.col === toNode.col && fromNode.row === toNode.row - 1) return "north";
    if (fromNode.col === toNode.col && fromNode.row === toNode.row + 1) return "south";
    return null;
  };
  const sidesFace = (left, right) =>
    (left === "east" && right === "west")
    || (left === "west" && right === "east")
    || (left === "north" && right === "south")
    || (left === "south" && right === "north");

  const graphForTarget = (target) => {
    if (target?.kind === "trick") {
      return initStage.tricks.find((trick) => trick.id === target.trick_id)?.graph ?? null;
    }
    return runtimeGraph;
  };

  const nodeAt = (graph, pos) => graph.nodes.find((entry) => equalsPos(entry.position, pos)) ?? null;
  const nodeIndexAt = (graph, pos) => graph.nodes.findIndex((entry) => equalsPos(entry.position, pos));
  const edgeAtParam = (graph, toNode, toParam) =>
    graph.edges.find((edge) => equalsPos(edge.to_node, toNode) && edge.to_param === toParam) ?? null;

  const currentCatalog = (target) => {
    if (target?.kind === "trick") {
      return [...builtins, trickInputPiece(1), trickInputPiece(2), trickInputPiece(3), trickOutputPiece()];
    }
    return [
      ...builtins,
      ...initStage.tricks.map((trick) => {
        const inputs = trickSignature(trick).inputs;
        const params = inputs.map((input, index) => ({
          id: `arg${input.slot}`,
          label: input.label,
          side: input.is_receiver ? "west" : ["west", "south", "north", "east"][index] ?? "south",
          schema: input.port_type === "number" ? numberSchema() : textSchema(""),
          required: input.required,
        }));
        if (inputs.some((input) => input.port_type === "pattern")) {
          for (const param of params) {
            if (param.id === `arg${inputs.find((input) => input.is_receiver)?.slot}`) {
              param.schema = patternSchema();
            }
          }
        }
        return {
          id: `cadence.trick.${trick.id}`,
          label: trick.name,
          category: "trick",
          params,
          output_type: "pattern",
          output_side: "east",
          description: "Generated trick tile.",
        };
      }),
    ];
  };

  const pieceById = (target) => new Map(currentCatalog(target).map((item) => [item.id, item]));

  const schemaPortType = (schema) => {
    if (!schema) return null;
    if (schema.kind === "custom") return String(schema.port_type ?? "");
    if (schema.kind === "enum") return "text";
    return String(schema.kind ?? "");
  };

  const schemaAccepts = (schema, sourceType) => {
    if (!schema || !sourceType) return false;
    if (sourceType === "any") return true;
    if (schema.kind === "bool") return sourceType === "bool" || sourceType === "number";
    const expected = schemaPortType(schema);
    return expected === sourceType || expected === "any";
  };

  const pickTargetParam = (graph, target, fromPos, toPos) => {
    const fromNode = nodeAt(graph, fromPos);
    const toNode = nodeAt(graph, toPos);
    if (!fromNode) return { to_param: null, reason: "unknown_source_node", detail: "missing source node" };
    if (!toNode) return { to_param: null, reason: "unknown_target_node", detail: "missing target node" };

    const catalogById = pieceById(target);
    const fromDef = catalogById.get(fromNode.node.piece_id);
    const toDef = catalogById.get(toNode.node.piece_id);
    if (!fromDef) return { to_param: null, reason: "unknown_source_piece", detail: "unknown source piece" };
    if (!toDef) return { to_param: null, reason: "unknown_target_piece", detail: "unknown target piece" };
    if (!fromDef.output_type) return { to_param: null, reason: "output_from_terminal", detail: "source cannot output" };

    const expectedSide = sideFromToNode(toPos, fromPos);
    if (!expectedSide) return { to_param: null, reason: "not_adjacent", detail: "nodes must be adjacent" };
    const sourceSide = String(fromDef.output_side ?? "east");
    if (!sidesFace(sourceSide, expectedSide)) {
      return { to_param: null, reason: "side_mismatch", detail: "source side must face target side" };
    }

    const sideCandidates = (toDef.params ?? []).filter((param) => String(param.side) === expectedSide);
    if (sideCandidates.length === 0) return { to_param: null, reason: "no_param_on_target_side", detail: "no input on side" };

    const openCandidates = sideCandidates.filter((param) => !edgeAtParam(graph, toPos, param.id));
    if (openCandidates.length === 0) {
      return { to_param: null, reason: "target_param_occupied", detail: "target already connected" };
    }

    const compatible = openCandidates.find((param) => schemaAccepts(param.schema, fromDef.output_type));
    if (!compatible) return { to_param: null, reason: "type_mismatch", detail: "type mismatch" };

    return { to_param: compatible.id, reason: null, detail: null };
  };

  const applyOps = (graph, target, ops) => {
    for (const op of ops) {
      if (!op || typeof op !== "object") continue;
      switch (op.op) {
        case "node_place": {
          const position = op.position;
          if (!position || nodeAt(graph, position)) break;
          graph.nodes.push({
            position: { col: Number(position.col), row: Number(position.row) },
            node: {
              piece_id: String(op.piece_id),
              inline_params: op.inline_params && typeof op.inline_params === "object" ? op.inline_params : {},
              input_sides: {},
              output_side: null,
            },
          });
          break;
        }
        case "node_move": {
          const fromIndex = nodeIndexAt(graph, op.from);
          if (fromIndex < 0 || nodeAt(graph, op.to)) break;
          graph.nodes[fromIndex].position = { col: Number(op.to.col), row: Number(op.to.row) };
          for (const edge of graph.edges) {
            if (equalsPos(edge.from, op.from)) edge.from = { col: Number(op.to.col), row: Number(op.to.row) };
            if (equalsPos(edge.to_node, op.from)) edge.to_node = { col: Number(op.to.col), row: Number(op.to.row) };
          }
          break;
        }
        case "node_swap": {
          const aIndex = nodeIndexAt(graph, op.a);
          const bIndex = nodeIndexAt(graph, op.b);
          if (aIndex < 0 || bIndex < 0) break;
          const aPos = graph.nodes[aIndex].position;
          const bPos = graph.nodes[bIndex].position;
          graph.nodes[aIndex].position = bPos;
          graph.nodes[bIndex].position = aPos;
          for (const edge of graph.edges) {
            if (equalsPos(edge.from, op.a)) edge.from = { col: Number(op.b.col), row: Number(op.b.row) };
            else if (equalsPos(edge.from, op.b)) edge.from = { col: Number(op.a.col), row: Number(op.a.row) };
            if (equalsPos(edge.to_node, op.a)) edge.to_node = { col: Number(op.b.col), row: Number(op.b.row) };
            else if (equalsPos(edge.to_node, op.b)) edge.to_node = { col: Number(op.a.col), row: Number(op.a.row) };
          }
          break;
        }
        case "node_remove": {
          const idx = nodeIndexAt(graph, op.position);
          if (idx < 0) break;
          const pos = graph.nodes[idx].position;
          graph.nodes.splice(idx, 1);
          graph.edges = graph.edges.filter((edge) => !equalsPos(edge.from, pos) && !equalsPos(edge.to_node, pos));
          break;
        }
        case "edge_connect": {
          const probe = pickTargetParam(graph, target, op.from, op.to_node);
          if (!probe.to_param) break;
          graph.edges.push({
            id: `edge-${edgeSeq++}`,
            from: { col: Number(op.from.col), row: Number(op.from.row) },
            to_node: { col: Number(op.to_node.col), row: Number(op.to_node.row) },
            to_param: String(op.to_param ?? probe.to_param),
          });
          break;
        }
        case "edge_disconnect": {
          graph.edges = graph.edges.filter((edge) => edge.id !== op.edge_id);
          break;
        }
        case "param_set_inline": {
          const entry = nodeAt(graph, op.position);
          if (!entry) break;
          entry.node.inline_params = entry.node.inline_params ?? {};
          entry.node.inline_params[op.param_id] = op.value;
          break;
        }
        case "param_clear_inline": {
          const entry = nodeAt(graph, op.position);
          if (!entry?.node?.inline_params) break;
          delete entry.node.inline_params[op.param_id];
          break;
        }
        case "param_set_side": {
          const entry = nodeAt(graph, op.position);
          if (!entry) break;
          entry.node.input_sides = entry.node.input_sides ?? {};
          entry.node.input_sides[op.param_id] = op.side;
          break;
        }
        case "param_clear_side": {
          const entry = nodeAt(graph, op.position);
          if (!entry?.node?.input_sides) break;
          delete entry.node.input_sides[op.param_id];
          break;
        }
        case "output_set_side": {
          const entry = nodeAt(graph, op.position);
          if (!entry) break;
          entry.node.output_side = op.side;
          break;
        }
        case "output_clear_side": {
          const entry = nodeAt(graph, op.position);
          if (!entry) break;
          entry.node.output_side = null;
          break;
        }
        default:
          break;
      }
    }
  };

  const trickSignature = (trick) => {
    const inputs = trick.graph.nodes
      .filter((entry) => String(entry.node.piece_id).startsWith("cadence.trick_input_"))
      .map((entry) => {
        const slot = Number(String(entry.node.piece_id).split("_").pop());
        return {
          slot,
          label: String(entry.node.inline_params?.label ?? `input ${slot}`),
          port_type: String(entry.node.inline_params?.port_type ?? "pattern"),
          required: Boolean(entry.node.inline_params?.required ?? true),
          is_receiver: Boolean(entry.node.inline_params?.is_receiver ?? false),
        };
      })
      .sort((left, right) => left.slot - right.slot);
    return { inputs };
  };

  const projectCode = () => {
    const lines = [];
    if (initStage.cps_expr) {
      lines.push(`setCps(${initStage.cps_expr})`);
    }
    for (const load of initStage.sample_loads) {
      lines.push(`await samples(${JSON.stringify(load.aliases)}, ${JSON.stringify(load.source)})`);
    }
    for (const trick of initStage.tricks) {
      const binding = trick.name.replace(/[^a-zA-Z0-9_$]+/g, "_") || `trick_${trick.id}`;
      lines.push(`const ${binding} = () => s("bd")`);
    }
    if (runtimeGraph.nodes.length > 0) {
      lines.push("s(\"bd\")");
    }
    return lines.join("\n");
  };

  const initSnapshot = () => ({
    cps_expr: initStage.cps_expr,
    sample_loads: deepClone(initStage.sample_loads),
    tricks: initStage.tricks.map((trick) => ({
      id: trick.id,
      name: trick.name,
      node_count: trick.graph.nodes.length,
      edge_count: trick.graph.edges.length,
    })),
  });

  const projectSnapshot = () => ({
    name: project.name,
    schema_version: project.schema_version,
    node_count: runtimeGraph.nodes.length,
    edge_count: runtimeGraph.edges.length,
    dirty: project.dirty,
    path: project.path,
  });

  const invoke = async (command, payload) => {
    const args = payload?.args ?? {};
    const target = args.target ?? { kind: "runtime" };
    const graph = graphForTarget(target);

    switch (command) {
      case "project_snapshot":
        return deepClone(projectSnapshot());
      case "project_new":
        project.name = String(args.name ?? "Untitled");
        project.dirty = false;
        runtimeGraph.nodes = [];
        runtimeGraph.edges = [];
        initStage.cps_expr = null;
        initStage.sample_loads = [];
        initStage.tricks = [];
        return deepClone(projectSnapshot());
      case "project_pick_open_path":
        return OPEN_PATH;
      case "project_open_path":
      case "project_open": {
        project.name = "Loaded";
        project.dirty = false;
        runtimeGraph.nodes = [
          {
            position: { col: 0, row: 0 },
            node: {
              piece_id: "cadence.trick.loaded_melodia",
              inline_params: {},
              input_sides: {},
              output_side: null,
            },
          },
          {
            position: { col: 1, row: 0 },
            node: {
              piece_id: "strudel.output",
              inline_params: {},
              input_sides: {},
              output_side: null,
            },
          },
        ];
        runtimeGraph.edges = [
          {
            id: `edge-${edgeSeq++}`,
            from: { col: 0, row: 0 },
            to_node: { col: 1, row: 0 },
            to_param: "pattern",
          },
        ];
        initStage.cps_expr = "113/60/4";
        initStage.sample_loads = [];
        initStage.tricks = [
          {
            id: "loaded_melodia",
            name: "loaded melodia",
            graph: {
              name: "loaded melodia",
              nodes: [
                {
                  position: { col: 0, row: 0 },
                  node: {
                    piece_id: "cadence.trick_input_1",
                    inline_params: {
                      label: "pattern",
                      port_type: "pattern",
                      required: true,
                      is_receiver: true,
                    },
                    input_sides: {},
                    output_side: null,
                  },
                },
                {
                  position: { col: 1, row: 0 },
                  node: {
                    piece_id: "cadence.trick_output",
                    inline_params: {},
                    input_sides: {},
                    output_side: null,
                  },
                },
              ],
              edges: [
                {
                  id: `edge-${edgeSeq++}`,
                  from: { col: 0, row: 0 },
                  to_node: { col: 1, row: 0 },
                  to_param: "pattern",
                },
              ],
              cols: GRID_COLS,
              rows: GRID_ROWS,
            },
          },
        ];
        return deepClone(projectSnapshot());
      }
      case "project_init_snapshot":
        return deepClone(initSnapshot());
      case "project_init_apply": {
        for (const op of Array.isArray(args.ops) ? args.ops : []) {
          switch (op.op) {
            case "set_cps":
              initStage.cps_expr = typeof op.expr === "string" && op.expr.trim() ? op.expr.trim() : null;
              break;
            case "sample_load_upsert": {
              const next = {
                id: String(op.id),
                source: String(op.source),
                aliases: op.aliases && typeof op.aliases === "object" ? deepClone(op.aliases) : {},
              };
              const index = initStage.sample_loads.findIndex((entry) => entry.id === next.id);
              if (index >= 0) initStage.sample_loads[index] = next;
              else initStage.sample_loads.push(next);
              break;
            }
            case "sample_load_remove":
              initStage.sample_loads = initStage.sample_loads.filter((entry) => entry.id !== op.id);
              break;
            case "trick_create":
              if (!initStage.tricks.some((entry) => entry.id === op.id)) {
                initStage.tricks.push({
                  id: String(op.id),
                  name: String(op.name),
                  graph: {
                    name: String(op.name),
                    nodes: [],
                    edges: [],
                    cols: GRID_COLS,
                    rows: GRID_ROWS,
                  },
                });
              }
              break;
            case "trick_rename": {
              const trick = initStage.tricks.find((entry) => entry.id === op.id);
              if (trick) {
                trick.name = String(op.name);
                trick.graph.name = String(op.name);
              }
              break;
            }
            case "trick_delete":
              initStage.tricks = initStage.tricks.filter((entry) => entry.id !== op.id);
              break;
            default:
              break;
          }
        }
        project.dirty = true;
        return deepClone(initSnapshot());
      }
      case "graph_piece_catalog":
        return deepClone(currentCatalog(target));
      case "graph_snapshot":
        return deepClone(graph ?? runtimeGraph);
      case "graph_compile_preview":
        return {
          can_compile: true,
          code: graph?.nodes?.length ? "s(\"bd\")" : null,
          exprs: [],
          diagnostics: [],
          eval_order: [],
          terminals: [],
        };
      case "project_compile_preview":
        return {
          can_render: true,
          can_play: runtimeGraph.nodes.some((entry) => entry.node.piece_id === "strudel.output"),
          code: projectCode(),
          diagnostics: [],
        };
      case "graph_pick_target_param":
        return pickTargetParam(graph ?? runtimeGraph, target, args.from, args.to_node);
      case "graph_apply_ops":
        applyOps(graph ?? runtimeGraph, target, Array.isArray(args.ops) ? args.ops : []);
        project.dirty = true;
        return {
          graph: deepClone(graph ?? runtimeGraph),
          semantic: {
            diagnostics: [],
            eval_order: [],
            terminals: [],
          },
          preview_code: (graph ?? runtimeGraph).nodes.length ? "s(\"bd\")" : null,
          removed_edges: [],
        };
      case "ui_set_mini_console_visible":
      case "runtime_stop":
      case "history_undo":
      case "history_redo":
        return null;
      case "runtime_commit":
        return {
          success: true,
          playing: true,
          code: projectCode(),
          cps_expr: initStage.cps_expr,
          sample_loads: deepClone(initStage.sample_loads),
          declaration_code: initStage.tricks.map((trick) => `const ${trick.name.replace(/[^a-zA-Z0-9_$]+/g, "_")} = () => s("bd")`),
          runtime_code: runtimeGraph.nodes.length ? "s(\"bd\")" : null,
          diagnostics: [],
          error: null,
        };
      default:
        return null;
    }
  };

  window.__CADENCE_TEST__ = true;
  window.__TAURI__ = { core: { invoke } };
})();
