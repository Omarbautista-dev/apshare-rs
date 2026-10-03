//! Dashboard estilo tui.builders/templates → Dashboard + System Monitor +
//! CLI Wizard + Form + Log Viewer + Data Explorer, con Ratatui.

use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Row, Table, Wrap},
};
use std::io;

use crate::nm;

const NAV: [(&str, &str); 9] = [
    ("1", "Estado"),
    ("2", "Dispositivos"),
    ("3", "Clientes"),
    ("4", "Red on/off"),
    ("5", "Clave/QR"),
    ("6", "Diagnóstico"),
    ("7", "Logs"),
    ("8", "Config"),
    ("9", "Ayuda"),
];

#[derive(Clone, Copy, PartialEq)]
enum Section {
    Estado,
    Dispositivos,
    Clientes,
    Red,
    Clave,
    Diagnostico,
    Logs,
    Config,
    Ayuda,
}

impl Section {
    fn from_key(k: char) -> Option<Section> {
        match k {
            '1' => Some(Section::Estado),
            '2' => Some(Section::Dispositivos),
            '3' => Some(Section::Clientes),
            '4' => Some(Section::Red),
            '5' => Some(Section::Clave),
            '6' => Some(Section::Diagnostico),
            '7' => Some(Section::Logs),
            '8' => Some(Section::Config),
            '9' => Some(Section::Ayuda),
            _ => None,
        }
    }
    fn idx(&self) -> usize {
        match self {
            Section::Estado => 0,
            Section::Dispositivos => 1,
            Section::Clientes => 2,
            Section::Red => 3,
            Section::Clave => 4,
            Section::Diagnostico => 5,
            Section::Logs => 6,
            Section::Config => 7,
            Section::Ayuda => 8,
        }
    }
    fn title(&self) -> &'static str {
        NAV[self.idx()].1
    }
}

struct Field {
    label: &'static str,
    value: String,
    secret: bool,
}

struct Form {
    title: &'static str,
    fields: Vec<Field>,
    focus: usize,
    action: FormAction,
}

#[derive(Clone, Copy, PartialEq)]
enum FormAction {
    CreateHotspot,
    SetLines,
    SetFilter,
    SetProfile,
}

struct Confirm {
    text: String,
    kind: ConfirmKind,
}

#[derive(Clone, Copy, PartialEq)]
enum ConfirmKind {
    Create,
    DeleteProfile,
    FixMasquerade,
}

struct Data {
    devices: Vec<nm::Device>,
    eth: Option<nm::Device>,
    wifi: Option<nm::Device>,
    ap_ok: bool,
    active: bool,
    stations: Vec<nm::Station>,
    neighbours: Vec<String>,
    info: std::collections::HashMap<String, String>,
}

struct App {
    section: Section,
    con_name: String,
    log_n: String,
    log_filter: String,
    logs_text: String,
    full_config: String,
    reveal: bool,
    msg: String,
    data: Data,
    form: Option<Form>,
    confirm: Option<Confirm>,
    pending: Option<nm::HotspotOpts>,
}

fn wifi_name(d: &Data) -> &str {
    d.wifi.as_ref().map(|w| w.name.as_str()).unwrap_or("")
}

fn load_data(con: &str) -> Data {
    let devices = nm::get_devices().unwrap_or_default();
    let eth = nm::find_ethernet(&devices);
    let wifi = nm::find_wifi(&devices);
    let wname = wifi.as_ref().map(|w| w.name.clone()).unwrap_or_default();
    let ap_ok = nm::supports_ap().unwrap_or(false);
    let active = nm::hotspot_active(con);
    let stations = nm::get_stations(&wname);
    let neighbours = nm::get_neighbours(&wname);
    let info = nm::hotspot_info(con);
    Data {
        devices,
        eth,
        wifi,
        ap_ok,
        active,
        stations,
        neighbours,
        info,
    }
}

fn display(v: &str) -> String {
    if v.is_empty() { "--".into() } else { v.into() }
}

// ---------------- render ----------------

