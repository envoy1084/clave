//! `tecpan-keeper`: run a member's keeper.
//!
//! The supervising app (the desktop) launches `tecpan-keeper run --secrets-stdin` and writes one JSON line with the
//! master key (kept in the OS keychain) and a fresh RPC token. A keeper on a member's own server uses a passphrase
//! (`TECPAN_KEEPER_PASSPHRASE`) instead. When ready, `run` prints one JSON line on stdout with its identity, transport
//! key, addresses and RPC socket; logs go to stderr.

use std::{
    io::{BufRead, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};

use clap::{Parser, Subcommand, ValueEnum};
use serde::Deserialize;
use serde_json::json;
use tecpan_keeper::{
    Error, Keeper, KeeperParts, Result, SystemClock,
    config::FileConfig,
    custody::{KeeperKeys, MasterKey, SecretStore, create_private_dir, write_private},
    rpc::{self, RpcToken},
    store::Store,
    transport::TcpTransport,
};
use zeroize::Zeroizing;

#[derive(Parser)]
#[command(
    name = "tecpan-keeper",
    version,
    about = "Tecpan keeper: custody of a member's viewing shares"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Creates the data directory and the keeper's keys, and prints its public identity.
    Init(Common),
    /// Prints the keeper's public identity and transport key.
    Identity(Common),
    /// Runs the keeper.
    Run(Run),
}

#[derive(clap::Args)]
struct Common {
    /// Data directory.
    #[arg(long, env = "TECPAN_KEEPER_DATA")]
    data_dir: PathBuf,
    /// Read `{"masterKey": "<hex>", "rpcToken": "<token>"}` from the first line of stdin.
    #[arg(long)]
    secrets_stdin: bool,
}

#[derive(clap::Args)]
struct Run {
    #[command(flatten)]
    common: Common,
    /// RPC socket path (default `<data>/keeper.sock`).
    #[arg(long)]
    rpc_socket: Option<PathBuf>,
    /// Peer listen address.
    #[arg(long)]
    listen: Option<SocketAddr>,
    /// Address advertised to other members.
    #[arg(long)]
    advertise: Option<String>,
    /// Log format.
    #[arg(long, value_enum, default_value_t = LogFormat::Text)]
    log_format: LogFormat,
}

#[derive(Clone, Copy, ValueEnum)]
enum LogFormat {
    Text,
    Json,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaunchSecrets {
    master_key: String,
    rpc_token: Option<String>,
}

struct Unlocked {
    secrets: SecretStore,
    token: Option<Zeroizing<String>>,
}

fn unlock(common: &Common) -> Result<Unlocked> {
    let data = &common.data_dir;
    create_private_dir(data)?;
    let (key, token) = if common.secrets_stdin {
        let mut line = Zeroizing::new(String::new());
        std::io::stdin().lock().read_line(&mut line)?;
        let launch: LaunchSecrets = serde_json::from_str(&line)
            .map_err(|_| Error::Invalid("stdin: expected launch secrets JSON".into()))?;
        (
            MasterKey::from_hex(&launch.master_key)?,
            launch.rpc_token.map(Zeroizing::new),
        )
    } else {
        let passphrase =
            Zeroizing::new(std::env::var("TECPAN_KEEPER_PASSPHRASE").map_err(|_| {
                Error::Invalid("set TECPAN_KEEPER_PASSPHRASE or use --secrets-stdin".into())
            })?);
        (
            MasterKey::from_passphrase(passphrase.as_bytes(), &salt(data)?)?,
            None,
        )
    };
    Ok(Unlocked {
        secrets: SecretStore::open(data.join("secrets"), &key)?,
        token,
    })
}

fn salt(data: &Path) -> Result<[u8; 16]> {
    let path = data.join("salt");
    if let Ok(bytes) = std::fs::read(&path) {
        return bytes
            .try_into()
            .map_err(|_| Error::DataDir("salt file is corrupt".into()));
    }
    let mut salt = [0u8; 16];
    getrandom::fill(&mut salt).map_err(|e| Error::Invalid(e.to_string()))?;
    write_private(&path, &salt)?;
    Ok(salt)
}

fn print(value: &serde_json::Value) -> Result<()> {
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "{value}")?;
    stdout.flush()?;
    Ok(())
}

