# apshare-rs

Comparte el internet que entra por **cable ethernet** creando un punto de
acceso **WiFi** desde tu laptop. Hecho para laptops sin entorno de escritorio
(como CachyOS + Niri), pero funciona en cualquier Linux con NetworkManager.

Incluye dashboard en terminal (Ratatui) con estado, dispositivos, clientes
conectados, asistente de creación, clave/QR, diagnóstico, logs y
configuración, además de comandos directos para scripts.

## Requisitos

- Linux con tarjeta WiFi que soporte modo **AP**
- `NetworkManager`, `nmcli`, `iw`, `git`
- Cable ethernet conectado al momento de compartir
- `sudo` (crear el hotspot requiere permisos de root)
- Rust (solo para compilar al instalar)

## Instalación en un equipo nuevo (paso a paso)

### 1. Dependencias del sistema

**Arch / CachyOS:**
```bash
sudo pacman -S networkmanager iw git base-devel
```

**Ubuntu / Debian / Mint:**
```bash
sudo apt install networkmanager iw git build-essential curl
```

**Fedora:**
```bash
sudo dnf install NetworkManager iw git gcc
```

Activa NetworkManager (en casi todas ya viene activo):
```bash
sudo systemctl enable --now NetworkManager
```

### 2. Rust (omite si ya tienes `cargo`)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
```

### 3. Instalar apshare-rs

```bash
cargo install --git https://github.com/Omarbautista-dev/apshare-rs
```

El binario queda en `~/.cargo/bin/apshare-rs`. Si la terminal dice
`command not found`, agrega esa carpeta a tu PATH (una sola vez):

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc
# En zsh cambia ~/.bashrc por ~/.zshrc
# En fish: fish_add_path ~/.cargo/bin
```

### 4. Verificar que todo funciona

```bash
apshare-rs check
```

Deberías ver algo así:

```
OK: NetworkManager activo, nmcli + iw encontrados.
Ethernet: enp0s20f0u5c2 (connected, Conexión cableada 1)
WiFi: wlan0 (connected)
OK: tu WiFi soporta modo AP.
```

Si dice que tu WiFi **NO** soporta modo AP, tu tarjeta no puede crear
hotspots y el programa no te servirá en ese equipo.

## Uso

### Dashboard (recomendado)

```bash
apshare-rs
```

Teclas: `1-9` cambian de panel, `r` refresca, `q` o `Esc` sale.

1. **Estado**: ethernet, wifi, modo AP, hotspot, clientes y SSID de un vistazo
2. **Dispositivos**: tabla de interfaces
3. **Clientes**: quién está conectado (MAC, señal, velocidad, IP)
4. **Red on/off**: `a` asistente paso a paso, `b` apagar, `c` reiniciar,
   `d` olvidar el perfil
5. **Clave/QR**: QR escaneable generado en el panel (requiere `qrencode`;
   la clave queda oculta, pulsa `v` para verla) · `apshare-rs qr` lo imprime directo
6. **Diagnóstico**: chequeo completo + botón para arreglar el firewall
7. **Logs**: últimos eventos de NetworkManager con filtro de texto
8. **Config**: ver y cambiar el perfil guardado
9. **Ayuda**

### Comandos directos (sin TUI)

```bash
apshare-rs check                          # requisitos
apshare-rs status                         # estado general
apshare-rs devices                         # interfaces
apshare-rs clients                         # clientes conectados ahora
apshare-rs logs 50 hotspot                # últimos 50 logs filtrados
apshare-rs qr                             # QR WiFi en terminal (sin mostrar la clave)
sudo apshare-rs start --ssid MiLaptop --password clave12345
apshare-rs stop                           # apaga y vuelves a tu wifi normal
apshare-rs restart                        # apaga y enciende de nuevo
apshare-rs forget Hotspot                 # borra el perfil guardado
```

> `start` pide `sudo` porque NetworkManager exige root para crear el punto
> de acceso. La contraseña del WiFi debe tener mínimo 8 caracteres.

## Problemas comunes

**Clientes conectan pero sin internet.**
Casi siempre es el firewall bloqueando el NAT. En el panel 6
(Diagnóstico) pulsa `f` para arreglarlo, o manual:
```bash
sudo firewall-cmd --add-masquerade --permanent
sudo firewall-cmd --reload
```

**`ERROR: tu WiFi no soporta modo AP`.**
Tu tarjeta/driver no puede emitir WiFi. Comprueba con:
```bash
iw list | grep -A 8 "Supported interface modes"
# debe aparecer * AP
```

**El hotspot no se crea / error de wpa_supplicant.**
```bash
nmcli radio wifi on && rfkill unblock wifi
```

**`NetworkManager no está activo`.**
```bash
sudo systemctl enable --now NetworkManager
```

**Quiero ver el QR para compartir la clave.**
Instala `qrencode` (`sudo pacman -S qrencode` o `sudo apt install qrencode`) y:
```bash
nmcli device wifi show-password | qrencode -t ANSIUTF8
```

## Desinstalar

```bash
cargo uninstall apshare-rs
# opcional: borrar el perfil guardado
sudo nmcli con delete Hotspot
```

## Desarrollo

```bash
git clone https://github.com/Omarbautista-dev/apshare-rs
cd apshare-rs
cargo test     # tests
cargo clippy   # linter, debe dar 0 warnings
cargo run      # probar el dashboard
```