fn render(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    // Header
    let hs = if app.data.active { "● ACTIVO" } else { "○ inactivo" };
    let hs_color = if app.data.active { Color::Green } else { Color::DarkGray };
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(
                format!(" apshare-rs {} · ethernet → wifi · ", crate::VERSION),
                Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan),
            ),
            Span::styled(hs, Style::default().fg(hs_color).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" · {} clientes", app.data.stations.len())),
        ]),
        Line::from(Span::styled(
            " Dashboard (tui.builders/templates → dashboard) · 1-9 panel · r refrescar · q salir",
            Style::default().fg(Color::DarkGray),
        )),
    ])
    .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(header, root[0]);

    // Main: sidebar + contenido
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(22), Constraint::Min(20)])
        .split(root[1]);

    let items: Vec<ListItem> = NAV
        .iter()
        .enumerate()
        .map(|(i, (k, name))| {
            let line = format!(" {k}. {name}");
            if i == app.section.idx() {
                ListItem::new(Line::from(Span::styled(
                    format!("▸{line}"),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                )))
            } else {
                ListItem::new(Line::from(line))
            }
        })
        .collect();
    let side = List::new(items).block(Block::default().title(" NAVEGACIÓN ").borders(Borders::ALL));
    f.render_widget(side, cols[0]);

    let body_block = Block::default()
        .title(format!(" {} ", app.section.title().to_uppercase()))
        .borders(Borders::ALL);
    let inner = body_block.inner(cols[1]);
    f.render_widget(body_block, cols[1]);
    render_section(f, app, inner);

    // Overlay: formulario o confirmación
    if let Some(form) = &app.form {
        render_form(f, form);
    } else if let Some(c) = &app.confirm {
        render_confirm(f, &c.text);
    }

    // Status bar
    let eth = app.data.eth.as_ref().map(|e| e.name.as_str()).unwrap_or("--");
    let status = Paragraph::new(vec![
        Line::from(Span::styled(
            app.msg.clone(),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(vec![
            Span::styled(
                format!(
                    " eth:{} wifi:{} ap:{} hs:{} perfil:{}",
                    eth,
                    wifi_name(&app.data),
                    if app.data.ap_ok { "sí" } else { "no" },
                    if app.data.active { "on" } else { "off" },
                    app.con_name
                ),
                Style::default().fg(Color::DarkGray),
            ),
        ]),
    ])
    .block(Block::default().borders(Borders::TOP));
    f.render_widget(status, root[2]);
}

fn render_section(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    match app.section {
        Section::Estado => render_estado(f, app, area),
        Section::Dispositivos => render_dispositivos(f, app, area),
        Section::Clientes => render_clientes(f, app, area),
        Section::Red => render_red(f, app, area),
        Section::Clave => render_clave(f, app, area),
        Section::Diagnostico => render_diag(f, app, area),
        Section::Logs => render_logs(f, app, area),
        Section::Config => render_config(f, app, area),
        Section::Ayuda => render_ayuda(f, area),
    }
}

fn kpi_lines(label: &str, value: &str, color: Color) -> Vec<Line<'static>> {
    vec![
        Line::from(Span::styled(format!("● {label}"), Style::default().fg(color))),
        Line::from(Span::styled(format!("  {value}"), Style::default().add_modifier(Modifier::BOLD))),
    ]
}

