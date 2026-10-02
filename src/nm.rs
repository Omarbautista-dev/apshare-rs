//! Backend: llamadas a `nmcli`, `iw`, `ip`, `journalctl`, `rfkill`, `firewall-cmd`.
//! Sin dependencias externas: solo std.

use std::collections::HashMap;
use std::process::Command;

/// Ejecuta un binario y devuelve stdout (recorta). En error devuelve
/// stdout+stderr junto al error.
pub fn run(bin: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| format!("no se pudo ejecutar {bin}: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if out.status.success() {
        return Ok(stdout);
    }
    let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
    let msg = if stdout.is_empty() {
        stderr
    } else {
        format!("{stdout}\n{stderr}").trim().to_string()
    };
    Err(if msg.is_empty() {
        format!("{bin} salió con error")
    } else {
        msg
    })
}

fn have(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Ejecuta nmcli, con sudo si no somos root y la operación lo requiere.
pub fn run_nm(needs_root: bool, args: &[&str]) -> Result<String, String> {
    let is_root = libc_geteuid() == 0;
    if needs_root && !is_root {
        let mut full = vec!["nmcli"];
        full.extend_from_slice(args);
        return run("sudo", &full);
    }
    run("nmcli", args)
}

#[cfg(unix)]
fn libc_geteuid() -> u32 {
    // Evita depender de la crate `libc` solo para esto.
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe { geteuid() }
}

#[cfg(not(unix))]
fn libc_geteuid() -> u32 {
    0
}

/// Una línea de `nmcli device status`.
#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub dtype: String,
    pub state: String,
    pub connection: String,
}

/// Valida requisitos: Linux + nmcli + iw + NetworkManager activo.
pub fn check_system() -> Result<(), String> {
    if !have("nmcli") {
        return Err("no se encontró `nmcli` en PATH. Arch/CachyOS: sudo pacman -S networkmanager".into());
    }
    if !have("iw") {
        return Err("no se encontró `iw` en PATH. Arch/CachyOS: sudo pacman -S iw".into());
    }
    let out = run("systemctl", &["is-active", "NetworkManager"])?;
    if out != "active" {
        return Err(format!(
            "NetworkManager no está activo ({out}). Actívalo con: sudo systemctl enable --now NetworkManager"
        ));
    }
    Ok(())
}

/// Parsea `nmcli -t -f DEVICE,TYPE,STATE,CONNECTION device status`.
pub fn get_devices() -> Result<Vec<Device>, String> {
    let out = run(
        "nmcli",
        &["-t", "-f", "DEVICE,TYPE,STATE,CONNECTION", "device", "status"],
    )?;
    let mut devs = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts: Vec<String> = line.splitn(4, ':').map(|s| s.replace("\\:", ":")).collect();
        while parts.len() < 4 {
            parts.push(String::new());
        }
        devs.push(Device {
            name: parts[0].clone(),
            dtype: parts[1].clone(),
            state: parts[2].clone(),
            connection: parts[3].clone(),
        });
    }
    Ok(devs)
}

pub fn find_ethernet(devs: &[Device]) -> Option<Device> {
    devs.iter()
        .find(|d| {
            d.dtype == "ethernet"
                && d.state.contains("connected")
                && !d.state.contains("externally")
        })
        .or_else(|| {
            devs.iter()
                .find(|d| d.dtype == "ethernet" && d.state.contains("connected"))
        })
        .cloned()
}

pub fn find_wifi(devs: &[Device]) -> Option<Device> {
    devs
        .iter()
        .find(|d| d.dtype == "wifi" && d.state.contains("disconnected"))
        .or_else(|| {
            devs.iter()
                .find(|d| d.dtype == "wifi" && d.state.contains("connected"))
        })
        .or_else(|| devs.iter().find(|d| d.dtype == "wifi"))
        .cloned()
}

/// ¿La tarjeta soporta modo AP? (`iw list` contiene `* AP`).
pub fn supports_ap() -> Result<bool, String> {
    let out = run("iw", &["list"]).map_err(|e| format!("iw list falló: {e}"))?;
    Ok(out.contains("* AP"))
}

#[derive(Debug, Clone)]
pub struct HotspotOpts {
    pub wifi: String,
    pub con_name: String,
    pub ssid: String,
    pub band: String,
    pub channel: String,
    pub password: String,
}

