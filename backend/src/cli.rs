//! Command line: `tessera [serve]` runs the server, `tessera admin …` manages accounts.
//!
//! In the cluster: `kubectl exec deploy/tessera -- /tessera admin users`.

use std::io::Write;

use clap::{Parser, Subcommand};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::accounts::{repo, service};
use crate::clock::now_unix;
use crate::config::Config;
use crate::error::ApiError;

#[derive(Debug, Parser)]
#[command(name = "tessera", version, about = "Self-hosted loyalty-card wallet")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run the HTTP server (default).
    Serve,
    /// Manage users and invite links.
    #[command(subcommand)]
    Admin(AdminCommand),
}

#[derive(Debug, Subcommand)]
pub enum AdminCommand {
    /// List users with their household and passkey count.
    Users,
    /// Print a link to join a household (the first-admin link while there are no users).
    Invite {
        /// Household to join; defaults to the only household.
        #[arg(long)]
        household: Option<Uuid>,
    },
    /// Print a link that lets a user register a new passkey after losing theirs.
    Recover {
        /// User id, as shown by `tessera admin users`.
        user_id: Uuid,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum AdminError {
    #[error("{0}")]
    Usage(String),
    #[error(transparent)]
    Api(#[from] ApiError),
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub async fn run_admin(
    command: AdminCommand,
    config: &Config,
    pool: &SqlitePool,
    out: &mut impl Write,
) -> Result<(), AdminError> {
    let now = now_unix();
    match command {
        AdminCommand::Users => print_users(pool, out).await,
        AdminCommand::Invite { household } => {
            if let Some(invite) = service::ensure_bootstrap_invite(pool, None, now).await? {
                writeln!(out, "No users yet. First-admin link (valid 24h):")?;
                return print_link(config, &invite, out);
            }
            let household_id = resolve_household(pool, household).await?;
            let invite = service::issue_household_invite(pool, household_id, None, now).await?;
            writeln!(out, "Household invite (valid 48h, single use):")?;
            print_link(config, &invite, out)
        }
        AdminCommand::Recover { user_id } => {
            let invite = service::create_recovery_invite(pool, user_id, now)
                .await
                .map_err(|e| match e {
                    ApiError::NotFound => AdminError::Usage(format!("no user with id {user_id}")),
                    other => other.into(),
                })?;
            writeln!(out, "Recovery link (valid 24h, single use):")?;
            print_link(config, &invite, out)
        }
    }
}

async fn print_users(pool: &SqlitePool, out: &mut impl Write) -> Result<(), AdminError> {
    let users = repo::list_users(pool).await?;
    if users.is_empty() {
        writeln!(
            out,
            "No users yet. Run `tessera admin invite` for the first-admin link."
        )?;
        return Ok(());
    }
    for user in users {
        writeln!(
            out,
            "{}  {:<20} {:<6} household={:<12} passkeys={}",
            user.id,
            user.display_name,
            if user.is_admin { "admin" } else { "" },
            user.household_name.as_deref().unwrap_or("-"),
            user.passkey_count
        )?;
    }
    Ok(())
}

async fn resolve_household(pool: &SqlitePool, requested: Option<Uuid>) -> Result<Uuid, AdminError> {
    let households = repo::household_ids(pool).await?;
    match requested {
        Some(id) if households.contains(&id) => Ok(id),
        Some(id) => Err(AdminError::Usage(format!("no household with id {id}"))),
        None => match households.as_slice() {
            [only] => Ok(*only),
            _ => Err(AdminError::Usage(
                "several households exist; pass --household <id>".into(),
            )),
        },
    }
}

fn print_link(
    config: &Config,
    invite: &service::IssuedInvite,
    out: &mut impl Write,
) -> Result<(), AdminError> {
    writeln!(
        out,
        "{}/join#{}",
        config.public_origin(),
        invite.token.value
    )?;
    Ok(())
}
