//! `clave-demo`: a narrated two-member Clave demo in one terminal. Alice and Bob each get a real shell in a pane;
//! the narration bar says what happens next, Enter runs it, and while a command runs your keys go to its terminal
//! (passphrases, confirmations). `--headless` runs the same story without a screen, as a smoke test.

mod pane;
mod runner;
mod script;
mod ui;

use std::{
    io::Write,
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

use clap::Parser;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::{
    pane::Pane,
    runner::{Runner, State, Tools},
    script::Who,
    ui::Input,
};

#[derive(Parser)]
#[command(
    name = "clave-demo",
    about = "A narrated two-member Clave demo: Alice and Bob side by side"
)]
struct Args {
    /// Run each step by itself after this many seconds (Enter still runs it at once).
    #[arg(long, value_name = "SECONDS")]
    auto: Option<u64>,
    /// No screen: run the whole story with a passphrase file and --yes, logging each terminal to the demo folder.
    #[arg(long)]
    headless: bool,
    /// Leave Alice's and Bob's daemons running on exit.
    #[arg(long)]
    keep: bool,
    /// Aztec node.
    #[arg(long, env = "AZTEC_NODE_URL", default_value = "http://localhost:8080")]
    node_url: String,
}

/// Paths of the built tools, from this crate's location in the repository.
struct Paths {
    root: PathBuf,
    bin: PathBuf,
    demo: PathBuf,
}

impl Paths {
    fn new() -> Self {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = root.canonicalize().unwrap_or(root);
        let bin = root.join("crates/target/debug");
        let demo = std::env::temp_dir().join("clave-demo");
        Self { root, bin, demo }
    }

    fn clave(&self) -> PathBuf {
        self.bin.join("clave")
    }
}

fn preflight(paths: &Paths, node_url: &str) -> Result<(), String> {
    for (path, fix) in [
        (
            paths.clave(),
            "cargo build -p clave-cli -p clave-keeper (in crates/)",
        ),
        (
            paths.bin.join("clave-keeper"),
            "cargo build -p clave-keeper (in crates/)",
        ),
        (
            paths.root.join("packages/engine/dist/claved.js"),
            "pnpm --filter @clave/engine... build",
        ),
    ] {
        if !path.exists() {
            return Err(format!("missing {}; build it: {fix}", path.display()));
        }
    }
    let authority = node_url
        .split("://")
        .nth(1)
        .unwrap_or(node_url)
        .split('/')
        .next()
        .unwrap_or_default();
    let reachable = authority
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok());
    if reachable {
        Ok(())
    } else {
        Err(format!(
            "no Aztec node at {node_url}; start one: aztec start --local-network"
        ))
    }
}