fn render_estado(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let d = &app.data;
    let eth_v = d.eth.as_ref().map(|e| format!("{} ({})", e.name, e.connection)).unwrap_or("sin ethernet".into());
    let eth_c = if d.eth.is_some() { Color::Green } else { Color::Red };
    let (wifi_v, wifi_c) = match &d.wifi {
        Some(w) => (format!("{} [{}]", w.name, w.state), Color::Green),
        None => ("no detectado".into(), Color::Red),
    };
    let (ap_v, ap_c) = if d.ap_ok { ("SÍ".to_string(), Color::Green) } else { ("NO".to_string(), Color::Red) };
    let (hs_v, hs_c) = if d.active {
        (format!("activo ({})", app.con_name), Color::Green)
    } else {
        ("inactivo".into(), Color::DarkGray)
    };
    let ssid = d.info.get("ssid").map(|s| s.as_str()).unwrap_or("--");
    let mut ch = d.info.get("channel").map(|s| s.as_str()).unwrap_or("").to_string();
    if ch.is_empty() || ch == "0" {
        ch = "auto".into();
    }
    let ip = d.info.get("ip").map(|s| s.as_str()).filter(|s| !s.is_empty())
        .or_else(|| d.info.get("addresses").map(|s| s.as_str()))
        .unwrap_or("--");

    let mut lines = Vec::new();
    let cards: Vec<(String, String, Color)> = vec![
        ("Ethernet".into(), eth_v, eth_c),
        ("WiFi".into(), wifi_v, wifi_c),
        ("Modo AP".into(), ap_v, ap_c),
        ("Hotspot".into(), hs_v, hs_c),
        ("Clientes".into(), d.stations.len().to_string(), Color::DarkGray),
        ("SSID".into(), display(ssid), Color::DarkGray),
    ];
    for (l, v, c) in cards {
        lines.extend(kpi_lines(&l, &v, c));
    }
    if d.active {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "Detalle: banda={} canal={} ip={} seg={}",
                display(d.info.get("band").map(|s| s.as_str()).unwrap_or("")),
                ch,
                ip,
                display(d.info.get("security").map(|s| s.as_str()).unwrap_or("")),
            ),
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "Tip: ve a [4] Red on/off para iniciar con el wizard.",
            Style::default().fg(Color::DarkGray),
        )));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_dispositivos(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let rows: Vec<Row> = app
        .data
        .devices
        .iter()
        .map(|d| {
            let conn = if d.connection.is_empty() { "--" } else { &d.connection };
            let style = if d.state.contains("connected") && !d.state.contains("externally") && !d.state.contains("disconnected") {
                Style::default().fg(Color::Green)
            } else if d.state.contains("disconnected") {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            Row::new(vec![
                ratatui::widgets::Cell::from(d.name.clone()),
                ratatui::widgets::Cell::from(d.dtype.clone()),
                ratatui::widgets::Cell::from(d.state.clone()).style(style),
                ratatui::widgets::Cell::from(conn.to_string()),
            ])
        })
        .collect();
    let t = Table::new(
        rows,
        [Constraint::Length(16), Constraint::Length(10), Constraint::Length(22), Constraint::Min(10)],
    )
    .header(
        Row::new(vec!["INTERFAZ", "TIPO", "ESTADO", "CONEXIÓN"])
            .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD)),
    );
    f.render_widget(t, area);
}

