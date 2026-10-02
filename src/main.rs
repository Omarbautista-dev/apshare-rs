//! apshare-rs: comparte internet de ethernet por WiFi (NetworkManager).
//! Backend en `nm`, dashboard Ratatui en `tui`.

mod nm;
mod tui;

use clap::{Parser, Subcommand};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "apshare-rs", version, about = "Comparte ethernet por WiFi con NetworkManager")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Dashboard TUI (por defecto)
    Tui,
    /// Alias de tui
    Ui,
    /// Crear/activar hotspot
    Start {
        #[arg(long, default_value = "MiLaptop")]
        ssid: String,
        #[arg(long, default_value = "")]
        password: String,
        #[arg(long, default_value = "")]
        wifi: String,
        #[arg(long, default_value = "Hotspot")]
        con: String,
        #[arg(long, default_value = "bg")]
        band: String,
        #[arg(long, default_value = "")]
        channel: String,
    },
    /// Apagar hotspot
    Stop {
        #[arg(default_value = "Hotspot")]
        con: String,
    },
    /// Reiniciar hotspot (down+up)
    Restart {
        #[arg(default_value = "Hotspot")]
        con: String,
    },
    /// Eliminar perfil hotspot
    Forget {
        #[arg(default_value = "Hotspot")]
        con: String,
    },
    /// Ver dispositivos y estado
    Status {
        #[arg(default_value = "Hotspot")]
        con: String,
    },
    /// Listar interfaces
    Devices,
    /// Listar clientes conectados al AP
    Clients,
    /// Logs de NetworkManager: apshare-rs logs [n] [filtro]
    Logs {
        #[arg(default_value = "50")]
        n: String,
        #[arg(default_value = "")]
        filter: String,
    },
    /// Verificar requisitos
    Check,
}

fn cmd_check() -> i32 {
    if let Err(e) = nm::check_system() {
        eprintln!("ERROR: {e}");
        return 1;
    }
    println!("OK: NetworkManager activo, nmcli + iw encontrados.");
    let devs = match nm::get_devices() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("ERROR listando dispositivos: {e}");
            return 1;
        }
    };
    match nm::find_ethernet(&devs) {
        Some(e) => println!("Ethernet: {} ({}, {})", e.name, e.state, e.connection),
        None => println!("AVISO: no hay ethernet conectado."),
    }
    match nm::find_wifi(&devs) {
        Some(w) => println!("WiFi: {} ({})", w.name, w.state),
        None => {
            eprintln!("ERROR: no se encontró interfaz wifi.");
            return 1;
        }
    }
    match nm::supports_ap() {
        Ok(true) => println!("OK: tu WiFi soporta modo AP."),
        Ok(false) => {
            eprintln!("ERROR: tu WiFi NO soporta modo AP.");
            return 1;
        }
        Err(e) => {
            eprintln!("ERROR: {e}");
            return 1;
        }
    }
    0
}

fn cmd_start(ssid: &str, password: &str, wifi: &str, con: &str, band: &str, channel: &str) -> i32 {
    if let Err(e) = nm::check_system() {
        eprintln!("ERROR: {e}");
        return 1;
    }
    let devs = nm::get_devices().unwrap_or_default();
    let auto = nm::find_wifi(&devs);
    let wifi = if wifi.is_empty() {
        auto.clone().map(|w| w.name).unwrap_or_default()
    } else {
        wifi.to_string()
    };
    match nm::find_ethernet(&devs) {
        Some(e) => println!("Compartiendo internet desde: {} ({})", e.name, e.connection),
        None => eprintln!("AVISO: no se detectó ethernet conectado."),
    }
    if let Some(w) = &auto
        && w.state.contains("connected")
    {
        println!("AVISO: {} está como cliente, se desconectará para modo AP.", w.name);
    }
    if !nm::supports_ap().unwrap_or(false) {
        eprintln!("ERROR: tu WiFi no soporta modo AP.");
        return 1;
    }
    let opts = nm::HotspotOpts {
        wifi,
        con_name: con.to_string(),
        ssid: ssid.to_string(),
        band: band.to_string(),
        channel: channel.to_string(),
        password: password.to_string(),
    };
    match nm::start_hotspot(&opts) {
        Ok(o) => {
            println!("{o}\nListo. Red: {} | Perfil: {}", opts.ssid, opts.con_name);
            let pw = nm::show_password(&opts.wifi);
            if !pw.is_empty() {
                println!("{pw}");
            }
            0
        }
        Err(e) => {
            eprintln!("ERROR creando hotspot: {e}");
            eprintln!("Típico en CachyOS: nmcli radio wifi on && rfkill unblock wifi");
            1
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let code = match cli.cmd.unwrap_or(Cmd::Tui) {
        Cmd::Tui | Cmd::Ui => tui::run_tui(),
        Cmd::Start { ssid, password, wifi, con, band, channel } => {
            cmd_start(&ssid, &password, &wifi, &con, &band, &channel)
        }
        Cmd::Stop { con } => match nm::stop_hotspot(&con) {
            Ok(o) => {
                println!("{o}\nHotspot apagado.");
                0
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                1
            }
        },
        Cmd::Restart { con } => match nm::restart_hotspot(&con) {
            Ok(o) => {
                println!("{o}");
                0
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                1
            }
        },
        Cmd::Forget { con } => match nm::delete_connection(&con) {
            Ok(o) => {
                println!("{o}");
                0
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                1
            }
        },
        Cmd::Status { con } => {
            println!("{}", nm::status_text(&con));
            0
        }
        Cmd::Devices => match nm::get_devices() {
            Ok(devs) => {
                for d in &devs {
                    println!("{:15} {:10} {:25} {}", d.name, d.dtype, d.state, d.connection);
                }
                0
            }
            Err(e) => {
                eprintln!("ERROR: {e}");
                1
            }
        },
        Cmd::Clients => {
            let devs = nm::get_devices().unwrap_or_default();
            let name = nm::find_wifi(&devs).map(|w| w.name).unwrap_or_default();
            let st = nm::get_stations(&name);
            println!("AP {name}: {} clientes", st.len());
            for s in &st {
                println!("{:17} señal={:<12} rx={:<14} tx={:<14} {}", s.mac, s.signal, s.rx, s.tx, s.time);
            }
            for n in nm::get_neighbours(&name) {
                println!("{n}");
            }
            0
        }
        Cmd::Logs { n, filter } => {
            println!("{}", nm::get_logs(&n, &filter));
            0
        }
        Cmd::Check => cmd_check(),
    };
    std::process::exit(code);
}
