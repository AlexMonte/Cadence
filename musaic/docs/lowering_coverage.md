//! Lowering coverage matrix for Musaic Tessera → Cadence handoff.

| PatternNodeIr variant | Musaic lowering | UI placement |
| --- | --- | --- |
| Note / Rest / Scalar | Supported | Atoms in stack |
| TimeScale (slow/fast) | Supported | Trick transforms |
| Reflect | Supported | Rev transform |
| Gain / Attack / Transpose / Degrade | Supported | Trick transforms |
| Merge / Alternate / Layer | Supported | Container kinds |
| Concat | Supported | Arrangement (Phase 9) |
| Control maps | Partial | Port inspector |
| UnsupportedPatternNode | Blocked with diagnostic | — |

All placeable drawer tiles either lower successfully or produce a compile/lowering diagnostic.
