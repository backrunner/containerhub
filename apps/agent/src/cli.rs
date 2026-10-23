use clap::{Parser, Subcommand};
use std::{net::SocketAddr, path::PathBuf};
#[derive(Parser)]
#[command(
    name = "containerhub-agent",
    about = "ContainerHub 远程 Agent、密文中继和控制工具"
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub(crate) enum Command {
    /// Explicitly initialize an Agent. Existing or damaged installations are never reset here.
    Init {
        #[arg(long)]
        state: PathBuf,
        #[arg(long)]
        app_issuer: PathBuf,
    },
    /// Run a user-scoped Apple container Agent. Expose external endpoints through a WSS reverse proxy.
    Serve {
        #[arg(long)]
        state: PathBuf,
        #[arg(long, default_value = "127.0.0.1:7443")]
        listen: SocketAddr,
        #[arg(long)]
        apple_binary: Option<PathBuf>,
        #[arg(long, conflicts_with = "apple_binary")]
        docker_context: Option<String>,
        #[arg(long, requires = "docker_context")]
        docker_binary: Option<PathBuf>,
        #[arg(long)]
        relay: Option<String>,
    },
    /// Forward opaque encrypted frames. Use a TLS reverse proxy for public deployment.
    Relay {
        #[arg(long, default_value = "127.0.0.1:7444")]
        listen: SocketAddr,
    },
    /// Create an owner and independent controller key. macOS private keys go to Keychain.
    Identity {
        #[arg(long)]
        profile: PathBuf,
    },
    /// Explicitly authorize new image/container management actions for this existing Native owner device.
    EnableManagement {
        #[arg(long)]
        profile: PathBuf,
    },
    /// Import a credential issued by a separately trusted application-enrollment service.
    ImportCredential {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        credential: PathBuf,
    },
    /// Claim once or reconnect to a pinned Agent. The pin is saved after a verified receipt.
    Connect {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        url: String,
        #[arg(long)]
        relay_agent: Option<String>,
        #[arg(long)]
        agent: Option<String>,
        #[arg(long)]
        claim: bool,
    },
    /// Read resources from a previously bound Agent.
    Snapshot {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        agent: Option<String>,
    },
    /// Revoke a device and push the signed policy to all known Agents. Offline failures are reported.
    Revoke {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        device: String,
    },
    /// Authorize an independently generated device public key without exporting the owner private key.
    Grant {
        #[arg(long)]
        profile: PathBuf,
        #[arg(long)]
        device_public: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        allow_lifecycle: bool,
        #[arg(long)]
        allow_management: bool,
    },
}
