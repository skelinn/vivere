//! Headless GIF renderer: steps a world and rasterizes frames with a tiny
//! software renderer — no window, no GPU, reproducible anywhere. Like the
//! viewer, it is a window onto the world: it consumes no simulation
//! randomness and changes nothing.

use gif::{Encoder, Frame, Repeat};
use std::borrow::Cow;
use std::fs::File;
use vivere_core::{Config, World};

const FRAME_DELAY: u16 = 5; // hundredths of a second → 20 fps

// Global palette layout (256 indexed colors):
// 0..=7    background, dark → sunlit
// 8        food
// 9        detritus
// 16..=255 organism hues
const LIGHT_LEVELS: usize = 8;
const FOOD_IDX: u8 = 8;
const DETRITUS_IDX: u8 = 9;
const BITE_IDX: u8 = 10;
const HUE_BASE: usize = 16;
const HUE_COUNT: usize = 240;

struct Args {
    seed: u64,
    resume: Option<String>,
    ticks: u64,
    every: u64,
    scale: f32,
    out: String,
}

fn usage() -> ! {
    eprintln!(
        "usage: vivere-gif [--seed N | --resume file.snap] [--ticks N] [--every N] \
         [--scale F] [--out file.gif]"
    );
    std::process::exit(2);
}

fn parse_args() -> Args {
    let mut args = Args {
        seed: 42,
        resume: None,
        ticks: 4000,
        every: 8,
        scale: 0.5,
        out: "docs/assets/protocell.gif".to_string(),
    };
    let argv: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < argv.len() {
        let value = argv.get(i + 1).cloned().unwrap_or_else(|| usage());
        match argv[i].as_str() {
            "--seed" => args.seed = value.parse().unwrap_or_else(|_| usage()),
            "--resume" => args.resume = Some(value),
            "--ticks" => args.ticks = value.parse().unwrap_or_else(|_| usage()),
            "--every" => args.every = value.parse::<u64>().unwrap_or_else(|_| usage()).max(1),
            "--scale" => args.scale = value.parse().unwrap_or_else(|_| usage()),
            "--out" => args.out = value,
            _ => usage(),
        }
        i += 2;
    }
    args
}

fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let h = h.rem_euclid(1.0) * 6.0;
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
    [(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8]
}

fn build_palette() -> Vec<u8> {
    let mut pal = vec![0u8; 256 * 3];
    let set = |pal: &mut Vec<u8>, i: usize, c: [u8; 3]| {
        pal[i * 3] = c[0];
        pal[i * 3 + 1] = c[1];
        pal[i * 3 + 2] = c[2];
    };
    for l in 0..LIGHT_LEVELS {
        let t = l as f32 / (LIGHT_LEVELS - 1) as f32;
        set(
            &mut pal,
            l,
            [
                (11.0 + 52.0 * t) as u8,
                (14.0 + 44.0 * t) as u8,
                (18.0 + 16.0 * t) as u8,
            ],
        );
    }
    set(&mut pal, FOOD_IDX as usize, [72, 200, 108]);
    set(&mut pal, DETRITUS_IDX as usize, [115, 97, 77]);
    set(&mut pal, BITE_IDX as usize, [255, 74, 60]);
    for i in 0..HUE_COUNT {
        let c = hsv(i as f32 / HUE_COUNT as f32, 0.72, 0.95);
        set(&mut pal, HUE_BASE + i, c);
    }
    pal
}

/// Static background: the light field quantized to the palette's dark ramp.
fn build_background(world: &World, w: usize, h: usize, scale: f32) -> Vec<u8> {
    let (ww, wh) = (world.cfg.world.width, world.cfg.world.height);
    let max = world.light.max_value();
    let mut bg = vec![0u8; w * h];
    for py in 0..h {
        for px in 0..w {
            let l = world
                .light
                .sample(px as f32 / scale, py as f32 / scale, ww, wh)
                / max;
            bg[py * w + px] =
                ((l * (LIGHT_LEVELS - 1) as f32) as usize).min(LIGHT_LEVELS - 1) as u8;
        }
    }
    bg
}

fn dot(buf: &mut [u8], w: i32, h: i32, x: f32, y: f32, r: i32, idx: u8) {
    let (cx, cy) = (x as i32, y as i32);
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                let px = (cx + dx).rem_euclid(w);
                let py = (cy + dy).rem_euclid(h);
                buf[(py * w + px) as usize] = idx;
            }
        }
    }
}

fn render(world: &World, bg: &[u8], w: i32, h: i32, scale: f32) -> Vec<u8> {
    let mut buf = bg.to_vec();
    for d in &world.detritus {
        dot(&mut buf, w, h, d.x * scale, d.y * scale, 1, DETRITUS_IDX);
    }
    for f in &world.food {
        dot(&mut buf, w, h, f.x * scale, f.y * scale, 1, FOOD_IDX);
    }
    for o in &world.organisms {
        let r = ((o.radius(&world.cfg) * scale) as i32).max(1);
        let hue_idx = HUE_BASE as u8
            + ((o.genome.body.hue * HUE_COUNT as f32) as usize).min(HUE_COUNT - 1) as u8;
        // A biting organism flashes a red rim — the tool steps the world
        // itself, so the transient bite effort is live here.
        if o.last_bite > 0.05 {
            dot(&mut buf, w, h, o.x * scale, o.y * scale, r + 1, BITE_IDX);
        }
        dot(&mut buf, w, h, o.x * scale, o.y * scale, r, hue_idx);
    }
    buf
}

fn main() {
    let args = parse_args();
    let mut world = match &args.resume {
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
    };

    let scale = args.scale;
    let (w, h) = (
        (world.cfg.world.width * scale) as u16,
        (world.cfg.world.height * scale) as u16,
    );
    let bg = build_background(&world, w as usize, h as usize, scale);

    let file = File::create(&args.out).unwrap_or_else(|e| {
        eprintln!("error creating {}: {e}", args.out);
        std::process::exit(1);
    });
    let palette = build_palette();
    let mut encoder = Encoder::new(file, w, h, &palette).expect("gif encoder");
    encoder.set_repeat(Repeat::Infinite).expect("gif repeat");

    let start = world.tick;
    let mut frames = 0u32;
    while world.tick - start <= args.ticks {
        if (world.tick - start) % args.every == 0 {
            let buf = render(&world, &bg, w as i32, h as i32, scale);
            let mut frame = Frame {
                width: w,
                height: h,
                buffer: Cow::Owned(buf),
                delay: FRAME_DELAY,
                ..Frame::default()
            };
            frame.make_lzw_pre_encoded();
            encoder.write_lzw_pre_encoded_frame(&frame).expect("frame");
            frames += 1;
        }
        world.step();
    }
    eprintln!(
        "wrote {} frames ({}x{}, ticks {}..{}) to {}",
        frames, w, h, start, world.tick, args.out
    );
}
