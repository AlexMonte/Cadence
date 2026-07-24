//! Lowering coverage matrix for Musaic Tessera → Cadence handoff.

Homomorphism rule: each `PatternNodeIr` arm maps 1:1 to `ScoreKind` and/or
`ControlScoreKind` (see [`cadence/HOST_API.md`](../../cadence/HOST_API.md)).
Cadence owns score/control semantics; Musaic tests assert “maps X → X”.

| PatternNodeIr | Event path (`Score`) | Control path (`ControlScore`) | UI placement |
| --- | --- | --- | --- |
| Note / Rest | Supported (`Mosaic` / empty) | — | Atoms in stack |
| Scalar stream | — | Gate `Track` | Scalar atoms |
| Control stream | — | `Track` | Port inspector / control atoms |
| Merge | `Merge` | `Merge` | Container kinds |
| Concat | `Concat` | `Concat` | Arrangement |
| CycleRoute | `CycleRoute` | `CycleRoute` | Alternate |
| CycleSlots | `CycleSlots` | `CycleSlots` | Slot containers |
| TimeScale (slow/fast) | `TimeScale` | `TimeScale` | Trick transforms |
| Shift | `Shift` | `Shift` | Offset transforms |
| ReflectCycle | `ReflectCycle` | `ReflectCycle` | Rev transform |
| PriorityMerge | `PriorityMerge` | `PriorityMerge` | Priority layer |
| WeightedChoice | `WeightedChoice` | `WeightedChoice` | Weighted choice |
| MaskClip | `MaskClip` | `MaskClip` | Gate mask; never silent merge |
| SpaceShift / SpaceScale / SpaceReflect | Supported | Pass-through (no spatial controls) | Space tricks |
| Degrade | `Degrade` | Unsupported diagnostic (Score-only) | Degrade field / trick |
| Deduplicate | `Deduplicate` | Unsupported diagnostic (Score-only) | Dedup transform |
| Event field Gain / Attack / … | Control tiles → `with_controls` | — | Trick transforms |
| UnsupportedPatternNode | Blocked with diagnostic | Blocked with diagnostic | — |

All placeable drawer tiles either lower successfully or produce a compile/lowering diagnostic.
