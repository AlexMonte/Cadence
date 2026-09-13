# Musaic asset-loader snapshot

This directory preserves the local `load_up` 0.3.0 source, copied on 2026-09-05,
under its original MIT OR Apache-2.0 licenses. Musaic previously depended on a
mutable sibling folder, which had upgraded to Bevy 0.19.1 while Musaic remains on
Bevy 0.18.1. Its macros, resources, and plugins consequently used incompatible ECS
types.

The snapshot pins its core Bevy dependencies to 0.18.1 and removes the 0.19-only
`Command::Out` associated type from four command implementations. Musaic uses only the default
core feature set. The optional upstream integrations are retained but are not
part of this compatibility target. Keep this adapter snapshot until a published,
versioned compatible dependency or a deliberate whole-application upgrade replaces
it. Do not edit the user's separate `workspace/load_up` project to build Musaic.
