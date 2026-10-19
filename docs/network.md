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

## W5500 API notes (embassy 0.10)

- `Spi::new(SPI0, sck, mosi, miso, DMA_CH0, DMA_CH1, Irqs, config)` needs DMA
  interrupt bindings in `Irqs` (in addition to `USBCTRL_IRQ`).
- Wrap the bus with `embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs, delay)`
  to get `embedded_hal_async::spi::SpiDevice`.
- `static STATE: StaticCell<embassy_net_wiznet::State<2, 2>>`.
- `let (device, runner) = embassy_net_wiznet::new::<W5500, _, _, _>(mac, &mut state, spi_dev, int, rst).await?;`
- `let (stack, net_runner) = embassy_net::new(device, Config::dhcpv4(Default::default()), &mut RESOURCES, seed);`
- Spawn `runner.run()` and a `net_task` polling `stack.wait_config_up()`, then
  call `network::tcp::serve::<4096, 16384>(stack, &model::STUDIO, identity, input)`.
- CYW43 uses the same `Stack` type, so `tcp::serve` and `run_cora` are shared.

## Implemented: W5500-EVB-Pico (`cora-w5500`)

`firmware/src/network/w5500.rs` initializes the W5500-EVB-Pico wiring:

```text
MISO GP16  CSn  GP17
SCK  GP18  MOSI GP19
RSTn GP20  INTn GP21
```

It builds the `embassy-net` stack with DHCP, spawns the WIZnet runner and net
task, waits for the link and calls `network::tcp::serve`. The `studio-cora`
binary uses it:

```bash
cargo build -p productiondeck-firmware --target thumbv6m-none-eabi \
  --features cora-w5500 --bin studio-cora
```

MAC is currently a fixed locally-administered address; derive it from the flash
unique ID when the board profile is finalized.
