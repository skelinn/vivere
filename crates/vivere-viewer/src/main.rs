//! The viewer is a window onto the world, never a hand inside it. It steps
//! the same `World::step` the CLI uses, consumes no simulation randomness,
//! and only reads state — what you watch is exactly what a headless run
//! with the same seed would have logged.

use macroquad::prelude::*;
use macroquad::ui::root_ui;
use vivere_core::brain::{OUTPUT_NAMES, SENSE_NAMES};
use vivere_core::genome::Connection;
use vivere_core::organism::Organism;
use vivere_core::{Config, World};

const PANEL_W: f32 = 330.0;
const TOPBAR_H: f32 = 70.0;
const LIGHT_TILE: f32 = 25.0;
const SPEEDS: [u32; 8] = [1, 2, 4, 8, 16, 32, 64, 128];
/// Max sim time per frame, so high speeds degrade gracefully instead of
/// freezing the window.
const FRAME_BUDGET: f64 = 0.012;

struct Camera {
    zoom: f32,
    off_x: f32,
    off_y: f32,
}

impl Camera {
    fn fit(world_w: f32, world_h: f32) -> Self {
        let (sw, sh) = (screen_width() - PANEL_W, screen_height() - TOPBAR_H);
        let zoom = (sw / world_w).min(sh / world_h).max(0.05);
        Self {
            zoom,
            off_x: (sw - world_w * zoom) * 0.5,
            off_y: TOPBAR_H + (sh - world_h * zoom) * 0.5,
        }
    }
}

fn window_conf() -> Conf {
    Conf {
        window_title: "vivere — protocell".to_owned(),
        window_width: 1440,
        window_height: 940,
        high_dpi: true,
        ..Default::default()
    }
}

fn usage() -> ! {
    eprintln!("usage: vivere-viewer [--seed N] [--config file.toml] [--resume file.snap]");
    std::process::exit(2);
}

fn parse_args() -> (u64, Option<String>, Option<String>) {
    let args: Vec<String> = std::env::args().collect();
    let (mut seed, mut config, mut resume) = (42u64, None, None);
    let mut i = 1;
    while i < args.len() {
        let value = args.get(i + 1).cloned();
        match args[i].as_str() {
            "--seed" => {
                seed = value
                    .and_then(|v| v.parse().ok())
                    .unwrap_or_else(|| usage())
            }
            "--config" => config = Some(value.unwrap_or_else(|| usage())),
            "--resume" => resume = Some(value.unwrap_or_else(|| usage())),
            _ => usage(),
        }
        i += 2;
    }
    (seed, config, resume)
}

fn load_world() -> World {
    let (seed, config, resume) = parse_args();
    if let Some(path) = resume {
        let bytes = std::fs::read(&path).unwrap_or_else(|e| {
            eprintln!("error reading {path}: {e}");
            std::process::exit(1);
        });
        return World::from_snapshot_bytes(&bytes).unwrap_or_else(|e| {
            eprintln!("error: {e}");
            std::process::exit(1);
        });
    }
    let cfg = match config {
        Some(path) => {
            let text = std::fs::read_to_string(&path).unwrap_or_else(|e| {
                eprintln!("error reading {path}: {e}");
                std::process::exit(1);
            });
            toml_from_str(&text)
        }
        None => Config::default(),
    };
    World::new(cfg, seed)
}

/// vivere-viewer deliberately avoids a TOML dependency; configs are a CLI
/// concern. Accept snapshots or defaults here, and point config users at
/// the snapshot workflow instead.
fn toml_from_str(_text: &str) -> Config {
    eprintln!(
        "note: the viewer doesn't parse TOML configs; run the CLI with --config and \
         --save-snapshot, then open the snapshot with --resume"
    );
    std::process::exit(2);
}

fn hsv(h: f32, s: f32, v: f32) -> Color {
    let h = (h.rem_euclid(1.0)) * 6.0;
    let i = h.floor();
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Color::new(r, g, b, 1.0)
}