fn env(paths: &Paths, args: &Args) -> Vec<(String, String)> {
    let path = format!(
        "{}:{}",
        paths.bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let mut env = vec![
        ("PATH".to_owned(), path),
        (
            "CLAVE_HOME".to_owned(),
            paths.demo.join("home").display().to_string(),
        ),
        (
            "CLAVE_ENGINE".to_owned(),
            paths
                .root
                .join("packages/engine/dist/claved.js")
                .display()
                .to_string(),
        ),
        (
            "CLAVE_KEEPER_BIN".to_owned(),
            paths.bin.join("clave-keeper").display().to_string(),
        ),
        ("AZTEC_NODE_URL".to_owned(), args.node_url.clone()),
    ];
    if args.headless {
        env.push((
            "CLAVE_PASSPHRASE_FILE".to_owned(),
            paths.demo.join("passphrase").display().to_string(),
        ));
        env.push(("NO_COLOR".to_owned(), "1".to_owned()));
    }
    env
}

/// Stops both members' daemons in the demo home.
fn stop_daemons(paths: &Paths, env: &[(String, String)]) {
    for profile in ["alice", "bob"] {
        let _ = Command::new(paths.clave())
            .args(["-q", "-P", profile, "daemon", "stop"])
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output();
    }
}

/// A fresh demo folder: daemons from a previous run stopped, old profiles removed.
fn fresh(paths: &Paths, env: &[(String, String)], headless: bool) -> Result<(), String> {
    stop_daemons(paths, env);
    let _ = std::fs::remove_dir_all(&paths.demo);
    std::fs::create_dir_all(&paths.demo).map_err(|e| e.to_string())?;
    if headless {
        std::fs::write(
            paths.demo.join("passphrase"),
            "correct horse battery staple",
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn start(paths: &Paths, args: &Args) -> Result<Runner, String> {
    preflight(paths, &args.node_url)?;
    let env = env(paths, args);
    fresh(paths, &env, args.headless)?;
    let log = |name: &str| {
        args.headless
            .then(|| paths.demo.join(format!("{name}.log")))
    };
    let size = (40, 100);
    let alice = Pane::spawn("Alice", Who::Alice.profile(), &env, size, log("alice"))?;
    let bob = Pane::spawn("Bob", Who::Bob.profile(), &env, size, log("bob"))?;
    let tools = Tools {
        clave: paths.clave(),
        env,
    };
    Ok(Runner::new(
        [alice, bob],
        script::story(),
        tools,
        args.headless,
    ))
}

fn interactive(runner: &mut Runner, auto: Option<u64>) -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    let mut ready_since: Option<Instant> = None;
    let result = loop {
        let now = Instant::now();
        runner.tick(now);
        let countdown = match (auto, &runner.state) {
            (Some(gap), State::Ready) if runner.shells_ready() => {
                let since = *ready_since.get_or_insert(now);
                let left = gap.saturating_sub(now.duration_since(since).as_secs());
                if left == 0 {
                    runner.go();
                    ready_since = None;
                }
                Some(left)
            }
            _ => {
                ready_since = None;
                None
            }
        };
        if let Err(e) = terminal.draw(|frame| ui::draw(frame, runner, countdown)) {
            break Err(e);
        }
        if !event::poll(Duration::from_millis(16))? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match ui::input(runner, key) {
            Input::Quit => break Ok(()),
            Input::Run | Input::Retry => runner.go(),
            Input::Skip => runner.skip(),
            Input::SwitchFocus => {
                runner.focus = match runner.focus {
                    Who::Alice => Who::Bob,
                    Who::Bob => Who::Alice,
                };
            }
            Input::Forward(bytes) => {
                let target = runner.busy_pane().unwrap_or(runner.focus);
                runner.pane(target).send(&bytes);
            }
            Input::Ignore => {}
        }
    };
    ratatui::restore();
    result
}

fn headless(runner: &mut Runner, demo: &Path) -> Result<(), String> {
    let mut out = std::io::stdout().lock();
    let deadline = Instant::now() + Duration::from_mins(15);
    let mut announced = usize::MAX;
    loop {
        runner.tick(Instant::now());
        match runner.state.clone() {
            State::Ready if runner.shells_ready() => {
                if announced != runner.step {
                    announced = runner.step;
                    let title = runner.steps.get(runner.step).map_or("", |s| s.title);
                    let _ = writeln!(out, "[{}/{}] {title}", runner.step + 1, runner.steps.len());
                }
                runner.go();
            }
            State::Failed(reason) => {
                return Err(format!("{reason}; terminal logs in {}", demo.display()));
            }
            State::Finished => return Ok(()),
            _ => {}
        }
        if Instant::now() > deadline {
            return Err("timed out".to_owned());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn main() -> ExitCode {
    let args = Args::parse();
    let paths = Paths::new();
    let mut runner = match start(&paths, &args) {
        Ok(runner) => runner,
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "✗ {error}");
            return ExitCode::FAILURE;
        }
    };
    let result = if args.headless {
        headless(&mut runner, &paths.demo)
    } else {
        interactive(&mut runner, args.auto).map_err(|e| e.to_string())
    };
    if !args.keep {
        stop_daemons(&paths, &runner_env(&runner));
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(std::io::stderr(), "✗ {error}");
            ExitCode::FAILURE
        }
    }
}

fn runner_env(runner: &Runner) -> Vec<(String, String)> {
    runner.tools_env()
}
