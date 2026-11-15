//! Account rules: invites, registration, household membership.
//!
//! Functions take `now` (Unix seconds) explicitly so expiry is testable.

use sqlx::SqlitePool;
use uuid::Uuid;

use super::model::{HouseholdDto, InviteKind, InvitePreview, Me, Role, User, UserDto};
use super::repo::{self, NewInvite, NewPasskey};
use crate::error::ApiError;
use crate::tokens::{self, SecretToken};

pub const BOOTSTRAP_INVITE_TTL: i64 = 24 * 60 * 60;
/// Short enough that a forgotten or leaked link stops working soon.
pub const HOUSEHOLD_INVITE_TTL: i64 = 2 * 24 * 60 * 60;
pub const RECOVERY_INVITE_TTL: i64 = 24 * 60 * 60;
pub const DEFAULT_HOUSEHOLD_NAME: &str = "Home";
pub const DEFAULT_PASSKEY_NAME: &str = "Passkey";
const MAX_NAME_CHARS: usize = 64;

/// Who is registering, decided when the ceremony starts and re-checked at the end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRegistration {
    pub invite_id: Uuid,
    pub kind: InviteKind,
    pub user_id: Uuid,
    pub display_name: String,
    pub household_id: Option<Uuid>,
}

pub struct NewCredential<'a> {
    pub credential_id: &'a [u8],
    pub passkey_json: &'a str,
    pub name: &'a str,
}

pub struct IssuedInvite {
    pub token: SecretToken,
    pub expires_at: i64,
}

/// Trims and validates a user-visible name (display name, passkey name).
pub fn validate_name(field: &str, raw: &str) -> Result<String, ApiError> {
    let name = raw.trim();
    let length = name.chars().count();
    if length == 0 || length > MAX_NAME_CHARS {
        return Err(ApiError::Validation(format!(
            "{field} must be between 1 and {MAX_NAME_CHARS} characters"
        )));
    }
    if name.chars().any(char::is_control) {
        return Err(ApiError::Validation(format!(
            "{field} contains invalid characters"
        )));
    }
    Ok(name.to_owned())
}

// ---------- invites ----------

/// While no user exists, keeps exactly one unused bootstrap invite and returns it.
/// `configured` (TESSERA_BOOTSTRAP_TOKEN) makes the link predictable for automation.
pub async fn ensure_bootstrap_invite(
    pool: &SqlitePool,
    configured: Option<&str>,
    now: i64,
) -> Result<Option<IssuedInvite>, ApiError> {
    let mut tx = pool.begin().await?;
    repo::delete_unused_invites_of_kind(&mut *tx, InviteKind::Bootstrap).await?;
    if repo::count_users(&mut *tx).await? > 0 {
        tx.commit().await?;
        return Ok(None);
    }
    let token = configured.map_or_else(SecretToken::generate, |value| {
        SecretToken::from_value(value.to_owned())
    });
    let expires_at = now + BOOTSTRAP_INVITE_TTL;
    insert_invite(
        &mut tx,
        &token,
        InviteKind::Bootstrap,
        None,
        None,
        None,
        now,
        expires_at,
    )
    .await?;
    tx.commit().await?;
    Ok(Some(IssuedInvite { token, expires_at }))
}

/// Household owners invite new members.
pub async fn create_household_invite(
    pool: &SqlitePool,
    actor: &User,
    now: i64,
) -> Result<IssuedInvite, ApiError> {
    let membership = repo::membership_of(pool, actor.id)
        .await?
        .ok_or(ApiError::Forbidden)?;
    if membership.role != Role::Owner {
        return Err(ApiError::Forbidden);
    }
    issue_household_invite(pool, membership.household_id, Some(actor.id), now).await
}

/// Admin CLI: invite into a household without an acting user.
pub async fn issue_household_invite(
    pool: &SqlitePool,
    household_id: Uuid,
    created_by: Option<Uuid>,
    now: i64,
) -> Result<IssuedInvite, ApiError> {
    let token = SecretToken::generate();
    let expires_at = now + HOUSEHOLD_INVITE_TTL;
    let mut tx = pool.begin().await?;
    insert_invite(
        &mut tx,
        &token,
        InviteKind::Household,
        Some(household_id),
        None,
        created_by,
        now,
        expires_at,
    )
    .await?;
    tx.commit().await?;
    Ok(IssuedInvite { token, expires_at })
}

