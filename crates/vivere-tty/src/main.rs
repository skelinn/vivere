//! The terminal window onto a world. Like the graphical viewer and the GIF
//! renderer, it is a pure observer: it steps the same `World::step`,
//! consumes no simulation randomness, and changes nothing. Organisms are
//! truecolor dots, food is the green dust they graze, and a biting organism
//! flashes red. Runs over SSH, in tmux, wherever a terminal lives.

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use crossterm::{cursor, execute, terminal};
use std::io::{self, BufWriter, Write};
use std::time::{Duration, Instant};
use vivere_core::{Config, World};

const FRAME: Duration = Duration::from_millis(33);
const HELP: &str = " space pause   . step   +/- speed   q quit ";

struct Args {
    seed: u64,
    resume: Option<String>,
    /// Sim ticks per second (best effort).
    tps: u32,
    /// Render exactly one frame to stdout and exit (no raw mode) — used for
    /// tests, pipes, and screenshots.
    once: bool,
}

fn usage() -> ! {
    eprintln!("usage: vivere-tty [--seed N | --resume file.snap] [--tps N] [--once]");
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut args = Args {
        seed: 42,
        resume: None,
        tps: 60,
        once: false,
    };
    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        if argv[i] == "--once" {
            args.once = true;
            i += 1;
            continue;
        }
        let value = argv.get(i + 1).cloned().unwrap_or_else(|| usage());
        match argv[i].as_str() {
            "--seed" => args.seed = value.parse().unwrap_or_else(|_| usage()),
            "--resume" => args.resume = Some(value),
            "--tps" => {
                args.tps = value
                    .parse::<u32>()
                    .unwrap_or_else(|_| usage())
                    .clamp(1, 100_000)
            }
            _ => usage(),
        }
        i += 2;
    }
    args
}

fn load_world(args: &Args) -> World {
    match &args.resume {
        Some(path) => {
            let bytes = std::fs::read(path).unwrap_or_else(|e| {
                eprintln!("error reading {path}: {e}");
                std::process::exit(1);
            });
            World::from_snapshot_bytes(&bytes).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                std::process::exit(1);
            })
        }
        None => World::new(Config::default(), args.seed),
    }
}

fn hue_color(hue: f32) -> Color {
    let h = hue.rem_euclid(1.0) * 6.0;
    let i = h.floor();
    let f = h - i;
    let (s, v) = (0.72, 0.95);
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
    Color::Rgb {
        r: (r * 255.0) as u8,
        g: (g * 255.0) as u8,
        b: (b * 255.0) as u8,
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Cell {
    glyph: char,
    color: Color,
}

const EMPTY: Cell = Cell {
    glyph: ' ',
    color: Color::Reset,
};

/// Rasterize the world into a character grid: `rows` lines of `cols` cells.
fn rasterize(world: &World, cols: u16, rows: u16) -> Vec<Cell> {
    let (cols, rows) = (cols as usize, rows.max(1) as usize);
    let mut grid = vec![EMPTY; cols * rows];
    let sx = cols as f32 / world.cfg.world.width;
    let sy = rows as f32 / world.cfg.world.height;
    let mut plot = |x: f32, y: f32, cell: Cell| {
        let cx = ((x * sx) as usize).min(cols - 1);
        let cy = ((y * sy) as usize).min(rows - 1);
        grid[cy * cols + cx] = cell;
    };
    for d in &world.detritus {
        plot(
            d.x,
            d.y,
            Cell {
                glyph: ',',
                color: Color::Rgb {
                    r: 115,
                    g: 97,
                    b: 77,
                },
            },
        );
    }
    for f in &world.food {
        plot(
            f.x,
            f.y,
            Cell {
                glyph: '·',
                color: Color::Rgb {
                    r: 72,
                    g: 200,
                    b: 108,
                },
            },
        );
    }
    for o in &world.organisms {
        let glyph = match o.genome.body.size {
            s if s < 1.2 => '•',
            s if s < 2.5 => '●',
            _ => '@',
        };
        let color = if o.last_bite > 0.05 {
            Color::Rgb {
                r: 255,
                g: 74,
                b: 60,
            }
        } else {
            hue_color(o.genome.body.hue)
        };
        plot(o.x, o.y, Cell { glyph, color });
    }
    grid
}

fn draw(out: &mut impl Write, world: &World, tps: u32, paused: bool) -> io::Result<()> {
    let (cols, rows) = terminal::size().unwrap_or((100, 30));
    let world_rows = rows.saturating_sub(2).max(1);
    let grid = rasterize(world, cols, world_rows);

    crossterm::queue!(out, cursor::MoveTo(0, 0), ResetColor)?;
    let hud = format!(
        " vivere  tick {}  pop {}  food {}  births {}  kills {}  {} tps{}",
        world.tick,
        world.organisms.len(),
        world.food.len(),
        world.births,
        world.deaths_predation,
        tps,
        if paused { "  PAUSED" } else { "" },
    );
    let mut line = hud;
    line.truncate(cols as usize);
    write!(out, "{line:<width$}", width = cols as usize)?;

    let mut current = Color::Reset;
    for row in 0..world_rows {
        crossterm::queue!(out, cursor::MoveTo(0, row + 1))?;
        for col in 0..cols {
            let cell = grid[row as usize * cols as usize + col as usize];
            if cell.glyph == ' ' {
                write!(out, " ")?;
                continue;
            }
            if cell.color != current {
                crossterm::queue!(out, SetForegroundColor(cell.color))?;
                current = cell.color;
            }
            write!(out, "{}", cell.glyph)?;
        }
    }
    crossterm::queue!(out, cursor::MoveTo(0, rows - 1), ResetColor)?;
    let mut help = HELP.to_string();
    help.truncate(cols as usize);
    write!(out, "{help:<width$}", width = cols as usize)?;
    out.flush()
}

fn run(world: &mut World, tps_start: u32) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout());
    let mut tps = tps_start;
    let mut paused = false;
    let mut carry = 0.0f64;
    loop {
        let frame_start = Instant::now();
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char(' ') => paused = !paused,
                    KeyCode::Char('.') => {
                        if paused {
                            world.step();
                        }
                    }
                    KeyCode::Char('+') | KeyCode::Char('=') => tps = (tps * 2).min(100_000),
                    KeyCode::Char('-') => tps = (tps / 2).max(1),
                    _ => {}
                }
            }
        }
        if !paused {
            carry += f64::from(tps) * FRAME.as_secs_f64();
            let mut budget = 0;
            while carry >= 1.0 && budget < 200_000 {
                world.step();
                carry -= 1.0;
                budget += 1;
                if frame_start.elapsed() > Duration::from_millis(25) {
                    carry = 0.0;
                    break;
                }
            }
        }
        draw(&mut out, world, tps, paused)?;
        let elapsed = frame_start.elapsed();
        if elapsed < FRAME {
            std::thread::sleep(FRAME - elapsed);
        }
    }
}

fn main() -> io::Result<()> {
    let args = parse_args();
    let mut world = load_world(&args);

    if args.once {
        let mut out = BufWriter::new(io::stdout());
        world.step();
        draw(&mut out, &world, args.tps, false)?;
        writeln!(out)?;
        return Ok(());
    }

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;
    let result = run(&mut world, args.tps);
    execute!(
        stdout,
        cursor::Show,
        terminal::LeaveAlternateScreen,
        ResetColor
    )?;
    terminal::disable_raw_mode()?;
    result
}
