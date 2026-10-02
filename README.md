# apshare-rs

Comparte internet de **ethernet por WiFi** en Linux con NetworkManager.
Port en Rust de [go-apshare](https://github.com/Omarbautista-dev/go-apshare),
con dashboard [Ratatui](https://ratatui.rs) estilo
[tui.builders/templates → Dashboard](https://tui.builders/templates)
(sidebar + tabla + status bar) + System Monitor + CLI Wizard + Form +
Log Viewer + Data Explorer.

## Requisitos

- Linux con `NetworkManager` activo
- `nmcli` + `iw` (`sudo pacman -S networkmanager iw` en Arch/CachyOS)
- Tarjeta WiFi con modo `AP` (`iw list | grep -A 8 "Supported interface modes"`)
- Ethernet conectado (autodetectado) y `sudo` para crear el hotspot

## Instalación

```bash
cargo install --git https://github.com/Omarbautista-dev/apshare-rs
# o desde código:
git clone https://github.com/Omarbautista-dev/apshare-rs
cd apshare-rs
cargo install --path .
```

El binario se llama `apshare-rs` y convive con la versión Go (`apshare`).

## Uso

```bash
apshare-rs            # dashboard TUI (paneles 1-9, q salir)
apshare-rs check
apshare-rs devices
apshare-rs status
apshare-rs clients
apshare-rs logs 50 hotspot
sudo apshare-rs start --ssid MiLaptop --password clave12345
apshare-rs stop
apshare-rs restart
apshare-rs forget Hotspot
```

Paneles TUI: 1 Estado · 2 Dispositivos · 3 Clientes · 4 Red on/off (wizard,
apagar, reiniciar, olvidar) · 5 Clave/QR · 6 Diagnóstico (rfkill +
masquerade con arreglo) · 7 Logs con filtro · 8 Config · 9 Ayuda.

## Desarrollo

```bash
cargo test
cargo clippy
cargo build --release
```
