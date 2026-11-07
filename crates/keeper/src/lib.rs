//! Clave keeper: the member daemon that holds one viewing share per vault and never lets it out. It answers threshold
//! ECDH requests with DLEQ-proven partials (for its owner's engine and for other members), runs the DKG and reshare
//! ceremonies, keeps the proposal store, and activates or destroys shares as the vault's on-chain committee changes.
//! See `architecture/keeper.md`.

pub mod ceremony;
mod clock;
pub mod committee;
pub mod config;
pub mod custody;
mod error;
pub mod events;
mod keeper;
mod limits;
pub mod mailbox;
mod random;
pub mod rpc;
pub mod store;
pub mod transport;

pub use clock::{Clock, SystemClock};
pub use error::{Error, Result};
pub use keeper::{
    CeremonyInfo, DkgResult, Keeper, KeeperConfig, KeeperParts, LobbyInfo, LobbyOpen,
    LobbyPlanInfo, LobbyRole, LobbyStatus, MemberPartials, MemberReachability, NewMember,
    Observation, ReshareRequest, ReshareResult, VaultInfo,
};
