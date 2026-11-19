# 0011 - Command names map to server API functions

Status: accepted; corrected to reflect the owner's clarification. The current command rows exist;
the shared API and registry described below remain planned.

## Decision

`data/commands.json` holds command names, aliases and their mapping to an operation. The `execution`
value names the registered Rust API function: for example, `tp` maps to the teleport function. The
same function executes when called from a GM command, console, management tool or another authorized
caller. Adding an alias to an existing mapping needs a data change, not another implementation.

The command reader currently resolves `execution` to an `Operation` enum in
`game/src/Chat/Commands.rs`; `game/src/Handlers/ChatHandler.rs` handles the operations using the
session. This is not yet the shared API or an extensible function registry. The contract and the
work needed to reach a running world from another process are planned in
`docs/port/14-operator-surfaces.md`.

Lua remains the planned authoring surface for world behaviour and can call server operations. Its
runtime, bindings and any script-defined commands remain to be designed in
`docs/port/11-content-and-behaviour.md`. Lua is not a required intermediate step for a JSON command
mapped to a Rust function.

## Constraints and status

- Command names and aliases are content; the function owns the operation. The same execution must
  not be duplicated for each caller.
- ASURA remains the authority for client-visible behaviour, including existing command arguments,
  refusals, order and frames; the parity audit applies to any change to them.
- The existing travel target alias table and `/maps` text remain in Rust. Their relocation is not
  complete. Client-facing changes require comparison with ASURA and the audit.
- Current account rows provide no command permission model. Authorization, the registry contract,
  supported callers and cross-process access are not implemented by this decision.
- `docs/decisions/0012-chat-text-is-sent.md` records the chat output and its live-client evidence.

## Proof required when implemented

A JSON alias resolves to the same registered function as its primary name; invalid mappings are
reported; and each supported caller obtains the result of the same operation. Existing chat commands
retain their ASURA behaviour and client-visible frames, checked against evidence and a real client.