/// Admin CLI: lets a user who lost every passkey register a new one.
pub async fn create_recovery_invite(
    pool: &SqlitePool,
    user_id: Uuid,
    now: i64,
) -> Result<IssuedInvite, ApiError> {
    repo::find_user(pool, user_id)
        .await?
        .ok_or(ApiError::NotFound)?;
    let token = SecretToken::generate();
    let expires_at = now + RECOVERY_INVITE_TTL;
    let mut tx = pool.begin().await?;
    insert_invite(
        &mut tx,
        &token,
        InviteKind::Recovery,
        None,
        Some(user_id),
        None,
        now,
        expires_at,
    )
    .await?;
    tx.commit().await?;
    Ok(IssuedInvite { token, expires_at })
}

#[allow(clippy::too_many_arguments)]
async fn insert_invite(
    tx: &mut sqlx::SqliteConnection,
    token: &SecretToken,
    kind: InviteKind,
    household_id: Option<Uuid>,
    user_id: Option<Uuid>,
    created_by: Option<Uuid>,
    now: i64,
    expires_at: i64,
) -> sqlx::Result<()> {
    repo::insert_invite(
        &mut *tx,
        &NewInvite {
            id: Uuid::new_v4(),
            token_hash: &token.hash,
            kind,
            household_id,
            user_id,
            created_by,
            created_at: now,
            expires_at,
        },
    )
    .await
}

/// What the invite page shows before the user commits to registering.
pub async fn preview_invite(
    pool: &SqlitePool,
    token: &str,
    now: i64,
) -> Result<InvitePreview, ApiError> {
    let invite = usable_invite(pool, token, now).await?;
    let (household_name, display_name) = match invite.kind {
        InviteKind::Bootstrap => (None, None),
        InviteKind::Household => {
            let id = invite.household_id.ok_or(ApiError::InvalidInvite)?;
            (repo::household_name(pool, id).await?, None)
        }
        InviteKind::Recovery => {
            let id = invite.user_id.ok_or(ApiError::InvalidInvite)?;
            let user = repo::find_user(pool, id)
                .await?
                .ok_or(ApiError::InvalidInvite)?;
            (None, Some(user.display_name))
        }
    };
    Ok(InvitePreview {
        kind: invite.kind,
        household_name,
        display_name,
    })
}

async fn usable_invite(
    pool: &SqlitePool,
    token: &str,
    now: i64,
) -> Result<super::model::Invite, ApiError> {
    if !tokens::looks_valid(token) {
        return Err(ApiError::InvalidInvite);
    }
    let invite = repo::find_usable_invite(pool, &tokens::hash(token), now)
        .await?
        .ok_or(ApiError::InvalidInvite)?;
    if invite.kind == InviteKind::Bootstrap && repo::count_users(pool).await? > 0 {
        return Err(ApiError::InvalidInvite);
    }
    Ok(invite)
}

// ---------- registration ----------

/// Validates the invite and decides which account the new passkey belongs to.
pub async fn plan_registration(
    pool: &SqlitePool,
    token: &str,
    display_name: Option<&str>,
    now: i64,
) -> Result<PendingRegistration, ApiError> {
    let invite = usable_invite(pool, token, now).await?;
    let (user_id, display_name) = match invite.kind {
        InviteKind::Bootstrap | InviteKind::Household => {
            let name = validate_name("display name", display_name.unwrap_or_default())?;
            (Uuid::new_v4(), name)
        }
        InviteKind::Recovery => {
            let id = invite.user_id.ok_or(ApiError::InvalidInvite)?;
            let user = repo::find_user(pool, id)
                .await?
                .ok_or(ApiError::InvalidInvite)?;
            (user.id, user.display_name)
        }
    };
    Ok(PendingRegistration {
        invite_id: invite.id,
        kind: invite.kind,
        user_id,
        display_name,
        household_id: invite.household_id,
    })
}