fn identity_json(keys: &KeeperKeys) -> serde_json::Value {
    json!({
        "identity": format!("0x{}", hex::encode(keys.identity.public().0)),
        "transport": format!("0x{}", hex::encode(keys.transport_public.0)),
    })
}

async fn run(args: Run) -> Result<()> {
    let data = args.common.data_dir.clone();
    let file = FileConfig::load(&data.join("keeper.toml"))?;
    let unlocked = unlock(&args.common)?;
    let keys = KeeperKeys::load_or_create(&unlocked.secrets)?;
    let store = Store::open(&data.join("keeper.db"))?;
    let listen = args
        .listen
        .or(file.listen)
        .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 0)));
    let (transport, inbound) = TcpTransport::bind(listen, &keys.transport).await?;
    let advertise = args
        .advertise
        .or_else(|| file.advertise.clone())
        .unwrap_or_else(|| format!("tcp://{}", transport.local_addr()));
    let socket = args
        .rpc_socket
        .or_else(|| file.rpc_socket.as_ref().map(PathBuf::from))
        .unwrap_or_else(|| data.join("keeper.sock"));
    let token = if let Some(token) = unlocked.token {
        token
    } else {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| Error::Invalid(e.to_string()))?;
        let token = Zeroizing::new(hex::encode(bytes));
        write_private(&data.join("rpc.token"), token.as_bytes())?;
        token
    };
    let public = identity_json(&keys);
    let keeper = Keeper::start(KeeperParts {
        keys,
        secrets: unlocked.secrets,
        store,
        transport: Box::new(Arc::clone(&transport)),
        inbound,
        clock: Arc::new(SystemClock),
        config: file.keeper_config(),
        addresses: vec![advertise.clone()],
        mailbox: None,
    })?;
    let mut ready = public;
    ready["addresses"] = json!([advertise]);
    ready["rpcSocket"] = json!(socket.display().to_string());
    write_private(&data.join("keeper.json"), ready.to_string().as_bytes())?;

    let server = tokio::spawn(rpc::serve(keeper, socket.clone(), RpcToken::new(&token)));
    // `serve` binds before accepting; give it a moment, then announce readiness.
    tokio::task::yield_now().await;
    while !socket_ready(&socket) {
        if server.is_finished() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    print(&json!({ "ready": ready }))?;
    tracing::info!("keeper running");
    tokio::select! {
        result = server => result.map_err(|e| Error::Invalid(e.to_string()))??,
        () = shutdown() => tracing::info!("shutting down"),
    }
    let _ = std::fs::remove_file(&socket);
    Ok(())
}

#[cfg(unix)]
fn socket_ready(path: &Path) -> bool {
    path.exists()
}

#[cfg(not(unix))]
fn socket_ready(_: &Path) -> bool {
    true
}

async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut term) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = term.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

fn init_logging(format: LogFormat) {
    let filter =
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into());
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr);
    match format {
        LogFormat::Text => builder.init(),
        LogFormat::Json => builder.json().init(),
    }
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Init(common) | Command::Identity(common) => {
            init_logging(LogFormat::Text);
            unlock(&common)
                .and_then(|unlocked| KeeperKeys::load_or_create(&unlocked.secrets))
                .and_then(|keys| print(&identity_json(&keys)))
        }
        Command::Run(args) => {
            init_logging(args.log_format);
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(Error::from)
                .and_then(|runtime| runtime.block_on(run(args)))
        }
    };
    match result {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "tecpan-keeper failed");
            std::process::ExitCode::FAILURE
        }
    }
}
