//! Account domain types and the DTOs exposed to the frontend.

use serde::{Deserialize, Serialize};
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type, TS)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Role {
    Owner,
    Member,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type, TS)]
#[sqlx(rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum InviteKind {
    Bootstrap,
    Household,
    Recovery,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub display_name: String,
    pub is_admin: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Invite {
    pub id: Uuid,
    pub kind: InviteKind,
    pub household_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub expires_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Membership {
    pub household_id: Uuid,
    pub household_name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct StoredPasskey {
    pub id: Uuid,
    pub user_id: Uuid,
    pub passkey: String,
    pub name: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct UserSummary {
    pub id: Uuid,
    pub display_name: String,
    pub is_admin: bool,
    pub household_name: Option<String>,
    pub passkey_count: i64,
}

// ---------- DTOs ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, sqlx::FromRow, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MemberDto {
    pub user_id: Uuid,
    pub display_name: String,
    pub role: Role,
    #[ts(type = "number")]
    pub joined_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HouseholdDto {
    pub id: Uuid,
    pub name: String,
    pub role: Role,
    pub members: Vec<MemberDto>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UserDto {
    pub id: Uuid,
    pub display_name: String,
    pub is_admin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Me {
    pub user: UserDto,
    pub household: HouseholdDto,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PasskeyDto {
    pub id: Uuid,
    pub name: String,
    #[ts(type = "number")]
    pub created_at: i64,
    #[ts(type = "number | null")]
    pub last_used_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InvitePreview {
    pub kind: InviteKind,
    /// Household being joined (household invites only).
    pub household_name: Option<String>,
    /// Account being recovered (recovery invites only).
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InviteCreated {
    /// Full link to share; the secret is in the fragment so it never reaches server logs.
    pub url: String,
    #[ts(type = "number")]
    pub expires_at: i64,
}

impl From<&User> for UserDto {
    fn from(user: &User) -> Self {
        Self {
            id: user.id,
            display_name: user.display_name.clone(),
            is_admin: user.is_admin,
        }
    }
}

impl From<StoredPasskey> for PasskeyDto {
    fn from(passkey: StoredPasskey) -> Self {
        Self {
            id: passkey.id,
            name: passkey.name,
            created_at: passkey.created_at,
            last_used_at: passkey.last_used_at,
        }
    }
}
