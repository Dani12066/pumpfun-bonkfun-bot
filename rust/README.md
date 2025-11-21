# Pump.fun Sniper (Rust)

This crate provides an ultra-low-latency Pump.fun sniper prototype written in
Rust with Tokio. It mirrors the production pipeline described in the repo's
Python bot while focusing on speed, concurrency, and modularity.

## Highlights

- **Async architecture:** LaserStream (gRPC) placeholder and WebSocket fallback
  listeners forward new token creation events into an in-memory pipeline.
- **Developer filters:** Whitelist, blacklist, and per-minute rate limiting are
  applied before any transaction work happens.
- **Cached state:** Background blockhash and balance refreshers avoid hot-path
  RPC calls. A shared cache keeps seen mints and developer rate data.
- **Transaction builder:** Creates ATA + Pump.fun buy instructions with optional
  compute-budget priority fees.
- **Multi-path dispatch:** Races RPC, Jito, and Nozomi HTTP submission futures
  and returns on the first success.
- **Dry-run support:** Skip signing/broadcasting while keeping the entire flow
  intact for safe testing.

## Running

1. Copy `config.example.toml` to your own file and update endpoints, keypair,
   and filters.
2. Point the runner to your config:

```bash
SNIPER_CONFIG=/path/to/your.toml cargo run --release
```

LaserStream parsing is a placeholder; wire it to the Helius proto to enable
real events. The WebSocket listener expects notifications with `mint` and
`developer` fields in the payload.
