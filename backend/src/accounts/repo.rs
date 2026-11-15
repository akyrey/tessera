//! SQL for users, households, passkeys, invites and sessions.
//!
//! Every function runs a single statement, so it accepts either the pool or
//! a transaction (`&mut *tx`).

use sqlx::SqliteExecutor;
use uuid::Uuid;

use super::model::{
    Invite, InviteKind, MemberDto, Membership, Role, StoredPasskey, User, UserSummary,
};

// ---------- users ----------

pub async fn count_users(ex: impl SqliteExecutor<'_>) -> sqlx::Result<i64> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(ex)
        .await
}

pub async fn insert_user(ex: impl SqliteExecutor<'_>, user: &User) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO users (id, display_name, is_admin, created_at) VALUES (?, ?, ?, ?)")
        .bind(user.id)
        .bind(&user.display_name)
        .bind(user.is_admin)
        .bind(user.created_at)
        .execute(ex)
        .await
        .map(|_| ())
}

pub async fn find_user(ex: impl SqliteExecutor<'_>, id: Uuid) -> sqlx::Result<Option<User>> {
    sqlx::query_as("SELECT id, display_name, is_admin, created_at FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(ex)
        .await
}

pub async fn list_users(ex: impl SqliteExecutor<'_>) -> sqlx::Result<Vec<UserSummary>> {
    sqlx::query_as(
        "SELECT u.id, u.display_name, u.is_admin, h.name AS household_name,
                (SELECT COUNT(*) FROM passkeys p WHERE p.user_id = u.id) AS passkey_count
         FROM users u
         LEFT JOIN household_members m ON m.user_id = u.id
         LEFT JOIN households h ON h.id = m.household_id
         ORDER BY u.created_at",
    )
    .fetch_all(ex)
    .await
}

// ---------- households ----------

pub async fn insert_household(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    name: &str,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO households (id, name, created_at) VALUES (?, ?, ?)")
        .bind(id)
        .bind(name)
        .bind(now)
        .execute(ex)
        .await
        .map(|_| ())
}

pub async fn add_member(
    ex: impl SqliteExecutor<'_>,
    household_id: Uuid,
    user_id: Uuid,
    role: Role,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO household_members (household_id, user_id, role, joined_at) VALUES (?, ?, ?, ?)",
    )
    .bind(household_id)
    .bind(user_id)
    .bind(role)
    .bind(now)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn membership_of(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<Option<Membership>> {
    sqlx::query_as(
        "SELECT m.household_id, h.name AS household_name, m.role
         FROM household_members m JOIN households h ON h.id = m.household_id
         WHERE m.user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(ex)
    .await
}

pub async fn members_of(
    ex: impl SqliteExecutor<'_>,
    household_id: Uuid,
) -> sqlx::Result<Vec<MemberDto>> {
    sqlx::query_as(
        "SELECT m.user_id, u.display_name, m.role, m.joined_at
         FROM household_members m JOIN users u ON u.id = m.user_id
         WHERE m.household_id = ?
         ORDER BY m.joined_at, u.display_name",
    )
    .bind(household_id)
    .fetch_all(ex)
    .await
}

pub async fn household_ids(ex: impl SqliteExecutor<'_>) -> sqlx::Result<Vec<Uuid>> {
    sqlx::query_scalar("SELECT id FROM households ORDER BY created_at")
        .fetch_all(ex)
        .await
}

pub async fn household_name(ex: impl SqliteExecutor<'_>, id: Uuid) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT name FROM households WHERE id = ?")
        .bind(id)
        .fetch_optional(ex)
        .await
}

// ---------- passkeys ----------

pub struct NewPasskey<'a> {
    pub id: Uuid,
    pub user_id: Uuid,
    pub credential_id: &'a [u8],
    pub passkey_json: &'a str,
    pub name: &'a str,
    pub created_at: i64,
}

pub async fn insert_passkey(
    ex: impl SqliteExecutor<'_>,
    passkey: &NewPasskey<'_>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO passkeys (id, user_id, credential_id, passkey, name, created_at)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(passkey.id)
    .bind(passkey.user_id)
    .bind(passkey.credential_id)
    .bind(passkey.passkey_json)
    .bind(passkey.name)
    .bind(passkey.created_at)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn passkeys_of(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<Vec<StoredPasskey>> {
    sqlx::query_as(
        "SELECT id, user_id, passkey, name, created_at, last_used_at
         FROM passkeys WHERE user_id = ? ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(ex)
    .await
}

pub async fn find_passkey_by_credential(
    ex: impl SqliteExecutor<'_>,
    credential_id: &[u8],
) -> sqlx::Result<Option<StoredPasskey>> {
    sqlx::query_as(
        "SELECT id, user_id, passkey, name, created_at, last_used_at
         FROM passkeys WHERE credential_id = ?",
    )
    .bind(credential_id)
    .fetch_optional(ex)
    .await
}

pub async fn credential_ids_of(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
) -> sqlx::Result<Vec<Vec<u8>>> {
    sqlx::query_scalar("SELECT credential_id FROM passkeys WHERE user_id = ?")
        .bind(user_id)
        .fetch_all(ex)
        .await
}

/// Records a successful login; `passkey_json` is set when the signature counter
/// or backup state changed and the stored credential must be updated.
pub async fn touch_passkey(
    ex: impl SqliteExecutor<'_>,
    id: Uuid,
    passkey_json: Option<&str>,
    now: i64,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE passkeys SET last_used_at = ?, passkey = COALESCE(?, passkey) WHERE id = ?")
        .bind(now)
        .bind(passkey_json)
        .bind(id)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Deletes one of the user's passkeys unless it is their last one.
pub async fn delete_passkey_unless_last(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    passkey_id: Uuid,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "DELETE FROM passkeys
         WHERE id = ? AND user_id = ?
           AND (SELECT COUNT(*) FROM passkeys WHERE user_id = ?) > 1",
    )
    .bind(passkey_id)
    .bind(user_id)
    .bind(user_id)
    .execute(ex)
    .await?;
    Ok(result.rows_affected() == 1)
}

// ---------- invites ----------

pub struct NewInvite<'a> {
    pub id: Uuid,
    pub token_hash: &'a [u8],
    pub kind: InviteKind,
    pub household_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub created_at: i64,
    pub expires_at: i64,
}

pub async fn insert_invite(
    ex: impl SqliteExecutor<'_>,
    invite: &NewInvite<'_>,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO invites
            (id, token_hash, kind, household_id, user_id, created_by, created_at, expires_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(invite.id)
    .bind(invite.token_hash)
    .bind(invite.kind)
    .bind(invite.household_id)
    .bind(invite.user_id)
    .bind(invite.created_by)
    .bind(invite.created_at)
    .bind(invite.expires_at)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn find_usable_invite(
    ex: impl SqliteExecutor<'_>,
    token_hash: &[u8],
    now: i64,
) -> sqlx::Result<Option<Invite>> {
    sqlx::query_as(
        "SELECT id, kind, household_id, user_id, expires_at FROM invites
         WHERE token_hash = ? AND used_at IS NULL AND expires_at > ?",
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(ex)
    .await
}

/// Marks the invite used; `false` if it was already used or has expired meanwhile.
pub async fn consume_invite(ex: impl SqliteExecutor<'_>, id: Uuid, now: i64) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE invites SET used_at = ? WHERE id = ? AND used_at IS NULL AND expires_at > ?",
    )
    .bind(now)
    .bind(id)
    .bind(now)
    .execute(ex)
    .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn delete_unused_invites_of_kind(
    ex: impl SqliteExecutor<'_>,
    kind: InviteKind,
) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM invites WHERE kind = ? AND used_at IS NULL")
        .bind(kind)
        .execute(ex)
        .await
        .map(|r| r.rows_affected())
}

pub async fn delete_expired_invites(ex: impl SqliteExecutor<'_>, now: i64) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM invites WHERE expires_at <= ?")
        .bind(now)
        .execute(ex)
        .await
        .map(|r| r.rows_affected())
}

// ---------- sessions ----------

pub async fn insert_session(
    ex: impl SqliteExecutor<'_>,
    token_hash: &[u8],
    user_id: Uuid,
    now: i64,
    expires_at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, created_at, expires_at) VALUES (?, ?, ?, ?)",
    )
    .bind(token_hash)
    .bind(user_id)
    .bind(now)
    .bind(expires_at)
    .execute(ex)
    .await
    .map(|_| ())
}

pub async fn find_session_user(
    ex: impl SqliteExecutor<'_>,
    token_hash: &[u8],
    now: i64,
) -> sqlx::Result<Option<User>> {
    sqlx::query_as(
        "SELECT u.id, u.display_name, u.is_admin, u.created_at
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ? AND s.expires_at > ?",
    )
    .bind(token_hash)
    .bind(now)
    .fetch_optional(ex)
    .await
}

pub async fn delete_session(ex: impl SqliteExecutor<'_>, token_hash: &[u8]) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token_hash)
        .execute(ex)
        .await
        .map(|_| ())
}

/// Ends every session of `user_id` except the one hashed as `keep`.
pub async fn delete_sessions_of(
    ex: impl SqliteExecutor<'_>,
    user_id: Uuid,
    keep: Option<&[u8]>,
) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ? AND token_hash IS NOT ?")
        .bind(user_id)
        .bind(keep)
        .execute(ex)
        .await
        .map(|r| r.rows_affected())
}

pub async fn delete_expired_sessions(ex: impl SqliteExecutor<'_>, now: i64) -> sqlx::Result<u64> {
    sqlx::query("DELETE FROM sessions WHERE expires_at <= ?")
        .bind(now)
        .execute(ex)
        .await
        .map(|r| r.rows_affected())
}
