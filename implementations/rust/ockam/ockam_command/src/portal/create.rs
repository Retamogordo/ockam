use crate::node::NodeOpts;
use crate::util::{api, connect_to, stop_node};
use crate::util::{ComposableSnippet, Operation, PortalMode, Protocol};
use crate::CommandGlobalOpts;
use clap::{Args, Subcommand};
use ockam::{Context, Route};
use ockam_api::error::ApiError;
use ockam_api::{
    nodes::models::portal::{InletStatus, OutletStatus},
    nodes::NODEMANAGER_ADDR,
    Status,
};
use ockam_core::Address;
use ockam_multiaddr::MultiAddr;

#[derive(Clone, Debug, Args)]
pub struct CreateCommand {
    #[clap(flatten)]
    node_opts: NodeOpts,

    /// Select a creation variant
    #[clap(subcommand)]
    pub create_subcommand: CreateTypeCommand,

    /// Give this portal endpoint a name.  If none is provided a
    /// random one will be generated.
    pub alias: Option<String>,
}

impl From<&'_ CreateCommand> for ComposableSnippet {
    fn from(cc: &'_ CreateCommand) -> Self {
        let bind = cc.create_subcommand.bind();
        let peer = cc.create_subcommand.peer();
        let mode = cc.create_subcommand.mode();

        Self {
            id: format!("_portal_{}_{}_{}_{}", mode, "tcp", bind, peer,),
            op: Operation::Portal {
                mode,
                protocol: Protocol::Tcp,
                bind,
                peer,
            },
            params: vec![],
        }
    }
}

#[derive(Clone, Debug, Subcommand)]
pub enum CreateTypeCommand {
    /// Create a TCP portal inlet
    TcpInlet {
        /// Portal inlet bind address
        bind: String,
        /// Forwarding point for the portal (ockam routing address)
        outlet_addr: MultiAddr,
    },
    /// Create a TCP portal outlet
    TcpOutlet {
        /// Portal outlet connection address
        tcp_address: String,
        /// Portal outlet worker address
        worker_address: Address,
    },
}

impl CreateTypeCommand {
    fn mode(&self) -> PortalMode {
        match self {
            Self::TcpInlet { .. } => PortalMode::Inlet,
            Self::TcpOutlet { .. } => PortalMode::Outlet,
        }
    }

    fn bind(&self) -> String {
        match self {
            Self::TcpInlet { bind, .. } => bind.clone(),
            Self::TcpOutlet { worker_address, .. } => worker_address.to_string(),
        }
    }

    fn peer(&self) -> String {
        match self {
            Self::TcpInlet { outlet_addr, .. } => outlet_addr.to_string(),
            Self::TcpOutlet { tcp_address, .. } => tcp_address.clone(),
        }
    }
}

impl CreateCommand {
    pub fn run(opts: CommandGlobalOpts, command: CreateCommand) {
        let cfg = &opts.config;
        let port = match cfg.select_node(&command.node_opts.api_node) {
            Some(cfg) => cfg.port,
            None => {
                eprintln!("No such node available.  Run `ockam node list` to list available nodes");
                std::process::exit(-1);
            }
        };

        let composite = (&command).into();
        let node = command.node_opts.api_node.clone();

        match command.create_subcommand {
            CreateTypeCommand::TcpInlet { .. } => connect_to(port, command, create_inlet),
            CreateTypeCommand::TcpOutlet { .. } => connect_to(port, command, create_outlet),
        }

        // Update the startup config
        let startup_cfg = match cfg.get_launch_config(&node) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("failed to load startup configuration: {}", e);
                std::process::exit(-1);
            }
        };

        startup_cfg.add_composite(composite);
        if let Err(e) = startup_cfg.atomic_update().run() {
            eprintln!("failed to update configuration: {}", e);
            std::process::exit(-1);
        }
    }
}

pub async fn create_inlet(
    ctx: Context,
    cmd: CreateCommand,
    mut base_route: Route,
) -> anyhow::Result<()> {
    let (bind, outlet_addr) = match cmd.create_subcommand {
        CreateTypeCommand::TcpInlet { bind, outlet_addr } => (bind, outlet_addr),
        CreateTypeCommand::TcpOutlet { .. } => {
            return Err(ApiError::generic("Internal logic error").into())
        }
    };

    let resp: Vec<u8> = ctx
        .send_and_receive(
            base_route.modify().append(NODEMANAGER_ADDR),
            api::create_inlet(&bind, &outlet_addr, &cmd.alias)?,
        )
        .await?;

    let (
        response,
        InletStatus {
            bind_addr, alias, ..
        },
    ) = api::parse_inlet_status(&resp)?;

    match response.status() {
        Some(Status::Ok) => {
            eprintln!(
                "Portal inlet '{}' created! You can send messages to it on this tcp address: \n{}`",
                alias, bind_addr
            )
        }

        _ => eprintln!("An unknown error occurred while creating an inlet..."),
    }

    stop_node(ctx).await
}

pub async fn create_outlet(
    ctx: Context,
    cmd: CreateCommand,
    mut base_route: Route,
) -> anyhow::Result<()> {
    let (tcp_address, worker_address) = match cmd.create_subcommand {
        CreateTypeCommand::TcpInlet { .. } => {
            return Err(ApiError::generic("Internal logic error").into())
        }
        CreateTypeCommand::TcpOutlet {
            tcp_address,
            worker_address,
        } => (tcp_address, worker_address),
    };

    let resp: Vec<u8> = ctx
        .send_and_receive(
            base_route.modify().append(NODEMANAGER_ADDR),
            api::create_outlet(&tcp_address, worker_address.to_string(), &cmd.alias)?,
        )
        .await?;

    let (
        response,
        OutletStatus {
            worker_addr, alias, ..
        },
    ) = api::parse_outlet_status(&resp)?;

    match response.status() {
        Some(Status::Ok) => {
            eprintln!(
                "Portal outlet '{}' created! You can send messages through it via this address:\n{}",
                alias,
                worker_addr
            );
        }

        _ => eprintln!("An unknown error occurred while creating an outlet..."),
    }

    stop_node(ctx).await
}
