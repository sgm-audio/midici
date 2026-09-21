# midici-responder

Reserved for the Phase-8 ergonomic façade (timers, ring pairings, event pump) on top of the sans-io engines.

## Today

The complete responder already lives in **`midici_pe::ResponderEngine`** — that
is the type transports drive. This crate currently exposes only package
identity (`VERSION`, `CRATE_NAME`) and will gain the façade (poll-loop helper,
bounded ring wiring, logging hooks) in a later phase.

- Use `midici_pe::ResponderEngine` for engine composition.
- Use `midici-transport-alsa` (virtual endpoint + 10 ms control loop) or
  `midici-transport-clap` (RT ring) for ready-made glue.