fn conn_label(c: &Connection) -> String {
    let src = if c.from_hidden {
        format!("h{}", c.from)
    } else {
        SENSE_NAMES[c.from as usize].to_string()
    };
    let dst = if c.to_output {
        OUTPUT_NAMES[c.to as usize].to_string()
    } else {
        format!("h{}", c.to)
    };
    format!("{src:>9} > {dst:<7} {:+.2}", c.weight)
}

#[macroquad::main(window_conf)]
async fn main() {
    let mut world = load_world();
    let mut cam = Camera::fit(world.cfg.world.width, world.cfg.world.height);
    let mut paused = false;
    let mut speed_idx = 0usize;
    let mut selected: Option<u64> = None;
    let mut last_mouse = mouse_position();

    // The light field is static; sample it once into tiles.
    let light_tiles = build_light_tiles(&world);

    loop {
        let (ww, wh) = (world.cfg.world.width, world.cfg.world.height);

        // --- input ---
        if is_key_pressed(KeyCode::Space) {
            paused = !paused;
        }
        let step_once = paused && is_key_pressed(KeyCode::Period);
        if is_key_pressed(KeyCode::Equal) || is_key_pressed(KeyCode::Up) {
            speed_idx = (speed_idx + 1).min(SPEEDS.len() - 1);
        }
        if is_key_pressed(KeyCode::Minus) || is_key_pressed(KeyCode::Down) {
            speed_idx = speed_idx.saturating_sub(1);
        }
        if is_key_pressed(KeyCode::R) {
            cam = Camera::fit(ww, wh);
        }
        if is_key_pressed(KeyCode::Escape) {
            selected = None;
        }

        let mouse = mouse_position();
        let wheel = mouse_wheel().1;
        if wheel != 0.0 {
            let f = if wheel > 0.0 { 1.12 } else { 1.0 / 1.12 };
            cam.off_x = mouse.0 - (mouse.0 - cam.off_x) * f;
            cam.off_y = mouse.1 - (mouse.1 - cam.off_y) * f;
            cam.zoom *= f;
        }
        if is_mouse_button_down(MouseButton::Right) {
            cam.off_x += mouse.0 - last_mouse.0;
            cam.off_y += mouse.1 - last_mouse.1;
        }
        last_mouse = mouse;

        // --- ui buttons (top bar) ---
        let mut ui_clicked = false;
        if root_ui().button(vec2(12.0, 36.0), if paused { "play " } else { "pause" }) {
            paused = !paused;
            ui_clicked = true;
        }
        if root_ui().button(vec2(76.0, 36.0), "step") && paused {
            world.step();
            ui_clicked = true;
        }
        if root_ui().button(vec2(126.0, 36.0), "-") {
            speed_idx = speed_idx.saturating_sub(1);
            ui_clicked = true;
        }
        if root_ui().button(vec2(150.0, 36.0), "+") {
            speed_idx = (speed_idx + 1).min(SPEEDS.len() - 1);
            ui_clicked = true;
        }

        // click to inspect (ignore clicks on the top bar / panel / buttons)
        if is_mouse_button_pressed(MouseButton::Left)
            && !ui_clicked
            && mouse.1 > TOPBAR_H
            && mouse.0 < screen_width() - PANEL_W
        {
            let wx = (mouse.0 - cam.off_x) / cam.zoom;
            let wy = (mouse.1 - cam.off_y) / cam.zoom;
            selected = pick_organism(&world, wx, wy, 14.0 / cam.zoom);
        }

        // --- advance the world ---
        if step_once {
            world.step();
        } else if !paused {
            let t0 = get_time();
            for _ in 0..SPEEDS[speed_idx] {
                world.step();
                if get_time() - t0 > FRAME_BUDGET {
                    break;
                }
            }
        }

        // --- draw ---
        clear_background(Color::new(0.043, 0.055, 0.071, 1.0));

        for &(x, y, w, h, l) in &light_tiles {
            draw_rectangle(
                x * cam.zoom + cam.off_x,
                y * cam.zoom + cam.off_y,
                w * cam.zoom + 1.0,
                h * cam.zoom + 1.0,
                Color::new(0.16 + 0.13 * l, 0.15 + 0.11 * l, 0.055 + 0.04 * l, 0.35),
            );
        }

        for d in &world.detritus {
            draw_circle(
                d.x * cam.zoom + cam.off_x,
                d.y * cam.zoom + cam.off_y,
                (1.6 * cam.zoom).max(1.0),
                Color::new(0.45, 0.38, 0.30, 0.9),
            );
        }
        let pellet_e = world.cfg.env.food_energy as f32;
        for f in &world.food {
            let s = (1.2 + 1.3 * (f.energy as f32 / pellet_e).min(2.0)) * cam.zoom;
            draw_rectangle(
                f.x * cam.zoom + cam.off_x - s * 0.5,
                f.y * cam.zoom + cam.off_y - s * 0.5,
                s.max(1.5),
                s.max(1.5),
                Color::new(0.30, 0.78, 0.42, 0.95),
            );
        }

        let mut selected_org: Option<&Organism> = None;
        for o in &world.organisms {
            let sx = o.x * cam.zoom + cam.off_x;
            let sy = o.y * cam.zoom + cam.off_y;
            let r = (o.radius(&world.cfg) * cam.zoom).max(1.5);
            let energy_frac = (o.energy / o.capacity(&world.cfg)).clamp(0.15, 1.0) as f32;
            let color = hsv(o.genome.body.hue, 0.72, 0.35 + 0.6 * energy_frac);
            draw_circle(sx, sy, r, color);
            draw_line(
                sx,
                sy,
                sx + o.heading.cos() * r * 1.8,
                sy + o.heading.sin() * r * 1.8,
                (0.15 * r).max(1.0),
                Color::new(0.9, 0.9, 0.9, 0.8),
            );
            if Some(o.id) == selected {
                draw_circle_lines(sx, sy, r + 4.0, 1.5, WHITE);
                selected_org = Some(o);
            }
        }

        draw_hud(&world, paused, speed_idx);
        if selected.is_some() && selected_org.is_none() {
            // the inspected organism died; keep the panel honest
            selected = None;
        }
        if let Some(o) = selected_org {
            draw_inspector(o, &world.cfg);
        }
        if world.organisms.is_empty() {
            let msg = format!("extinct at tick {}", world.tick);
            draw_text(
                &msg,
                screen_width() * 0.5 - 160.0,
                screen_height() * 0.5,
                40.0,
                RED,
            );
        }

        next_frame().await;
    }
}