impl HotspotOpts {
    pub fn validate(&self) -> Result<(), String> {
        if self.ssid.is_empty() {
            return Err("SSID vacío".into());
        }
        if self.password.len() < 8 {
            return Err("la contraseña debe tener mínimo 8 caracteres".into());
        }
        if !self.band.is_empty() && !["bg", "a", "6GHz"].contains(&self.band.as_str()) {
            return Err(format!(
                "banda inválida {:?}, usa bg, a o 6GHz",
                self.band
            ));
        }
        if self.wifi.is_empty() {
            return Err("interfaz wifi vacía, no se detectó ninguna".into());
        }
        if self.con_name.is_empty() {
            return Err("nombre de conexión vacío".into());
        }
        Ok(())
    }
}

pub fn start_hotspot(o: &HotspotOpts) -> Result<String, String> {
    o.validate()?;
    let mut args = vec![
        "device",
        "wifi",
        "hotspot",
        "ifname",
        o.wifi.as_str(),
        "con-name",
        o.con_name.as_str(),
        "ssid",
        o.ssid.as_str(),
    ];
    if !o.band.is_empty() {
        args.push("band");
        args.push(o.band.as_str());
    }
    if !o.channel.is_empty() {
        args.push("channel");
        args.push(o.channel.as_str());
    }
    args.push("password");
    args.push(o.password.as_str());
    run_nm(true, &args)
}

pub fn stop_hotspot(con: &str) -> Result<String, String> {
    run_nm(true, &["con", "down", con])
}

pub fn restart_hotspot(con: &str) -> Result<String, String> {
    let down = run_nm(true, &["con", "down", con])?;
    let up = run_nm(true, &["con", "up", con])?;
    Ok(format!("{down}\n{up}"))
}

pub fn delete_connection(con: &str) -> Result<String, String> {
    run_nm(true, &["con", "delete", con])
}

pub fn hotspot_active(con: &str) -> bool {
    match run("nmcli", &["-t", "-f", "NAME,TYPE,STATE", "con", "show", "--active"]) {
        Ok(out) => out.lines().any(|l| l.starts_with(con) || l.contains(con)),
        Err(_) => false,
    }
}

/// Info del perfil. `nmcli con show` usa secciones 802-11-wireless.*, no wifi.*.
pub fn hotspot_info(con: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let out = match run(
        "nmcli",
        &[
            "-t",
            "-f",
            "802-11-wireless.ssid,802-11-wireless.band,802-11-wireless.channel,\
             ipv4.method,ipv4.addresses,802-11-wireless-security.key-mgmt,IP4.ADDRESS",
            "con",
            "show",
            con,
        ],
    ) {
        Ok(o) => o,
        Err(_) => return m,
    };
    for line in out.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let key = match k.trim() {
                "802-11-wireless.ssid" => "ssid",
                "802-11-wireless.band" => "band",
                "802-11-wireless.channel" => "channel",
                "ipv4.method" => "method",
                "ipv4.addresses" => "addresses",
                "802-11-wireless-security.key-mgmt" => "security",
                "IP4.ADDRESS[1]" | "IP4.ADDRESS" => "ip",
                other => other,
            };
            // Solo la primera IP activa.
            if key == "ip" && m.contains_key("ip") {
                continue;
            }
            m.insert(key.to_string(), v.trim().to_string());
        }
    }
    m
}

/// Un cliente conectado al AP (`iw dev <wifi> station dump`).
#[derive(Debug, Clone, Default)]
pub struct Station {
    pub mac: String,
    pub signal: String,
    pub rx: String,
    pub tx: String,
    pub time: String,
}