fn render_clientes(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let d = &app.data;
    let mut lines = vec![Line::from(vec![
        Span::raw("AP: "),
        Span::styled(wifi_name(d).to_string(), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(format!(" · {} conectados (práctico 8-32, teórico /24)", d.stations.len())),
    ]), Line::from("")];
    if d.stations.is_empty() {
        lines.push(Line::from(Span::styled("Sin clientes (¿hotspot inactivo?).", Style::default().fg(Color::DarkGray))));
    }
    for s in &d.stations {
        lines.push(Line::from(Span::styled(format!("● {}", s.mac), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))));
        lines.push(Line::from(format!("  señal={} rx={} tx={} {}", display(&s.signal), display(&s.rx), display(&s.tx), display(&s.time))));
    }
    if !d.neighbours.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("IPs (ip neigh):", Style::default().add_modifier(Modifier::BOLD))));
        for n in &d.neighbours {
            lines.push(Line::from(format!("  {n}")));
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_red(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let st = if app.data.active { "activo" } else { "inactivo" };
    let lines = vec![
        Line::from(vec![
            Span::raw("Perfil: "),
            Span::styled(app.con_name.clone(), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" · {st}")),
        ]),
        Line::from(""),
        Line::from("  a) iniciar (wizard)"),
        Line::from("  b) apagar"),
        Line::from("  c) reiniciar"),
        Line::from("  d) olvidar perfil"),
        Line::from(""),
        Line::from(Span::styled("Pulsa la letra de la acción.", Style::default().fg(Color::DarkGray))),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn render_clave(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines = Vec::new();
    match nm::wifi_qr_payload(wifi_name(&app.data)) {
        None => lines.push(Line::from(Span::styled(
            "Sin datos (¿hotspot inactivo?).",
            Style::default().fg(Color::Yellow),
        ))),
        Some((payload, ssid, sec)) => {
            let sec_s = if sec.is_empty() { "?" } else { sec.as_str() };
            lines.push(Line::from(vec![
                Span::raw("Red: "),
                Span::styled(ssid, Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(format!(" · {sec_s} · escanea para conectar")),
            ]));
            lines.push(Line::from(""));
            if nm::has_qr() {
                match nm::qr_ansi(&payload) {
                    Some(qr) => {
                        for l in qr.lines() {
                            lines.push(Line::from(l.to_string()));
                        }
                    }
                    None => lines.push(Line::from("No se pudo generar el QR.")),
                }
            } else {
                lines.push(Line::from(
                    "Instala qrencode para ver el QR aquí: sudo pacman -S qrencode",
                ));
            }
            lines.push(Line::from(""));
            if app.reveal {
                for l in nm::show_password(wifi_name(&app.data)).lines() {
                    lines.push(Line::from(l.to_string()));
                }
                lines.push(Line::from(Span::styled(
                    "v) ocultar clave",
                    Style::default().fg(Color::DarkGray),
                )));
            } else {
                lines.push(Line::from(Span::styled(
                    "Clave oculta · v) mostrar clave",
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_diag(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let d = &app.data;
    let mut lines = vec![
        Line::from(Span::styled("● NetworkManager + nmcli + iw: ok", Style::default().fg(Color::Green))),
        Line::from(if let Some(e) = d.eth.as_ref() {
            Span::styled(format!("● ethernet: {}", e.name), Style::default().fg(Color::Green))
        } else {
            Span::styled("● sin ethernet conectado".to_string(), Style::default().fg(Color::Red))
        }),
        Line::from(if d.ap_ok {
            Span::styled("● modo AP: SÍ".to_string(), Style::default().fg(Color::Green))
        } else {
            Span::styled("● modo AP: NO".to_string(), Style::default().fg(Color::Red))
        }),
        Line::from(""),
        Line::from(Span::styled("rfkill wifi:", Style::default().add_modifier(Modifier::BOLD))),
    ];
    for l in nm::get_rfkill().lines().take(8) {
        lines.push(Line::from(format!("  {l}")));
    }
    let m = nm::masquerade_status();
    lines.push(Line::from(""));
    if m.is_empty() {
        lines.push(Line::from("masquerade: firewalld no instalado, NM hace NAT solo"));
    } else if m.contains("yes") {
        lines.push(Line::from(Span::styled("masquerade: yes", Style::default().fg(Color::Green))));
    } else {
        lines.push(Line::from(Span::styled("masquerade: no (clientes sin internet) → pulsa f para arreglar", Style::default().fg(Color::Red))));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_logs(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines = vec![
        Line::from(format!("journalctl -u NetworkManager -n {} filtro='{}'", app.log_n, app.log_filter)),
        Line::from(Span::styled("a) ver · b) líneas · c) filtrar · d) limpiar filtro", Style::default().fg(Color::DarkGray))),
        Line::from(""),
    ];
    if app.logs_text.is_empty() {
        lines.push(Line::from(Span::styled("Pulsa a para cargar.", Style::default().fg(Color::DarkGray))));
    } else {
        for l in app.logs_text.lines() {
            lines.push(Line::from(l.to_string()));
        }
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_config(f: &mut Frame, app: &App, area: ratatui::layout::Rect) {
    let mut lines = vec![
        Line::from(vec![
            Span::raw("Perfil: "),
            Span::styled(app.con_name.clone(), Style::default().add_modifier(Modifier::BOLD)),
        ]),
    ];
    let i = &app.data.info;
    if !i.is_empty() {
        lines.push(Line::from(format!(
            "SSID={} banda={} canal={} ip={} seg={}",
            display(i.get("ssid").map(|s| s.as_str()).unwrap_or("")),
            display(i.get("band").map(|s| s.as_str()).unwrap_or("")),
            display(i.get("channel").map(|s| s.as_str()).unwrap_or("")),
            display(i.get("ip").map(|s| s.as_str()).filter(|s| !s.is_empty()).or_else(|| i.get("addresses").map(|s| s.as_str())).unwrap_or("")),
            display(i.get("security").map(|s| s.as_str()).unwrap_or("")),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("  a) cambiar perfil"));
    if !app.full_config.is_empty() {
        lines.push(Line::from(""));
        for l in app.full_config.lines().take(30) {
            lines.push(Line::from(l.to_string()));
        }
    } else {
        lines.push(Line::from("  b) ver config completa"));
    }
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn render_ayuda(f: &mut Frame, area: ratatui::layout::Rect) {
    let lines = vec![
        Line::from("Atajos: 1-9 cambian de panel, r refresca, q/Esc sale."),
        Line::from("Flujo: [6] diagnóstico → [4] iniciar (wizard) → [3] clientes → [5] clave."),
        Line::from("CLI: apshare-rs check|devices|status|clients|logs|start|stop|restart|forget."),
        Line::from(""),
        Line::from("Patrón visual: tui.builders/templates → dashboard + system monitor"),
        Line::from("+ wizard + log viewer (reimplementado en Rust/Ratatui)."),
    ];
    f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
}

fn centered(area: ratatui::layout::Rect, w: u16, h: u16) -> ratatui::layout::Rect {
    let hbox = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(w),
            Constraint::Fill(1),
        ])
        .split(area);
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Fill(1),
            Constraint::Length(h),
            Constraint::Fill(1),
        ])
        .split(hbox[1])[1]
}

fn render_form(f: &mut Frame, form: &Form) {
    let area = centered(f.area(), 60, (form.fields.len() as u16) * 3 + 6);
    let block = Block::default().title(format!(" {} ", form.title)).borders(Borders::ALL);
    let inner = block.inner(area);
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(block, area);
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Length(3); form.fields.len() + 1])
        .split(inner);
    for (i, fd) in form.fields.iter().enumerate() {
        let shown = if fd.secret && !fd.value.is_empty() {
            "•".repeat(fd.value.chars().count().min(24))
        } else {
            fd.value.clone()
        };
        let style = if i == form.focus {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let cur = if i == form.focus { "▌" } else { "" };
        f.render_widget(
            Paragraph::new(format!("{shown}{cur}")).block(
                Block::default()
                    .title(format!(" {} ", fd.label))
                    .borders(Borders::ALL)
                    .border_style(style),
            ),
            rows[i],
        );
    }
    f.render_widget(
        Paragraph::new("Enter: siguiente · Esc: cancelar · último Enter: confirmar").style(Style::default().fg(Color::DarkGray)),
        rows[form.fields.len()],
    );
}

fn render_confirm(f: &mut Frame, text: &str) {
    let area = centered(f.area(), 60, 7);
    f.render_widget(ratatui::widgets::Clear, area);
    let p = Paragraph::new(vec![
        Line::from(""),
        Line::from(text.to_string()),
        Line::from(""),
        Line::from(Span::styled("s = sí · n/Esc = no", Style::default().fg(Color::DarkGray))),
    ])
    .block(Block::default().title(" Confirmar ").borders(Borders::ALL));
    f.render_widget(p, area);
}

fn wizard_form(def_wifi: &str, con: &str) -> Form {
    Form {
        title: "Nuevo hotspot (wizard)",
        fields: vec![
            Field { label: "SSID", value: "MiLaptop".into(), secret: false },
            Field { label: "Password (min 8)", value: String::new(), secret: true },
            Field { label: "Interfaz WiFi", value: def_wifi.to_string(), secret: false },
            Field { label: "Banda bg/a/6GHz", value: "bg".into(), secret: false },
            Field { label: "Canal (vacío=auto)", value: String::new(), secret: false },
            Field { label: "Perfil", value: con.to_string(), secret: false },
        ],
        focus: 0,
        action: FormAction::CreateHotspot,
    }
}

// ---------------- event loop ----------------

pub fn run_tui() -> i32 {
    if let Err(e) = nm::check_system() {
        eprintln!("ERROR: {e}");
        return 1;
    }
    // Sudo se autentica AQUÍ, en terminal normal: escribes tu contraseña
    // una vez (si hace falta). Dentro de la TUI sudo nunca pregunta (-n);
    // si el permiso expira, se avisa en vez de romper el dibujo.
    nm::set_noninteractive();
    let sudo_ok = nm::preauth_sudo();
    enable_raw_mode().expect("raw mode");
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).expect("alt screen");
    let backend = CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend).expect("terminal");
    let code = main_loop(&mut term, sudo_ok);
    disable_raw_mode().ok();
    execute!(term.backend_mut(), LeaveAlternateScreen).ok();
    code
}

fn refresh(app: &mut App) {
    app.data = load_data(&app.con_name);
}

fn main_loop(term: &mut Terminal<CrosstermBackend<io::Stdout>>, sudo_ok: bool) -> i32 {
    let mut app = App {
        section: Section::Estado,
        con_name: "Hotspot".into(),
        log_n: "50".into(),
        log_filter: String::new(),
        logs_text: String::new(),
        full_config: String::new(),
        reveal: false,
        msg: if sudo_ok {
            String::new()
        } else {
            "Sin sudo: acciones de red bloqueadas. Sal con q, ejecuta `sudo -v` y reingresa.".into()
        },
        data: load_data("Hotspot"),
        form: None,
        confirm: None,
        pending: None,
    };

    loop {
        term.draw(|f| render(f, &app)).expect("draw");
        let ev = match event::read() {
            Ok(e) => e,
            Err(_) => return 0,
        };
        let Event::Key(k) = ev else { continue };
        if k.modifiers.contains(KeyModifiers::CONTROL) && k.code == KeyCode::Char('c') {
            return 0;
        }

        // --- formulario activo: edición de texto ---
        if let Some(form) = app.form.as_mut() {
            match k.code {
                KeyCode::Esc => {
                    app.form = None;
                    app.msg = "Cancelado.".into();
                }
                KeyCode::Enter => {
                    if form.focus + 1 < form.fields.len() {
                        form.focus += 1;
                    } else {
                        let form = app.form.take().unwrap();
                        submit_form(&mut app, form);
                        refresh(&mut app);
                    }
                }
                KeyCode::Backspace => {
                    if let Some(fd) = form.fields.get_mut(form.focus) {
                        fd.value.pop();
                    }
                }
                KeyCode::Char(c) => {
                    if let Some(fd) = form.fields.get_mut(form.focus) {
                        fd.value.push(c);
                    }
                }
                KeyCode::Up => {
                    if form.focus > 0 {
                        form.focus -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Tab if form.focus + 1 < form.fields.len() => {
                    form.focus += 1;
                }
                KeyCode::Down | KeyCode::Tab => {}
                _ => {}
            }
            continue;
        }

        // --- confirmación activa ---
        if app.confirm.is_some() {
            match k.code {
                KeyCode::Char('s') | KeyCode::Char('S') | KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let c = app.confirm.take().unwrap();
                    apply_confirm(&mut app, c.kind);
                    refresh(&mut app);
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    app.confirm = None;
                    app.pending = None;
                    app.msg = "Cancelado.".into();
                }
                _ => {}
            }
            continue;
        }

        // --- navegación normal ---
        match k.code {
            KeyCode::Char('q') | KeyCode::Esc => return 0,
            KeyCode::Char('r') => {
                refresh(&mut app);
                app.msg = "Actualizado.".into();
            }
            KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                if let Some(s) = Section::from_key(c) {
                    app.section = s;
                    app.full_config.clear();
                    refresh(&mut app);
                }
            }
            KeyCode::Char(c) => {
                handle_section_key(&mut app, c);
                refresh(&mut app);
            }
            _ => {}
        }
    }
}

fn submit_form(app: &mut App, form: Form) {
    let v = |i: usize| form.fields.get(i).map(|f| f.value.trim().to_string()).unwrap_or_default();
    match form.action {
        FormAction::CreateHotspot => {
            let opts = nm::HotspotOpts {
                ssid: v(0),
                password: v(1),
                wifi: v(2),
                band: v(3),
                channel: v(4),
                con_name: if v(5).is_empty() { app.con_name.clone() } else { v(5) },
            };
            if let Err(e) = opts.validate() {
                app.msg = format!("ERROR: {e}");
                return;
            }
            app.pending = Some(opts.clone());
            app.confirm = Some(Confirm {
                text: format!("Crear '{}' en {} ({}+{})?", opts.ssid, opts.wifi, opts.band, app.con_name),
                kind: ConfirmKind::Create,
            });
        }
        FormAction::SetLines => {
            if !v(0).is_empty() {
                app.log_n = v(0);
            }
            app.logs_text = nm::get_logs(&app.log_n, &app.log_filter);
        }
        FormAction::SetFilter => {
            app.log_filter = v(0);
            app.logs_text = nm::get_logs(&app.log_n, &app.log_filter);
        }
        FormAction::SetProfile => {
            if !v(0).is_empty() {
                app.con_name = v(0);
            }
        }
    }
}

fn apply_confirm(app: &mut App, kind: ConfirmKind) {
    if !guard_sudo(app) {
        return;
    }
    match kind {
        ConfirmKind::Create => {
            if let Some(o) = app.pending.take() {
                match nm::start_hotspot(&o) {
                    Ok(_) => {
                        app.con_name = o.con_name.clone();
                        app.msg = format!("Hotspot '{}' activo.", o.ssid);
                    }
                    Err(e) => app.msg = format!("ERROR: {e}"),
                }
            }
        }
        ConfirmKind::DeleteProfile => match nm::delete_connection(&app.con_name) {
            Ok(_) => app.msg = "Perfil eliminado.".into(),
            Err(e) => app.msg = format!("ERROR: {e}"),
        },
        ConfirmKind::FixMasquerade => match nm::fix_masquerade() {
            Ok(_) => app.msg = "Masquerade activado.".into(),
            Err(e) => app.msg = format!("ERROR: {e}"),
        },
    }
}

/// Verifica permiso sudo vigente sin pedir nada. Si falló, deja mensaje
/// claro en vez de lanzar un `sudo` que pintaría su prompt sobre la TUI.
fn guard_sudo(app: &mut App) -> bool {
    if nm::sudo_alive() {
        return true;
    }
    app.msg = "Sin permiso sudo (expiró o denegado). Sal con q, ejecuta `sudo -v` y reingresa.".into();
    false
}

fn single_form(title: &'static str, label: &'static str, def: &str, action: FormAction) -> Form {
    Form {
        title,
        fields: vec![Field { label, value: def.to_string(), secret: false }],
        focus: 0,
        action,
    }
}

fn handle_section_key(app: &mut App, c: char) {
    let c = c.to_ascii_lowercase();
    match app.section {
        Section::Red => match c {
            'a' => app.form = Some(wizard_form(wifi_name(&app.data), &app.con_name.clone())),
            'b' => {
                if !guard_sudo(app) {
                    return;
                }
                match nm::stop_hotspot(&app.con_name) {
                    Ok(_) => app.msg = "Hotspot apagado.".into(),
                    Err(e) => app.msg = format!("ERROR: {e}"),
                }
            }
            'c' => {
                if !guard_sudo(app) {
                    return;
                }
                match nm::restart_hotspot(&app.con_name) {
                    Ok(_) => app.msg = "Hotspot reiniciado.".into(),
                    Err(e) => app.msg = format!("ERROR: {e}"),
                }
            }
            'd' => {
                app.confirm = Some(Confirm {
                    text: format!("¿Eliminar el perfil '{}'?", app.con_name),
                    kind: ConfirmKind::DeleteProfile,
                });
            }
            _ => {}
        },
        Section::Logs => match c {
            'a' => app.logs_text = nm::get_logs(&app.log_n, &app.log_filter),
            'b' => app.form = Some(single_form("Líneas de log", "N", &app.log_n.clone(), FormAction::SetLines)),
            'c' => app.form = Some(single_form("Filtrar logs", "Texto", &app.log_filter.clone(), FormAction::SetFilter)),
            'd' => {
                app.log_filter.clear();
                app.logs_text = nm::get_logs(&app.log_n, "");
            }
            _ => {}
        },
        Section::Config => match c {
            'a' => app.form = Some(single_form("Perfil activo", "Nombre", &app.con_name.clone(), FormAction::SetProfile)),
            'b' => {
                app.full_config = nm::run("nmcli", &["con", "show", &app.con_name]).unwrap_or_default();
            }
            _ => {}
        },
        Section::Clave if c == 'v' => {
            app.reveal = !app.reveal;
        }
        Section::Clave => {}
        Section::Diagnostico if c == 'f' => {
            app.confirm = Some(Confirm {
                text: "¿Activar masquerade de firewalld?".into(),
                kind: ConfirmKind::FixMasquerade,
            });
        }
        Section::Diagnostico => {}
        _ => {}
    }
}