fn build_light_tiles(world: &World) -> Vec<(f32, f32, f32, f32, f32)> {
    let (w, h) = (world.cfg.world.width, world.cfg.world.height);
    let max = world.light.max_value();
    let (nx, ny) = ((w / LIGHT_TILE) as usize, (h / LIGHT_TILE) as usize);
    let mut tiles = Vec::with_capacity(nx * ny);
    for iy in 0..ny {
        for ix in 0..nx {
            let x = ix as f32 * LIGHT_TILE;
            let y = iy as f32 * LIGHT_TILE;
            let l = world
                .light
                .sample(x + LIGHT_TILE * 0.5, y + LIGHT_TILE * 0.5, w, h)
                / max;
            tiles.push((x, y, LIGHT_TILE, LIGHT_TILE, l));
        }
    }
    tiles
}

fn pick_organism(world: &World, wx: f32, wy: f32, slack: f32) -> Option<u64> {
    let (w, h) = (world.cfg.world.width, world.cfg.world.height);
    let mut best: Option<(u64, f32)> = None;
    for o in &world.organisms {
        let dx = vivere_core::grid::wrap_delta(wx, o.x, w);
        let dy = vivere_core::grid::wrap_delta(wy, o.y, h);
        let d2 = dx * dx + dy * dy;
        let reach = o.radius(&world.cfg) + slack;
        if d2 <= reach * reach && best.is_none_or(|(_, b)| d2 < b) {
            best = Some((o.id, d2));
        }
    }
    best.map(|(id, _)| id)
}