/// Consumes the invite and stores the account and passkey atomically.
pub async fn complete_registration(
    pool: &SqlitePool,
    pending: &PendingRegistration,
    credential: &NewCredential<'_>,
    now: i64,
) -> Result<User, ApiError> {
    let mut tx = pool.begin().await?;
    if !repo::consume_invite(&mut *tx, pending.invite_id, now).await? {
        return Err(ApiError::InvalidInvite);
    }

    let user = match pending.kind {
        InviteKind::Bootstrap => {
            if repo::count_users(&mut *tx).await? > 0 {
                return Err(ApiError::InvalidInvite);
            }
            let user = new_user(pending, true, now);
            let household_id = Uuid::new_v4();
            repo::insert_user(&mut *tx, &user).await?;
            repo::insert_household(&mut *tx, household_id, DEFAULT_HOUSEHOLD_NAME, now).await?;
            repo::add_member(&mut *tx, household_id, user.id, Role::Owner, now).await?;
            user
        }
        InviteKind::Household => {
            let household_id = pending.household_id.ok_or(ApiError::InvalidInvite)?;
            let user = new_user(pending, false, now);
            repo::insert_user(&mut *tx, &user).await?;
            repo::add_member(&mut *tx, household_id, user.id, Role::Member, now).await?;
            user
        }
        InviteKind::Recovery => {
            let user = repo::find_user(&mut *tx, pending.user_id)
                .await?
                .ok_or(ApiError::InvalidInvite)?;
            // Recovery usually means a lost or stolen device: sign it out.
            repo::delete_sessions_of(&mut *tx, user.id, None).await?;
            user
        }
    };

    store_credential(&mut tx, user.id, credential, now).await?;
    tx.commit().await?;
    Ok(user)
}

/// Adds another passkey (e.g. a second phone) to a signed-in user.
pub async fn add_passkey(
    pool: &SqlitePool,
    user_id: Uuid,
    credential: &NewCredential<'_>,
    now: i64,
) -> Result<Uuid, ApiError> {
    let mut tx = pool.begin().await?;
    let id = store_credential(&mut tx, user_id, credential, now).await?;
    tx.commit().await?;
    Ok(id)
}

async fn store_credential(
    tx: &mut sqlx::SqliteConnection,
    user_id: Uuid,
    credential: &NewCredential<'_>,
    now: i64,
) -> Result<Uuid, ApiError> {
    let id = Uuid::new_v4();
    let result = repo::insert_passkey(
        &mut *tx,
        &NewPasskey {
            id,
            user_id,
            credential_id: credential.credential_id,
            passkey_json: credential.passkey_json,
            name: credential.name,
            created_at: now,
        },
    )
    .await;
    match result {
        Ok(()) => Ok(id),
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => {
            Err(ApiError::Conflict("this passkey is already registered"))
        }
        Err(e) => Err(e.into()),
    }
}

fn new_user(pending: &PendingRegistration, is_admin: bool, now: i64) -> User {
    User {
        id: pending.user_id,
        display_name: pending.display_name.clone(),
        is_admin,
        created_at: now,
    }
}

// ---------- profile ----------

pub async fn load_me(pool: &SqlitePool, user: &User) -> Result<Me, ApiError> {
    let membership = repo::membership_of(pool, user.id)
        .await?
        .ok_or_else(|| ApiError::Internal(format!("user {} has no household", user.id)))?;
    let members = repo::members_of(pool, membership.household_id).await?;
    Ok(Me {
        user: UserDto::from(user),
        household: HouseholdDto {
            id: membership.household_id,
            name: membership.household_name,
            role: membership.role,
            members,
        },
    })
}

