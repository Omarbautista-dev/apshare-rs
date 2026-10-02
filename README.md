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

En un equipo nuevo, paso a paso:

```bash
# 1. Sistema (Arch/CachyOS)
sudo pacman -S networkmanager iw git base-devel
# Ubuntu/Debian: sudo apt install networkmanager iw git build-essential curl
sudo systemctl enable --now NetworkManager

# 2. Rust (si no lo tienes)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env

# 3. apshare-rs
cargo install --git https://github.com/Omarbautista-dev/apshare-rs
# el binario queda en ~/.cargo/bin/apshare-rs (debe estar en tu PATH)

# 4. Verificar
apshare-rs check
```

O desde código:

```bash
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
