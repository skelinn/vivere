use clap::{Parser, Subcommand};
use std::fs;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use vivere_core::{Config, Metrics, World};

#[derive(Parser)]
#[command(name = "vivere", version, about = "vivere: headless world runner")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a world and log metrics to CSV.
    Run {
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Ticks to simulate (additional ticks, when resuming).
        #[arg(long, default_value_t = 100_000)]
        ticks: u64,
        /// Metrics CSV path. Omit to skip logging.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Log metrics every N ticks.
        #[arg(long, default_value_t = 100)]
        metrics_every: u64,
        /// TOML config (see `vivere default-config`). Ignored when resuming:
        /// a snapshot carries its own physics.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Write the final world state here.
        #[arg(long)]
        save_snapshot: Option<PathBuf>,
        /// Resume from a snapshot instead of creating a fresh world.
        #[arg(long)]
        resume: Option<PathBuf>,
        /// Flip the contact-physics channel on the loaded world (an explicit
        /// experimenter action, announced on stderr; for staged experiments
        /// like quarantine-then-teeth).
        #[arg(long)]
        override_contact: Option<bool>,
    },
    /// Print the default config as TOML (save, edit, pass back via --config).
    DefaultConfig,
    /// Convert a v0.1 snapshot into the current format: same world, same
    /// environment, v0.2 physics. The generation-600 grazers meet the new
    /// costs — and, if --contact true, eventually the teeth.
    ImportV01 {
        input: PathBuf,
        output: PathBuf,
        /// Whether the imported world has the contact channel enabled.
        #[arg(long, default_value_t = false)]
        contact: bool,
    },
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.cmd {
        Cmd::DefaultConfig => {
            let toml = toml::to_string_pretty(&Config::default())
                .map_err(|e| format!("serializing default config: {e}"))?;
            println!("{toml}");
            Ok(())
        }
        Cmd::ImportV01 {
            input,
            output,
            contact,
        } => {
            let bytes =
                fs::read(&input).map_err(|e| format!("reading {}: {e}", input.display()))?;
            let world = vivere_cli::legacy_v01::import_v01(&bytes, contact)?;
            fs::write(&output, world.snapshot_bytes())
                .map_err(|e| format!("writing {}: {e}", output.display()))?;
            eprintln!(
                "imported: tick {}  population {}  contact {}  -> {}",
                world.tick,
                world.organisms.len(),
                if contact { "ON" } else { "off (quarantined)" },
                output.display()
            );
            Ok(())
        }
        Cmd::Run {
            seed,
            ticks,
            out,
            metrics_every,
            config,
            save_snapshot,
            resume,
            override_contact,
        } => {
            let mut world = match &resume {
                Some(path) => {
                    let bytes =
                        fs::read(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
                    let w = World::from_snapshot_bytes(&bytes)?;
                    eprintln!(
                        "resumed snapshot: tick {}, population {}",
                        w.tick,
                        w.organisms.len()
                    );
                    w
                }
                None => {
                    let cfg = match &config {
                        Some(path) => {
                            let text = fs::read_to_string(path)
                                .map_err(|e| format!("reading {}: {e}", path.display()))?;
                            toml::from_str(&text)
                                .map_err(|e| format!("parsing {}: {e}", path.display()))?
                        }
                        None => Config::default(),
                    };
                    World::new(cfg, seed)
                }
            };
            if let Some(enabled) = override_contact {
                eprintln!(
                    "override: contact physics {} (was {})",
                    if enabled { "ENABLED" } else { "DISABLED" },
                    world.cfg.contact.enabled
                );
                world.cfg.contact.enabled = enabled;
            }

            let mut csv = match &out {
                Some(path) => {
                    if let Some(dir) = path.parent()
                        && !dir.as_os_str().is_empty()
                    {
                        fs::create_dir_all(dir)
                            .map_err(|e| format!("creating {}: {e}", dir.display()))?;
                    }
                    let file = fs::File::create(path)
                        .map_err(|e| format!("creating {}: {e}", path.display()))?;
                    let mut w = BufWriter::new(file);
                    writeln!(w, "{}", Metrics::csv_header()).map_err(|e| e.to_string())?;
                    Some(w)
                }
                None => None,
            };

            let metrics_every = metrics_every.max(1);
            let end = world.tick + ticks;
            let mut prev: Option<Metrics> = None;
            let mut extinct_announced = false;

            let log = |world: &World,
                       prev: &mut Option<Metrics>,
                       csv: &mut Option<BufWriter<fs::File>>|
             -> Result<(), String> {
                let m = world.sample_metrics();
                if let Some(w) = csv {
                    writeln!(w, "{}", m.csv_row(prev.as_ref())).map_err(|e| e.to_string())?;
                }
                *prev = Some(m);
                Ok(())
            };

            log(&world, &mut prev, &mut csv)?;
            while world.tick < end {
                world.step();
                if world.tick % metrics_every == 0 || world.tick == end {
                    log(&world, &mut prev, &mut csv)?;
                }
                if world.tick % 5000 == 0 {
                    eprintln!(
                        "tick {:>8}  pop {:>6}  food {:>6}  births {:>7}",
                        world.tick,
                        world.organisms.len(),
                        world.food.len(),
                        world.births
                    );
                }
                if world.organisms.is_empty() && !extinct_announced {
                    eprintln!("extinct at tick {}", world.tick);
                    extinct_announced = true;
                }
            }
            if let Some(w) = &mut csv {
                w.flush().map_err(|e| e.to_string())?;
            }

            let m = world.sample_metrics();
            eprintln!(
                "done: tick {}  population {}  births {}  deaths {} (starve) / {} (age)  \
                 world energy {:.1}  conservation drift {:.3e}",
                m.tick,
                m.population,
                m.births_total,
                m.deaths_starve_total,
                m.deaths_age_total,
                m.world_energy,
                m.drift
            );

            if let Some(path) = save_snapshot {
                if let Some(dir) = path.parent()
                    && !dir.as_os_str().is_empty()
                {
                    fs::create_dir_all(dir)
                        .map_err(|e| format!("creating {}: {e}", dir.display()))?;
                }
                fs::write(&path, world.snapshot_bytes())
                    .map_err(|e| format!("writing {}: {e}", path.display()))?;
                eprintln!("snapshot saved to {}", path.display());
            }
            Ok(())
        }
    }
}