/// Removes a passkey and signs out every other device, since the removed
/// passkey may belong to a lost phone that still holds a session.
pub async fn delete_passkey(
    pool: &SqlitePool,
    user_id: Uuid,
    passkey_id: Uuid,
    current_session: Option<&[u8]>,
) -> Result<(), ApiError> {
    let mut tx = pool.begin().await?;
    if repo::delete_passkey_unless_last(&mut *tx, user_id, passkey_id).await? {
        repo::delete_sessions_of(&mut *tx, user_id, current_session).await?;
        tx.commit().await?;
        return Ok(());
    }
    drop(tx);
    let owned = repo::passkeys_of(pool, user_id)
        .await?
        .iter()
        .any(|p| p.id == passkey_id);
    if owned {
        Err(ApiError::Conflict("you cannot remove your last passkey"))
    } else {
        Err(ApiError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::sessions;
    use crate::db;

    const NOW: i64 = 1_800_000_000;

    async fn pool() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("t.db").display());
        (db::connect(&url).await.unwrap(), dir)
    }

    fn credential(id: &[u8]) -> NewCredential<'_> {
        NewCredential {
            credential_id: id,
            passkey_json: "{}",
            name: "iPhone",
        }
    }

    async fn bootstrap_admin(pool: &SqlitePool) -> User {
        let invite = ensure_bootstrap_invite(pool, None, NOW)
            .await
            .unwrap()
            .unwrap();
        let pending = plan_registration(pool, &invite.token.value, Some("Alice"), NOW)
            .await
            .unwrap();
        complete_registration(pool, &pending, &credential(b"cred-alice"), NOW)
            .await
            .unwrap()
    }

    async fn join_household(pool: &SqlitePool, owner: &User, name: &str, cred: &[u8]) -> User {
        let invite = create_household_invite(pool, owner, NOW).await.unwrap();
        let pending = plan_registration(pool, &invite.token.value, Some(name), NOW)
            .await
            .unwrap();
        complete_registration(pool, &pending, &credential(cred), NOW)
            .await
            .unwrap()
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(validate_name("name", "  Alice ").unwrap(), "Alice");
        assert!(validate_name("name", "   ").is_err());
        assert!(validate_name("name", &"x".repeat(65)).is_err());
        assert!(validate_name("name", "Al\u{0}ice").is_err());
        assert_eq!(
            validate_name("name", &"è".repeat(64))
                .unwrap()
                .chars()
                .count(),
            64
        );
    }

    #[tokio::test]
    async fn bootstrap_creates_admin_owner_of_new_household() {
        let (pool, _dir) = pool().await;

        let admin = bootstrap_admin(&pool).await;
        let me = load_me(&pool, &admin).await.unwrap();

        assert!(admin.is_admin);
        assert_eq!(me.household.name, DEFAULT_HOUSEHOLD_NAME);
        assert_eq!(me.household.role, Role::Owner);
        assert_eq!(me.household.members.len(), 1);
    }

    #[tokio::test]
    async fn bootstrap_invite_is_not_issued_once_users_exist() {
        let (pool, _dir) = pool().await;
        bootstrap_admin(&pool).await;

        assert!(
            ensure_bootstrap_invite(&pool, None, NOW)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn reissuing_bootstrap_invalidates_the_previous_link() {
        let (pool, _dir) = pool().await;
        let first = ensure_bootstrap_invite(&pool, None, NOW)
            .await
            .unwrap()
            .unwrap();
        let second = ensure_bootstrap_invite(&pool, None, NOW)
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(
            preview_invite(&pool, &first.token.value, NOW).await,
            Err(ApiError::InvalidInvite)
        ));
        assert!(
            preview_invite(&pool, &second.token.value, NOW)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn configured_bootstrap_token_is_used_verbatim() {
        let (pool, _dir) = pool().await;
        let token = "A".repeat(tokens::ENCODED_LEN);

        ensure_bootstrap_invite(&pool, Some(&token), NOW)
            .await
            .unwrap();

        let preview = preview_invite(&pool, &token, NOW).await.unwrap();
        assert_eq!(preview.kind, InviteKind::Bootstrap);
    }

    #[tokio::test]
    async fn invite_can_only_be_used_once() {
        let (pool, _dir) = pool().await;
        let invite = ensure_bootstrap_invite(&pool, None, NOW)
            .await
            .unwrap()
            .unwrap();
        let first = plan_registration(&pool, &invite.token.value, Some("Alice"), NOW)
            .await
            .unwrap();
        let second = plan_registration(&pool, &invite.token.value, Some("Mallory"), NOW)
            .await
            .unwrap();

        complete_registration(&pool, &first, &credential(b"a"), NOW)
            .await
            .unwrap();
        let result = complete_registration(&pool, &second, &credential(b"m"), NOW).await;

        assert!(matches!(result, Err(ApiError::InvalidInvite)));
        assert_eq!(repo::count_users(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn expired_invite_is_rejected() {
        let (pool, _dir) = pool().await;
        let invite = ensure_bootstrap_invite(&pool, None, NOW)
            .await
            .unwrap()
            .unwrap();
        let later = invite.expires_at;

        assert!(matches!(
            plan_registration(&pool, &invite.token.value, Some("Alice"), later).await,
            Err(ApiError::InvalidInvite)
        ));
    }

    #[tokio::test]
    async fn invite_expiring_between_start_and_finish_is_rejected() {
        let (pool, _dir) = pool().await;
        let invite = ensure_bootstrap_invite(&pool, None, NOW)
            .await
            .unwrap()
            .unwrap();
        let pending = plan_registration(&pool, &invite.token.value, Some("Alice"), NOW)
            .await
            .unwrap();

        let result =
            complete_registration(&pool, &pending, &credential(b"a"), invite.expires_at).await;
        assert!(matches!(result, Err(ApiError::InvalidInvite)));
    }

    #[tokio::test]
    async fn malformed_or_unknown_tokens_are_invalid_invites() {
        let (pool, _dir) = pool().await;
        for token in ["", "not-a-token", &"B".repeat(tokens::ENCODED_LEN)] {
            assert!(matches!(
                preview_invite(&pool, token, NOW).await,
                Err(ApiError::InvalidInvite)
            ));
        }
    }

    #[tokio::test]
    async fn display_name_is_required_for_new_accounts() {
        let (pool, _dir) = pool().await;
        let invite = ensure_bootstrap_invite(&pool, None, NOW)
            .await
            .unwrap()
            .unwrap();

        assert!(matches!(
            plan_registration(&pool, &invite.token.value, None, NOW).await,
            Err(ApiError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn household_invite_adds_a_member_to_the_owner_household() {
        let (pool, _dir) = pool().await;
        let owner = bootstrap_admin(&pool).await;
        let invite = create_household_invite(&pool, &owner, NOW).await.unwrap();

        let preview = preview_invite(&pool, &invite.token.value, NOW)
            .await
            .unwrap();
        assert_eq!(preview.kind, InviteKind::Household);
        assert_eq!(
            preview.household_name.as_deref(),
            Some(DEFAULT_HOUSEHOLD_NAME)
        );

        let pending = plan_registration(&pool, &invite.token.value, Some("Bea"), NOW)
            .await
            .unwrap();
        let bea = complete_registration(&pool, &pending, &credential(b"bea"), NOW)
            .await
            .unwrap();
        let me = load_me(&pool, &bea).await.unwrap();

        assert!(!bea.is_admin);
        assert_eq!(me.household.role, Role::Member);
        let names: Vec<_> = me
            .household
            .members
            .iter()
            .map(|m| m.display_name.as_str())
            .collect();
        assert_eq!(names, ["Alice", "Bea"]);
        assert_eq!(invite.expires_at, NOW + HOUSEHOLD_INVITE_TTL);
    }

    #[tokio::test]
    async fn members_cannot_invite() {
        let (pool, _dir) = pool().await;
        let owner = bootstrap_admin(&pool).await;
        let member = join_household(&pool, &owner, "Bea", b"bea").await;

        assert!(matches!(
            create_household_invite(&pool, &member, NOW).await,
            Err(ApiError::Forbidden)
        ));
    }

    #[tokio::test]
    async fn recovery_adds_a_passkey_to_the_existing_user() {
        let (pool, _dir) = pool().await;
        let admin = bootstrap_admin(&pool).await;
        let invite = create_recovery_invite(&pool, admin.id, NOW).await.unwrap();

        let preview = preview_invite(&pool, &invite.token.value, NOW)
            .await
            .unwrap();
        assert_eq!(preview.display_name.as_deref(), Some("Alice"));

        let pending = plan_registration(&pool, &invite.token.value, Some("ignored"), NOW)
            .await
            .unwrap();
        let recovered = complete_registration(&pool, &pending, &credential(b"new-phone"), NOW)
            .await
            .unwrap();

        assert_eq!(recovered.id, admin.id);
        assert_eq!(recovered.display_name, "Alice");
        assert_eq!(repo::passkeys_of(&pool, admin.id).await.unwrap().len(), 2);
        assert_eq!(repo::count_users(&pool).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn recovery_signs_out_existing_sessions() {
        let (pool, _dir) = pool().await;
        let admin = bootstrap_admin(&pool).await;
        let stolen = sessions::start_session(&pool, admin.id, 3600, NOW)
            .await
            .unwrap();
        let invite = create_recovery_invite(&pool, admin.id, NOW).await.unwrap();
        let pending = plan_registration(&pool, &invite.token.value, None, NOW)
            .await
            .unwrap();

        complete_registration(&pool, &pending, &credential(b"new-phone"), NOW)
            .await
            .unwrap();

        assert!(
            repo::find_session_user(&pool, &stolen.hash, NOW)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn removing_a_passkey_signs_out_other_devices_only() {
        let (pool, _dir) = pool().await;
        let admin = bootstrap_admin(&pool).await;
        let stolen_key = repo::passkeys_of(&pool, admin.id).await.unwrap()[0].id;
        add_passkey(&pool, admin.id, &credential(b"new-phone"), NOW)
            .await
            .unwrap();
        let stolen = sessions::start_session(&pool, admin.id, 3600, NOW)
            .await
            .unwrap();
        let current = sessions::start_session(&pool, admin.id, 3600, NOW)
            .await
            .unwrap();

        delete_passkey(&pool, admin.id, stolen_key, Some(&current.hash))
            .await
            .unwrap();

        assert!(
            repo::find_session_user(&pool, &stolen.hash, NOW)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            repo::find_session_user(&pool, &current.hash, NOW)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn recovery_for_unknown_user_is_not_found() {
        let (pool, _dir) = pool().await;
        assert!(matches!(
            create_recovery_invite(&pool, Uuid::new_v4(), NOW).await,
            Err(ApiError::NotFound)
        ));
    }

    #[tokio::test]
    async fn duplicate_credential_is_a_conflict_and_rolls_back() {
        let (pool, _dir) = pool().await;
        let owner = bootstrap_admin(&pool).await;
        let invite = create_household_invite(&pool, &owner, NOW).await.unwrap();
        let pending = plan_registration(&pool, &invite.token.value, Some("Bea"), NOW)
            .await
            .unwrap();

        let result = complete_registration(&pool, &pending, &credential(b"cred-alice"), NOW).await;

        assert!(matches!(result, Err(ApiError::Conflict(_))));
        assert_eq!(repo::count_users(&pool).await.unwrap(), 1);
        // The invite was not consumed because the transaction rolled back.
        assert!(
            preview_invite(&pool, &invite.token.value, NOW)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn last_passkey_cannot_be_deleted() {
        let (pool, _dir) = pool().await;
        let admin = bootstrap_admin(&pool).await;
        let only = repo::passkeys_of(&pool, admin.id).await.unwrap()[0].id;

        assert!(matches!(
            delete_passkey(&pool, admin.id, only, None).await,
            Err(ApiError::Conflict(_))
        ));

        add_passkey(&pool, admin.id, &credential(b"second"), NOW)
            .await
            .unwrap();
        delete_passkey(&pool, admin.id, only, None).await.unwrap();
        assert_eq!(repo::passkeys_of(&pool, admin.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn cannot_delete_someone_elses_passkey() {
        let (pool, _dir) = pool().await;
        let owner = bootstrap_admin(&pool).await;
        let bea = join_household(&pool, &owner, "Bea", b"bea").await;
        add_passkey(&pool, owner.id, &credential(b"second"), NOW)
            .await
            .unwrap();
        let alice_key = repo::passkeys_of(&pool, owner.id).await.unwrap()[0].id;

        assert!(matches!(
            delete_passkey(&pool, bea.id, alice_key, None).await,
            Err(ApiError::NotFound)
        ));
    }
}