pub fn get_stations(wifi: &str) -> Vec<Station> {
    if wifi.is_empty() {
        return Vec::new();
    }
    let out = match run("iw", &["dev", wifi, "station", "dump"]) {
        Ok(o) if !o.is_empty() => o,
        _ => return Vec::new(),
    };
    let mut st = Vec::new();
    for line in out.lines() {
        let t = line.trim();
        if let Some(mac) = t.strip_prefix("Station ") {
            st.push(Station {
                mac: mac.split_whitespace().next().unwrap_or("").to_string(),
                ..Default::default()
            });
        } else if let Some(cur) = st.last_mut() {
            if let Some(v) = t.strip_prefix("signal:") {
                cur.signal = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("rx bitrate:") {
                cur.rx = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("tx bitrate:") {
                cur.tx = v.trim().to_string();
            } else if let Some(v) = t.strip_prefix("connected time:") {
                cur.time = v.trim().to_string();
            }
        }
    }
    st
}

/// Vecinos ARP/ND (IPs de clientes) en la interfaz.
pub fn get_neighbours(wifi: &str) -> Vec<String> {
    if wifi.is_empty() {
        return Vec::new();
    }
    match run("ip", &["-brief", "neigh", "show", "dev", wifi]) {
        Ok(o) => o.lines().map(|l| l.to_string()).filter(|l| !l.trim().is_empty()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn show_password(wifi: &str) -> String {
    if !wifi.is_empty()
        && let Ok(o) = run("nmcli", &["device", "wifi", "show-password", "ifname", wifi])
        && !o.is_empty()
    {
        return o;
    }
    run("nmcli", &["device", "wifi", "show-password"]).unwrap_or_default()
}

pub fn has_qr() -> bool {
    have("qrencode")
}

/// Últimas N líneas de NetworkManager, con filtro opcional (minúsculas).
pub fn get_logs(n: &str, filter: &str) -> String {
    let out = match run(
        "journalctl",
        &["-u", "NetworkManager", "--no-pager", "-n", n, "--no-hostname"],
    ) {
        Ok(o) => o,
        Err(e) => return format!("No se pudieron leer logs (¿journalctl disponible?): {e}"),
    };
    if filter.is_empty() {
        return out;
    }
    let fl = filter.to_lowercase();
    let keep: Vec<&str> = out
        .lines()
        .filter(|l| l.to_lowercase().contains(&fl))
        .collect();
    if keep.is_empty() {
        return format!("(sin coincidencias para '{filter}')");
    }
    keep.join("\n")
}

pub fn get_rfkill() -> String {
    run("rfkill", &["list", "wifi"]).unwrap_or_default()
}

/// "yes" / "no" / "" (sin firewalld).
pub fn masquerade_status() -> String {
    if !have("firewall-cmd") {
        return String::new();
    }
    run("firewall-cmd", &["--query-masquerade"])
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub fn fix_masquerade() -> Result<String, String> {
    if !have("firewall-cmd") {
        return Err("firewall-cmd no disponible".into());
    }
    let a = run("sudo", &["firewall-cmd", "--add-masquerade", "--permanent"])?;
    let b = run("sudo", &["firewall-cmd", "--reload"])?;
    Ok(format!("{a}\n{b}"))
}

/// Texto de estado para `apshare-rs status` (modo script).
pub fn status_text(con: &str) -> String {
    let mut b = String::new();
    b.push_str("== Dispositivos ==\n");
    if let Ok(devs) = get_devices() {
        for d in &devs {
            let conn = if d.connection.is_empty() { "--" } else { &d.connection };
            b.push_str(&format!("{:15} {:10} {:25} {conn}\n", d.name, d.dtype, d.state));
        }
    }
    b.push_str(&format!("\n== Hotspot '{con}' ==\n"));
    if hotspot_active(con) {
        b.push_str("activo\n");
        if let Ok(o) = run(
            "nmcli",
            &[
                "-f",
                "ipv4.method,ipv4.addresses,802-11-wireless.ssid,802-11-wireless.band",
                "con",
                "show",
                con,
            ],
        ) {
            b.push_str(&o);
            b.push('\n');
        }
        let pw = show_password("");
        if !pw.is_empty() {
            b.push('\n');
            b.push_str(&pw);
            b.push('\n');
        }
    } else {
        b.push_str("inactivo (inicia con: apshare-rs start)\n");
    }
    if have("firewall-cmd") {
        let m = masquerade_status();
        if !m.is_empty() {
            b.push_str(&format!("\nmasquerade: {m}\n"));
            if m.contains("no") {
                b.push_str("Si los clientes conectan pero sin internet: sudo firewall-cmd --add-masquerade --permanent && sudo firewall-cmd --reload\n");
            }
        }
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_ok() {
        let o = HotspotOpts {
            wifi: "wlan0".into(),
            con_name: "Hotspot".into(),
            ssid: "MiLaptop".into(),
            band: "bg".into(),
            channel: "".into(),
            password: "clave12345".into(),
        };
        assert!(o.validate().is_ok());
    }

    #[test]
    fn validate_short_password() {
        let o = HotspotOpts {
            wifi: "wlan0".into(),
            con_name: "Hotspot".into(),
            ssid: "MiLaptop".into(),
            band: "bg".into(),
            channel: "".into(),
            password: "corta".into(),
        };
        assert!(o.validate().is_err());
    }

    #[test]
    fn validate_bad_band() {
        let o = HotspotOpts {
            wifi: "wlan0".into(),
            con_name: "Hotspot".into(),
            ssid: "MiLaptop".into(),
            band: "xx".into(),
            channel: "".into(),
            password: "clave12345".into(),
        };
        assert!(o.validate().is_err());
    }
}
