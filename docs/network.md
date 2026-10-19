# Firmware network / Cora transport

## Layers

```
firmware/src/network/mod.rs   run_cora(): CoraSession <-> embedded-io-async stream
firmware/src/network/tcp.rs   serve(): embassy-net TCP 5343 accept loop
firmware/src/network/w5500.rs (next) SPI + W5500 + DHCP -> Stack -> serve()
firmware/src/network/cyw43.rs (next) Pico W -> Stack -> serve()
```

`run_cora` is transport-agnostic. It owns the `CoraSession` for one connection,
feeds received bytes, drains `poll_transmit`, pushes input events from an
`embassy_sync` channel, and advances keep-alive/timeout every 100 ms.

`tcp::serve` only needs an `embassy_net::Stack`:

```rust
network::tcp::serve::<4096, 16384>(stack, &core::model::STUDIO, identity, input).await
```

Features:

- `cora-net`: compile the TCP server layer (verified in CI).
- `cora-w5500` / `cora-cyw43`: board backends to be implemented next.

## Board backend checklist

1. Build an async `SpiDevice` (SPI + CS) and W5500 `INT`/`RST` pins.
2. `embassy_net_wiznet::new::<W5500, _, _, _>(mac, &mut STATE, spi, int, rst)`.
3. `embassy_net::new(device, Config::dhcpv4(...), &mut NET_RESOURCES, seed)`.
4. Spawn the W5500 runner and `net_task`.
5. Wait for `stack.wait_config_up()`, then call `tcp::serve`.
6. Add a minimal mDNS responder for `_elg._tcp` (PTR/SRV/TXT/A) on UDP 5353.

CYW43 follows the same flow with `cyw43::Control` / `embassy-net` and a Pico W
board profile.