fn draw_hud(world: &World, paused: bool, speed_idx: usize) {
    let white = Color::new(0.92, 0.94, 0.96, 1.0);
    let dim = Color::new(0.65, 0.70, 0.75, 1.0);
    let line1 = format!(
        "tick {}   pop {}   food {}   detritus {}   births {}   deaths {}/{} (starve/age)",
        world.tick,
        world.organisms.len(),
        world.food.len(),
        world.detritus.len(),
        world.births,
        world.deaths_starve,
        world.deaths_age,
    );
    let line2 = format!(
        "energy {:.0}   drift {:+.1e}   fps {}   speed x{}{}",
        world.world_energy(),
        world.conservation_error(),
        get_fps(),
        SPEEDS[speed_idx],
        if paused { "   PAUSED" } else { "" },
    );
    draw_text(&line1, 12.0, 20.0, 17.0, white);
    draw_text(&line2, 210.0, 50.0, 17.0, dim);
    draw_text(
        "space pause   . step   +/- speed   wheel zoom   right-drag pan   click inspect   R refit",
        12.0,
        screen_height() - 10.0,
        15.0,
        Color::new(0.5, 0.55, 0.6, 1.0),
    );
}

fn draw_inspector(o: &Organism, cfg: &Config) {
    let x0 = screen_width() - PANEL_W;
    draw_rectangle(
        x0,
        0.0,
        PANEL_W,
        screen_height(),
        Color::new(0.0, 0.0, 0.0, 0.78),
    );
    let x = x0 + 14.0;
    let mut y = 26.0;
    let line = |text: &str, size: f32, color: Color, y: &mut f32| {
        draw_text(text, x, *y, size, color);
        *y += size * 1.25;
    };
    let head = Color::new(0.95, 0.95, 0.98, 1.0);
    let body = Color::new(0.78, 0.82, 0.86, 1.0);
    let faint = Color::new(0.58, 0.62, 0.66, 1.0);

    line(&format!("organism #{}", o.id), 22.0, head, &mut y);
    line(
        &format!(
            "generation {}   age {} / {:.0}",
            o.generation, o.age, o.genome.body.max_age
        ),
        16.0,
        body,
        &mut y,
    );
    line(
        &format!(
            "energy {:.1} / {:.1}   soma {:.1}",
            o.energy,
            o.capacity(cfg),
            o.soma
        ),
        16.0,
        body,
        &mut y,
    );
    line(
        &format!(
            "size {:.2}  speed {:.2}  metab {:.2}",
            o.genome.body.size, o.genome.body.max_speed, o.genome.body.metab
        ),
        16.0,
        body,
        &mut y,
    );
    line(
        &format!(
            "hue {:.2}   upkeep {:.4}/tick",
            o.genome.body.hue,
            o.upkeep(cfg)
        ),
        16.0,
        body,
        &mut y,
    );

    y += 8.0;
    line("senses", 17.0, head, &mut y);
    for (name, value) in SENSE_NAMES.iter().zip(o.last_senses) {
        line(&format!("{name:>9} {value:+.2}"), 14.0, faint, &mut y);
    }
    line(
        &format!(
            "out: turn {:+.2}  speed {:.2}  bite {:.2}",
            o.last_turn, o.last_speed, o.last_bite
        ),
        15.0,
        body,
        &mut y,
    );

    y += 8.0;
    line(
        &format!("brain — {} connections", o.genome.connections.len()),
        17.0,
        head,
        &mut y,
    );
    let max_rows = (((screen_height() - y) / 16.0) as usize).saturating_sub(1);
    for c in o.genome.connections.iter().take(max_rows) {
        line(&conn_label(c), 14.0, faint, &mut y);
    }
    if o.genome.connections.len() > max_rows {
        line(
            &format!("… {} more", o.genome.connections.len() - max_rows),
            14.0,
            faint,
            &mut y,
        );
    }
}
