use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sourcebot_core::{
    claim_next_repository_sync_job, claim_next_review_agent_run, complete_review_agent_run,
    fail_review_agent_run, store_repository_sync_job as upsert_repository_sync_job, BootstrapStore,
    LocalSessionStore, OrganizationStore,
};
use sourcebot_models::RepositorySyncJobStatus;
use sourcebot_models::{
    ApiKey, BootstrapState, BootstrapStatus, LocalAccount, LocalSession, LocalSessionState,
    OAuthClient, Organization, OrganizationInvite, OrganizationMembership, OrganizationRole,
    OrganizationState, RepositoryPermissionBinding, RepositorySyncJob, ReviewAgentRun,
    ReviewAgentRunStatus,
};
use sqlx::{postgres::PgPoolOptions, Row};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

pub type DynBootstrapStore = Arc<dyn BootstrapStore>;
pub type DynLocalSessionStore = Arc<dyn LocalSessionStore>;
pub type DynOrganizationStore = Arc<dyn OrganizationStore>;

const LOCAL_BOOTSTRAP_ADMIN_USER_ID: &str = "local_user_bootstrap_admin";

#[derive(Clone, Debug)]
pub struct FileBootstrapStore {
    state_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct FileLocalSessionStore {
    state_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct FileOrganizationStore {
    state_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct PgLocalSessionStore {
    pool: sqlx::PgPool,
}

#[derive(Clone, Debug)]
pub struct PgBootstrapStore {
    pool: sqlx::PgPool,
}

#[derive(Clone, Debug)]
pub struct PgOrganizationAuthMetadataStore {
    pool: sqlx::PgPool,
}

#[derive(Clone, Debug)]
pub struct PgOrganizationStore {
    file_store: FileOrganizationStore,
    pool: sqlx::PgPool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrganizationAuthMetadataState {
    pub organizations: Vec<Organization>,
    pub accounts: Vec<LocalAccount>,
    pub memberships: Vec<OrganizationMembership>,
    pub invites: Vec<OrganizationInvite>,
    pub repo_permissions: Vec<RepositoryPermissionBinding>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedeemedInviteRecord {
    pub account: LocalAccount,
    pub membership: OrganizationMembership,
    pub invite: OrganizationInvite,
}

#[derive(Debug)]
struct StateFileWriteLock {
    file: File,
    lock_path: PathBuf,
}

impl Drop for StateFileWriteLock {
    fn drop(&mut self) {
        let _ = self.file.sync_all();
        let _ = fs::remove_file(&self.lock_path);
    }
}

impl FileBootstrapStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn read_persisted_state(&self) -> Result<Option<BootstrapState>> {
        if !self.state_path.is_file() {
            return Ok(None);
        }

        match fs::read(&self.state_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<BootstrapState>(&bytes).ok()),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl FileLocalSessionStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn lock_path(&self) -> PathBuf {
        let file_name = self
            .state_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("local-sessions.json");
        self.state_path.with_file_name(format!(".{file_name}.lock"))
    }

    fn acquire_write_lock(&self) -> Result<StateFileWriteLock> {
        const MAX_LOCK_WAIT: Duration = Duration::from_millis(100);
        const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);

        ensure_parent_directory(&self.state_path)?;
        let lock_path = self.lock_path();
        let start = SystemTime::now();

        loop {
            match open_new_private_file(&lock_path) {
                Ok(file) => return Ok(StateFileWriteLock { file, lock_path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if start.elapsed().unwrap_or_default() >= MAX_LOCK_WAIT {
                        return Err(anyhow!(
                            "timed out waiting for local session lock at {}",
                            lock_path.display()
                        ));
                    }
                    thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_persisted_state(&self) -> Result<LocalSessionState> {
        if !self.state_path.is_file() {
            return Ok(LocalSessionState::default());
        }

        match fs::read(&self.state_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<LocalSessionState>(&bytes)?),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(LocalSessionState::default()),
            Err(error) => Err(error.into()),
        }
    }
}

impl PgLocalSessionStore {
    pub fn connect_lazy(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }
}

impl PgBootstrapStore {
    pub fn connect_lazy(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }

    async fn bootstrap_row(&self) -> Result<Option<BootstrapState>> {
        let row = sqlx::query(
            "SELECT email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1 AND password_hash IS NOT NULL",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| BootstrapState {
            initialized_at: row.get("created_at"),
            admin_email: row.get("email"),
            admin_name: row.get("name"),
            password_hash: row.get("password_hash"),
        }))
    }
}

impl PgOrganizationAuthMetadataStore {
    pub fn connect_lazy(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
        Ok(Self { pool })
    }

    pub async fn local_account_by_email(&self, email: &str) -> Result<Option<LocalAccount>> {
        let rows = sqlx::query(
            "SELECT id, email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE lower(email) = lower($1) ORDER BY id LIMIT 2",
        )
        .bind(email.trim())
        .fetch_all(&self.pool)
        .await?;

        if rows.len() > 1 {
            return Ok(None);
        }

        rows.into_iter()
            .next()
            .map(local_account_from_row)
            .transpose()
    }

    pub async fn local_account_by_id(&self, user_id: &str) -> Result<Option<LocalAccount>> {
        let row = sqlx::query(
            "SELECT id, email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(local_account_from_row).transpose()
    }

    pub async fn admin_organization_auth_metadata(
        &self,
        user_id: &str,
    ) -> Result<OrganizationAuthMetadataState> {
        let organization_ids = sqlx::query_scalar::<_, String>(
            "SELECT organization_id FROM organization_memberships WHERE user_id = $1 AND role = 'admin' ORDER BY organization_id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        self.organization_auth_metadata_for_organization_ids(&organization_ids)
            .await
    }

    pub async fn user_organization_auth_metadata(
        &self,
        user_id: &str,
    ) -> Result<Option<OrganizationAuthMetadataState>> {
        let Some(account) = self.local_account_by_id(user_id).await? else {
            return Ok(None);
        };
        let memberships = sqlx::query(
            "SELECT organization_id, user_id, role, to_char(joined_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS joined_at FROM organization_memberships WHERE user_id = $1 ORDER BY organization_id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(organization_membership_from_row)
        .collect::<Result<Vec<_>>>()?;
        let organization_ids = memberships
            .iter()
            .map(|membership| membership.organization_id.clone())
            .collect::<Vec<_>>();
        let organizations = self.organizations_for_ids(&organization_ids).await?;
        let repo_permissions = self
            .repo_permissions_for_organization_ids(&organization_ids)
            .await?;

        Ok(Some(OrganizationAuthMetadataState {
            organizations,
            accounts: vec![account],
            memberships,
            invites: vec![],
            repo_permissions,
        }))
    }

    pub async fn redeem_invite(
        &self,
        invite_id: &str,
        email: &str,
        name: &str,
        password_hash: &str,
        accepted_at: &str,
        generated_user_id: &str,
    ) -> Result<Option<RedeemedInviteRecord>> {
        let mut tx = self.pool.begin().await?;
        let Some(invite_row) = sqlx::query(
            "SELECT id, organization_id, email, role, invited_by_user_id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at, accepted_by_user_id, CASE WHEN accepted_at IS NULL THEN NULL ELSE to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS accepted_at FROM organization_invites WHERE id = $1 FOR UPDATE",
        )
        .bind(invite_id)
        .fetch_optional(&mut *tx)
        .await? else {
            return Ok(None);
        };
        let invite = organization_invite_from_row(invite_row)?;
        let invite_expired = OffsetDateTime::parse(invite.expires_at.trim(), &Rfc3339)
            .ok()
            .map(|expires_at| expires_at < OffsetDateTime::now_utc())
            .unwrap_or(true);
        if !invite.email.eq_ignore_ascii_case(email)
            || invite.accepted_by_user_id.is_some()
            || invite.accepted_at.is_some()
            || invite_expired
        {
            return Ok(None);
        }

        let account_rows = sqlx::query(
            "SELECT id, email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE lower(email) = lower($1) ORDER BY id LIMIT 2 FOR UPDATE",
        )
        .bind(email.trim())
        .fetch_all(&mut *tx)
        .await?;

        let account = if account_rows.len() > 1 {
            return Ok(None);
        } else if let Some(account_row) = account_rows.into_iter().next() {
            let existing_account = local_account_from_row(account_row)?;
            if existing_account.password_hash.is_some() {
                return Ok(None);
            }
            let updated_name = if existing_account.name.trim().is_empty() {
                name.trim().to_string()
            } else {
                existing_account.name.clone()
            };
            sqlx::query("UPDATE local_accounts SET name = $2, password_hash = $3 WHERE id = $1")
                .bind(&existing_account.id)
                .bind(&updated_name)
                .bind(password_hash)
                .execute(&mut *tx)
                .await?;
            LocalAccount {
                id: existing_account.id,
                email: existing_account.email,
                name: updated_name,
                password_hash: Some(password_hash.to_string()),
                created_at: existing_account.created_at,
            }
        } else {
            sqlx::query(
                "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ($1, $2, $3, $4, $5::timestamptz)",
            )
            .bind(generated_user_id)
            .bind(email.trim())
            .bind(name.trim())
            .bind(password_hash)
            .bind(accepted_at)
            .execute(&mut *tx)
            .await?;
            LocalAccount {
                id: generated_user_id.to_string(),
                email: email.trim().to_string(),
                name: name.trim().to_string(),
                password_hash: Some(password_hash.to_string()),
                created_at: accepted_at.to_string(),
            }
        };

        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, joined_at) VALUES ($1, $2, $3, $4::timestamptz) ON CONFLICT (organization_id, user_id) DO NOTHING",
        )
        .bind(&invite.organization_id)
        .bind(&account.id)
        .bind(organization_role_as_str(&invite.role))
        .bind(accepted_at)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            "UPDATE organization_invites SET accepted_by_user_id = $2, accepted_at = $3::timestamptz WHERE id = $1",
        )
        .bind(invite_id)
        .bind(&account.id)
        .bind(accepted_at)
        .execute(&mut *tx)
        .await?;

        let membership = sqlx::query(
            "SELECT organization_id, user_id, role, to_char(joined_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS joined_at FROM organization_memberships WHERE organization_id = $1 AND user_id = $2",
        )
        .bind(&invite.organization_id)
        .bind(&account.id)
        .fetch_one(&mut *tx)
        .await
        .map(organization_membership_from_row)??;
        let invite = sqlx::query(
            "SELECT id, organization_id, email, role, invited_by_user_id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at, accepted_by_user_id, CASE WHEN accepted_at IS NULL THEN NULL ELSE to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS accepted_at FROM organization_invites WHERE id = $1",
        )
        .bind(invite_id)
        .fetch_one(&mut *tx)
        .await
        .map(organization_invite_from_row)??;

        tx.commit().await?;

        Ok(Some(RedeemedInviteRecord {
            account,
            membership,
            invite,
        }))
    }

    pub async fn api_keys_for_user(&self, user_id: &str) -> Result<Vec<ApiKey>> {
        sqlx::query(
            "SELECT id, user_id, name, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, CASE WHEN revoked_at IS NULL THEN NULL ELSE to_char(revoked_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS revoked_at, repo_scope FROM api_keys WHERE user_id = $1 ORDER BY created_at, id",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(api_key_from_row)
        .collect()
    }

    pub async fn api_key_by_id(&self, api_key_id: &str) -> Result<Option<ApiKey>> {
        let row = sqlx::query(
            "SELECT id, user_id, name, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, CASE WHEN revoked_at IS NULL THEN NULL ELSE to_char(revoked_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS revoked_at, repo_scope FROM api_keys WHERE id = $1",
        )
        .bind(api_key_id)
        .fetch_optional(&self.pool)
        .await?;

        row.map(api_key_from_row).transpose()
    }

    pub async fn create_api_key(&self, api_key: ApiKey) -> Result<ApiKey> {
        sqlx::query(
            "INSERT INTO api_keys (id, user_id, name, secret_hash, created_at, revoked_at, repo_scope) VALUES ($1, $2, $3, $4, $5::timestamptz, $6::timestamptz, $7::text[])",
        )
        .bind(&api_key.id)
        .bind(&api_key.user_id)
        .bind(&api_key.name)
        .bind(&api_key.secret_hash)
        .bind(&api_key.created_at)
        .bind(api_key.revoked_at.as_deref())
        .bind(&api_key.repo_scope)
        .execute(&self.pool)
        .await?;

        Ok(api_key)
    }

    pub async fn delete_api_key(&self, api_key_id: &str, user_id: &str) -> Result<bool> {
        Ok(
            sqlx::query("DELETE FROM api_keys WHERE id = $1 AND user_id = $2")
                .bind(api_key_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }

    pub async fn revoke_api_key(
        &self,
        api_key_id: &str,
        user_id: &str,
        revoked_at: &str,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE api_keys SET revoked_at = $3::timestamptz WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
        )
        .bind(api_key_id)
        .bind(user_id)
        .bind(revoked_at)
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn restore_api_key_revocation(
        &self,
        api_key_id: &str,
        user_id: &str,
        revoked_at: &str,
    ) -> Result<bool> {
        Ok(sqlx::query(
            "UPDATE api_keys SET revoked_at = NULL WHERE id = $1 AND user_id = $2 AND revoked_at = $3::timestamptz",
        )
        .bind(api_key_id)
        .bind(user_id)
        .bind(revoked_at)
        .execute(&self.pool)
        .await?
        .rows_affected()
            == 1)
    }

    pub async fn oauth_clients_for_organizations(
        &self,
        organization_ids: &[String],
    ) -> Result<Vec<OAuthClient>> {
        if organization_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query(
            "SELECT id, organization_id, name, client_id, client_secret_hash, redirect_uris, created_by_user_id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, CASE WHEN revoked_at IS NULL THEN NULL ELSE to_char(revoked_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS revoked_at FROM oauth_clients WHERE organization_id = ANY($1::text[]) ORDER BY created_at, id",
        )
        .bind(organization_ids)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(oauth_client_from_row)
        .collect()
    }

    pub async fn create_oauth_client(&self, client: OAuthClient) -> Result<OAuthClient> {
        sqlx::query(
            "INSERT INTO oauth_clients (id, organization_id, name, client_id, client_secret_hash, redirect_uris, created_by_user_id, created_at, revoked_at) VALUES ($1, $2, $3, $4, $5, $6::text[], $7, $8::timestamptz, $9::timestamptz)",
        )
        .bind(&client.id)
        .bind(&client.organization_id)
        .bind(&client.name)
        .bind(&client.client_id)
        .bind(&client.client_secret_hash)
        .bind(&client.redirect_uris)
        .bind(&client.created_by_user_id)
        .bind(&client.created_at)
        .bind(client.revoked_at.as_deref())
        .execute(&self.pool)
        .await?;

        Ok(client)
    }

    pub async fn delete_oauth_client(
        &self,
        oauth_client_id: &str,
        organization_id: &str,
    ) -> Result<bool> {
        Ok(
            sqlx::query("DELETE FROM oauth_clients WHERE id = $1 AND organization_id = $2")
                .bind(oauth_client_id)
                .bind(organization_id)
                .execute(&self.pool)
                .await?
                .rows_affected()
                == 1,
        )
    }

    async fn organization_auth_metadata_for_organization_ids(
        &self,
        organization_ids: &[String],
    ) -> Result<OrganizationAuthMetadataState> {
        let organizations = self.organizations_for_ids(organization_ids).await?;
        let memberships = if organization_ids.is_empty() {
            vec![]
        } else {
            sqlx::query(
                "SELECT organization_id, user_id, role, to_char(joined_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS joined_at FROM organization_memberships WHERE organization_id = ANY($1::text[]) ORDER BY organization_id, user_id",
            )
            .bind(organization_ids)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(organization_membership_from_row)
            .collect::<Result<Vec<_>>>()?
        };
        let user_ids = memberships
            .iter()
            .map(|membership| membership.user_id.clone())
            .collect::<Vec<_>>();
        let accounts = self.accounts_for_ids(&user_ids).await?;
        let invites = if organization_ids.is_empty() {
            vec![]
        } else {
            sqlx::query(
                "SELECT id, organization_id, email, role, invited_by_user_id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, to_char(expires_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS expires_at, accepted_by_user_id, CASE WHEN accepted_at IS NULL THEN NULL ELSE to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS accepted_at FROM organization_invites WHERE organization_id = ANY($1::text[]) ORDER BY organization_id, id",
            )
            .bind(organization_ids)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(organization_invite_from_row)
            .collect::<Result<Vec<_>>>()?
        };
        let repo_permissions = self
            .repo_permissions_for_organization_ids(organization_ids)
            .await?;

        Ok(OrganizationAuthMetadataState {
            organizations,
            accounts,
            memberships,
            invites,
            repo_permissions,
        })
    }

    async fn organizations_for_ids(
        &self,
        organization_ids: &[String],
    ) -> Result<Vec<Organization>> {
        if organization_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query(
            "SELECT id, slug, name FROM organizations WHERE id = ANY($1::text[]) ORDER BY id",
        )
        .bind(organization_ids)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(organization_from_row)
        .collect()
    }

    async fn accounts_for_ids(&self, user_ids: &[String]) -> Result<Vec<LocalAccount>> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query(
            "SELECT id, email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = ANY($1::text[]) ORDER BY id",
        )
        .bind(user_ids)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(local_account_from_row)
        .collect()
    }

    async fn repo_permissions_for_organization_ids(
        &self,
        organization_ids: &[String],
    ) -> Result<Vec<RepositoryPermissionBinding>> {
        if organization_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query(
            "SELECT organization_id, repository_id, to_char(synced_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS synced_at FROM repository_permission_bindings WHERE organization_id = ANY($1::text[]) ORDER BY organization_id, repository_id",
        )
        .bind(organization_ids)
        .fetch_all(&self.pool)
        .await?
        .into_iter()
        .map(repository_permission_binding_from_row)
        .collect()
    }
}

fn organization_role_from_db(value: &str) -> Result<OrganizationRole> {
    match value {
        "admin" => Ok(OrganizationRole::Admin),
        "viewer" => Ok(OrganizationRole::Viewer),
        other => Err(anyhow!("unrecognized organization role: {other}")),
    }
}

fn organization_role_as_str(role: &OrganizationRole) -> &'static str {
    match role {
        OrganizationRole::Admin => "admin",
        OrganizationRole::Viewer => "viewer",
    }
}

fn organization_from_row(row: sqlx::postgres::PgRow) -> Result<Organization> {
    Ok(Organization {
        id: row.get("id"),
        slug: row.get("slug"),
        name: row.get("name"),
    })
}

fn local_account_from_row(row: sqlx::postgres::PgRow) -> Result<LocalAccount> {
    Ok(LocalAccount {
        id: row.get("id"),
        email: row.get("email"),
        name: row.get("name"),
        password_hash: row.try_get("password_hash")?,
        created_at: row.get("created_at"),
    })
}

fn organization_membership_from_row(row: sqlx::postgres::PgRow) -> Result<OrganizationMembership> {
    Ok(OrganizationMembership {
        organization_id: row.get("organization_id"),
        user_id: row.get("user_id"),
        role: organization_role_from_db(&row.get::<String, _>("role"))?,
        joined_at: row.get("joined_at"),
    })
}

fn organization_invite_from_row(row: sqlx::postgres::PgRow) -> Result<OrganizationInvite> {
    Ok(OrganizationInvite {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        email: row.get("email"),
        role: organization_role_from_db(&row.get::<String, _>("role"))?,
        invited_by_user_id: row.get("invited_by_user_id"),
        created_at: row.get("created_at"),
        expires_at: row.get("expires_at"),
        accepted_by_user_id: row.try_get("accepted_by_user_id")?,
        accepted_at: row.try_get("accepted_at")?,
    })
}

fn repository_permission_binding_from_row(
    row: sqlx::postgres::PgRow,
) -> Result<RepositoryPermissionBinding> {
    Ok(RepositoryPermissionBinding {
        organization_id: row.get("organization_id"),
        repository_id: row.get("repository_id"),
        synced_at: row.get("synced_at"),
    })
}

fn api_key_from_row(row: sqlx::postgres::PgRow) -> Result<ApiKey> {
    Ok(ApiKey {
        id: row.get("id"),
        user_id: row.get("user_id"),
        name: row.get("name"),
        secret_hash: row.get("secret_hash"),
        created_at: row.get("created_at"),
        revoked_at: row.try_get("revoked_at")?,
        repo_scope: row.get("repo_scope"),
    })
}

fn oauth_client_from_row(row: sqlx::postgres::PgRow) -> Result<OAuthClient> {
    Ok(OAuthClient {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        name: row.get("name"),
        client_id: row.get("client_id"),
        client_secret_hash: row.get("client_secret_hash"),
        redirect_uris: row.get("redirect_uris"),
        created_by_user_id: row.get("created_by_user_id"),
        created_at: row.get("created_at"),
        revoked_at: row.try_get("revoked_at")?,
    })
}

fn repository_sync_job_status_to_str(status: &RepositorySyncJobStatus) -> &'static str {
    match status {
        RepositorySyncJobStatus::Queued => "queued",
        RepositorySyncJobStatus::Running => "running",
        RepositorySyncJobStatus::Succeeded => "succeeded",
        RepositorySyncJobStatus::Failed => "failed",
    }
}

fn repository_sync_job_status_from_str(status: &str) -> Result<RepositorySyncJobStatus> {
    match status {
        "queued" => Ok(RepositorySyncJobStatus::Queued),
        "running" => Ok(RepositorySyncJobStatus::Running),
        "succeeded" => Ok(RepositorySyncJobStatus::Succeeded),
        "failed" => Ok(RepositorySyncJobStatus::Failed),
        _ => Err(anyhow!("unknown repository sync job status: {status}")),
    }
}

fn repository_sync_job_from_row(row: sqlx::postgres::PgRow) -> Result<RepositorySyncJob> {
    Ok(RepositorySyncJob {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        repository_id: row.get("repository_id"),
        connection_id: row.get("connection_id"),
        status: repository_sync_job_status_from_str(row.get::<&str, _>("status"))?,
        queued_at: row.get("queued_at"),
        started_at: row.try_get("started_at")?,
        finished_at: row.try_get("finished_at")?,
        error: row.try_get("error")?,
    })
}

const REPOSITORY_SYNC_JOB_SELECT_COLUMNS: &str = "id, organization_id, repository_id, connection_id, status, to_char(queued_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS queued_at, to_char(started_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS started_at, to_char(finished_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS finished_at, error";

fn review_agent_run_status_to_str(status: &ReviewAgentRunStatus) -> &'static str {
    match status {
        ReviewAgentRunStatus::Queued => "queued",
        ReviewAgentRunStatus::Claimed => "claimed",
        ReviewAgentRunStatus::Completed => "completed",
        ReviewAgentRunStatus::Failed => "failed",
    }
}

fn review_agent_run_status_from_str(status: &str) -> Result<ReviewAgentRunStatus> {
    match status {
        "queued" => Ok(ReviewAgentRunStatus::Queued),
        "claimed" => Ok(ReviewAgentRunStatus::Claimed),
        "completed" => Ok(ReviewAgentRunStatus::Completed),
        "failed" => Ok(ReviewAgentRunStatus::Failed),
        _ => Err(anyhow!("unknown review agent run status: {status}")),
    }
}

fn review_agent_run_from_row(row: sqlx::postgres::PgRow) -> Result<ReviewAgentRun> {
    Ok(ReviewAgentRun {
        id: row.get("id"),
        organization_id: row.get("organization_id"),
        webhook_id: row.get("webhook_id"),
        delivery_attempt_id: row.get("delivery_attempt_id"),
        connection_id: row.get("connection_id"),
        repository_id: row.get("repository_id"),
        review_id: row.get("review_id"),
        status: review_agent_run_status_from_str(row.get::<&str, _>("status"))?,
        created_at: row.get("created_at"),
    })
}

const REVIEW_AGENT_RUN_SELECT_COLUMNS: &str = "id, organization_id, webhook_id, delivery_attempt_id, connection_id, repository_id, review_id, status, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at";

impl PgOrganizationStore {
    pub fn new(file_store: FileOrganizationStore, pool: sqlx::PgPool) -> Self {
        Self { file_store, pool }
    }

    pub fn connect_lazy(state_path: impl Into<PathBuf>, database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_lazy(database_url)?;
        Ok(Self::new(FileOrganizationStore::new(state_path), pool))
    }

    async fn repository_sync_jobs(&self) -> Result<Vec<RepositorySyncJob>> {
        let sql = format!(
            "SELECT {REPOSITORY_SYNC_JOB_SELECT_COLUMNS} FROM repository_sync_jobs ORDER BY queued_at, id"
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        rows.into_iter().map(repository_sync_job_from_row).collect()
    }

    async fn review_agent_runs(&self) -> Result<Vec<ReviewAgentRun>> {
        let sql = format!(
            "SELECT {REVIEW_AGENT_RUN_SELECT_COLUMNS} FROM review_agent_runs ORDER BY created_at, id"
        );
        let rows = sqlx::query(&sql).fetch_all(&self.pool).await?;
        rows.into_iter().map(review_agent_run_from_row).collect()
    }

    async fn upsert_repository_sync_job<'e, E>(executor: E, job: RepositorySyncJob) -> Result<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query(
            "INSERT INTO repository_sync_jobs (id, organization_id, repository_id, connection_id, status, queued_at, started_at, finished_at, error)
             VALUES ($1, $2, $3, $4, $5, $6::timestamptz, $7::timestamptz, $8::timestamptz, $9)
             ON CONFLICT (id) DO UPDATE SET
                organization_id = EXCLUDED.organization_id,
                repository_id = EXCLUDED.repository_id,
                connection_id = EXCLUDED.connection_id,
                status = EXCLUDED.status,
                queued_at = EXCLUDED.queued_at,
                started_at = EXCLUDED.started_at,
                finished_at = EXCLUDED.finished_at,
                error = EXCLUDED.error
             WHERE
                (CASE repository_sync_jobs.status WHEN 'queued' THEN 0 WHEN 'running' THEN 1 ELSE 2 END)
                  <= (CASE EXCLUDED.status WHEN 'queued' THEN 0 WHEN 'running' THEN 1 ELSE 2 END)
                AND NOT (
                    repository_sync_jobs.status IN ('succeeded', 'failed')
                    AND EXCLUDED.status IN ('succeeded', 'failed')
                    AND repository_sync_jobs.status <> EXCLUDED.status
                )",
        )
        .bind(job.id)
        .bind(job.organization_id)
        .bind(job.repository_id)
        .bind(job.connection_id)
        .bind(repository_sync_job_status_to_str(&job.status))
        .bind(job.queued_at)
        .bind(job.started_at)
        .bind(job.finished_at)
        .bind(job.error)
        .execute(executor)
        .await?;
        Ok(())
    }

    async fn upsert_review_agent_run<'e, E>(executor: E, run: ReviewAgentRun) -> Result<()>
    where
        E: sqlx::Executor<'e, Database = sqlx::Postgres>,
    {
        sqlx::query(
            "INSERT INTO review_agent_runs (id, organization_id, webhook_id, delivery_attempt_id, connection_id, repository_id, review_id, status, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9::timestamptz)
             ON CONFLICT (id) DO UPDATE SET
                organization_id = EXCLUDED.organization_id,
                webhook_id = EXCLUDED.webhook_id,
                delivery_attempt_id = EXCLUDED.delivery_attempt_id,
                connection_id = EXCLUDED.connection_id,
                repository_id = EXCLUDED.repository_id,
                review_id = EXCLUDED.review_id,
                status = EXCLUDED.status,
                created_at = EXCLUDED.created_at
             WHERE
                (CASE review_agent_runs.status WHEN 'queued' THEN 0 WHEN 'claimed' THEN 1 ELSE 2 END)
                  <= (CASE EXCLUDED.status WHEN 'queued' THEN 0 WHEN 'claimed' THEN 1 ELSE 2 END)
                AND NOT (
                    review_agent_runs.status IN ('completed', 'failed')
                    AND EXCLUDED.status IN ('completed', 'failed')
                    AND review_agent_runs.status <> EXCLUDED.status
                )",
        )
        .bind(run.id)
        .bind(run.organization_id)
        .bind(run.webhook_id)
        .bind(run.delivery_attempt_id)
        .bind(run.connection_id)
        .bind(run.repository_id)
        .bind(run.review_id)
        .bind(review_agent_run_status_to_str(&run.status))
        .bind(run.created_at)
        .execute(executor)
        .await?;
        Ok(())
    }
}

impl FileOrganizationStore {
    pub fn new(state_path: impl Into<PathBuf>) -> Self {
        Self {
            state_path: state_path.into(),
        }
    }

    fn lock_path(&self) -> PathBuf {
        let file_name = self
            .state_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("organization-state.json");
        self.state_path.with_file_name(format!(".{file_name}.lock"))
    }

    fn acquire_write_lock(&self) -> Result<StateFileWriteLock> {
        const MAX_LOCK_WAIT: Duration = Duration::from_millis(100);
        const LOCK_RETRY_DELAY: Duration = Duration::from_millis(10);

        ensure_parent_directory(&self.state_path)?;
        let lock_path = self.lock_path();
        let start = SystemTime::now();

        loop {
            match open_new_private_file(&lock_path) {
                Ok(file) => return Ok(StateFileWriteLock { file, lock_path }),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    if start.elapsed().unwrap_or_default() >= MAX_LOCK_WAIT {
                        return Err(anyhow!(
                            "timed out waiting for organization-state lock at {}",
                            lock_path.display()
                        ));
                    }
                    thread::sleep(LOCK_RETRY_DELAY);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    fn read_persisted_state(&self) -> Result<OrganizationState> {
        if !self.state_path.is_file() {
            return Ok(OrganizationState::default());
        }

        match fs::read(&self.state_path) {
            Ok(bytes) => Ok(serde_json::from_slice::<OrganizationState>(&bytes)?),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(OrganizationState::default()),
            Err(error) => Err(error.into()),
        }
    }
}

fn review_agent_run_status_rank(status: &ReviewAgentRunStatus) -> u8 {
    match status {
        ReviewAgentRunStatus::Queued => 0,
        ReviewAgentRunStatus::Claimed => 1,
        ReviewAgentRunStatus::Completed | ReviewAgentRunStatus::Failed => 2,
    }
}

fn preserve_terminal_review_agent_runs(
    persisted_state: &OrganizationState,
    next_state: &mut OrganizationState,
) {
    for persisted_run in &persisted_state.review_agent_runs {
        if !matches!(
            persisted_run.status,
            ReviewAgentRunStatus::Claimed
                | ReviewAgentRunStatus::Completed
                | ReviewAgentRunStatus::Failed
        ) {
            continue;
        }

        if let Some(next_run) = next_state
            .review_agent_runs
            .iter_mut()
            .find(|run| run.id == persisted_run.id)
        {
            let persisted_rank = review_agent_run_status_rank(&persisted_run.status);
            let next_rank = review_agent_run_status_rank(&next_run.status);
            let persisted_terminal_mismatch =
                persisted_rank == 2 && next_rank == 2 && next_run.status != persisted_run.status;

            if persisted_rank > next_rank || persisted_terminal_mismatch {
                next_run.status = persisted_run.status.clone();
            }
        }
    }
}

fn repository_sync_job_status_rank(status: &RepositorySyncJobStatus) -> u8 {
    match status {
        RepositorySyncJobStatus::Queued => 0,
        RepositorySyncJobStatus::Running => 1,
        RepositorySyncJobStatus::Succeeded | RepositorySyncJobStatus::Failed => 2,
    }
}

fn preserve_repository_sync_job_progress(
    persisted_state: &OrganizationState,
    next_state: &mut OrganizationState,
) {
    for persisted_job in &persisted_state.repository_sync_jobs {
        if let Some(next_job) = next_state
            .repository_sync_jobs
            .iter_mut()
            .find(|job| job.id == persisted_job.id)
        {
            let persisted_rank = repository_sync_job_status_rank(&persisted_job.status);
            let next_rank = repository_sync_job_status_rank(&next_job.status);
            let persisted_terminal_mismatch =
                persisted_rank == 2 && next_rank == 2 && next_job.status != persisted_job.status;

            if persisted_rank > next_rank || persisted_terminal_mismatch {
                *next_job = persisted_job.clone();
            }
        } else {
            next_state.repository_sync_jobs.push(persisted_job.clone());
        }
    }
}

fn temporary_state_path(state_path: &Path, fallback_name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let file_name = state_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(fallback_name);
    state_path.with_file_name(format!(".{file_name}.{nanos}.tmp"))
}

fn open_new_private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        options.mode(0o600);
    }
    options.open(path)
}

fn sync_parent_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            File::open(parent)?.sync_all()?;
        }
    }

    Ok(())
}

fn ensure_parent_directory(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

fn write_json_file(path: &Path, payload: &[u8], replace_existing: bool) -> std::io::Result<()> {
    ensure_parent_directory(path)?;

    let temp_path = temporary_state_path(path, "state.json");
    let write_result = (|| -> std::io::Result<()> {
        let mut file = open_new_private_file(&temp_path)?;
        file.write_all(payload)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        drop(file);

        if replace_existing {
            fs::rename(&temp_path, path)?;
        } else {
            fs::hard_link(&temp_path, path)?;
            fs::remove_file(&temp_path)?;
        }

        sync_parent_directory(path)?;
        Ok(())
    })();

    if let Err(error) = write_result {
        match fs::remove_file(&temp_path) {
            Ok(()) => {}
            Err(remove_error) if remove_error.kind() == ErrorKind::NotFound => {}
            Err(remove_error) => return Err(remove_error),
        }
        return Err(error);
    }

    Ok(())
}

#[async_trait]
impl BootstrapStore for FileBootstrapStore {
    async fn bootstrap_status(&self) -> Result<BootstrapStatus> {
        let bootstrap_required = self.read_persisted_state()?.is_none();

        Ok(BootstrapStatus { bootstrap_required })
    }

    async fn bootstrap_state(&self) -> Result<Option<BootstrapState>> {
        self.read_persisted_state()
    }

    async fn initialize_bootstrap(&self, state: BootstrapState) -> Result<()> {
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, false)?;
        Ok(())
    }
}

#[async_trait]
impl BootstrapStore for PgBootstrapStore {
    async fn bootstrap_status(&self) -> Result<BootstrapStatus> {
        let bootstrap_required =
            sqlx::query("SELECT 1 FROM local_accounts WHERE id = $1 AND password_hash IS NOT NULL")
                .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
                .fetch_optional(&self.pool)
                .await?
                .is_none();

        Ok(BootstrapStatus { bootstrap_required })
    }

    async fn bootstrap_state(&self) -> Result<Option<BootstrapState>> {
        self.bootstrap_row().await
    }

    async fn initialize_bootstrap(&self, state: BootstrapState) -> Result<()> {
        let result = sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ($1, $2, $3, $4, $5::timestamptz) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email, name = EXCLUDED.name, password_hash = EXCLUDED.password_hash, created_at = EXCLUDED.created_at WHERE local_accounts.password_hash IS NULL",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind(state.admin_email)
        .bind(state.admin_name)
        .bind(state.password_hash)
        .bind(state.initialized_at)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "bootstrap already initialized",
            )
            .into());
        }

        Ok(())
    }
}

#[async_trait]
impl LocalSessionStore for FileLocalSessionStore {
    async fn local_session(&self, session_id: &str) -> Result<Option<LocalSession>> {
        let state = self.read_persisted_state()?;
        Ok(state
            .sessions
            .into_iter()
            .find(|session| session.id == session_id))
    }

    async fn store_local_session(&self, session: LocalSession) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        state
            .sessions
            .retain(|persisted| persisted.id != session.id);
        state.sessions.push(session);

        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }

    async fn delete_local_session(&self, session_id: &str) -> Result<bool> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let original_len = state.sessions.len();
        state
            .sessions
            .retain(|persisted| persisted.id != session_id);

        if state.sessions.len() == original_len {
            return Ok(false);
        }

        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(true)
    }
}

#[async_trait]
impl LocalSessionStore for PgLocalSessionStore {
    async fn local_session(&self, session_id: &str) -> Result<Option<LocalSession>> {
        let row = sqlx::query(
            "SELECT id, user_id, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| LocalSession {
            id: row.get("id"),
            user_id: row.get("user_id"),
            secret_hash: row.get("secret_hash"),
            created_at: row.get("created_at"),
        }))
    }

    async fn persist_local_session_account(&self, account: LocalAccount) -> Result<()> {
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ($1, $2, $3, $4::timestamptz) ON CONFLICT (id) DO UPDATE SET email = EXCLUDED.email, name = EXCLUDED.name, created_at = EXCLUDED.created_at",
        )
        .bind(account.id)
        .bind(account.email)
        .bind(account.name)
        .bind(account.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn store_local_session(&self, session: LocalSession) -> Result<()> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, secret_hash, created_at) VALUES ($1, $2, $3, $4::timestamptz) ON CONFLICT (id) DO UPDATE SET user_id = EXCLUDED.user_id, secret_hash = EXCLUDED.secret_hash, created_at = EXCLUDED.created_at",
        )
        .bind(session.id)
        .bind(session.user_id)
        .bind(session.secret_hash)
        .bind(session.created_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_local_session(&self, session_id: &str) -> Result<bool> {
        let result = sqlx::query("DELETE FROM sessions WHERE id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[async_trait]
impl OrganizationStore for PgOrganizationStore {
    async fn organization_state(&self) -> Result<OrganizationState> {
        let mut state = self.file_store.organization_state().await?;
        state.repository_sync_jobs = self.repository_sync_jobs().await?;
        state.review_agent_runs = self.review_agent_runs().await?;
        Ok(state)
    }

    async fn store_organization_state(&self, state: OrganizationState) -> Result<()> {
        let mut file_state = state.clone();
        file_state.repository_sync_jobs.clear();
        file_state.review_agent_runs.clear();
        self.file_store.store_organization_state(file_state).await?;

        let mut transaction = self.pool.begin().await?;
        for job in state.repository_sync_jobs {
            Self::upsert_repository_sync_job(&mut *transaction, job).await?;
        }
        for run in state.review_agent_runs {
            Self::upsert_review_agent_run(&mut *transaction, run).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn store_repository_sync_job(&self, job: RepositorySyncJob) -> Result<()> {
        Self::upsert_repository_sync_job(&self.pool, job).await
    }

    async fn claim_next_repository_sync_job(
        &self,
        started_at: &str,
    ) -> Result<Option<RepositorySyncJob>> {
        let mut transaction = self.pool.begin().await?;
        let sql = format!(
            "UPDATE repository_sync_jobs SET status = 'running', started_at = $1::timestamptz, finished_at = NULL, error = NULL
             WHERE id = (
                SELECT id FROM repository_sync_jobs
                WHERE status = 'queued'
                ORDER BY queued_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             RETURNING {REPOSITORY_SYNC_JOB_SELECT_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(started_at)
            .fetch_optional(&mut *transaction)
            .await?;
        transaction.commit().await?;
        row.map(repository_sync_job_from_row).transpose()
    }

    async fn claim_and_complete_next_repository_sync_job(
        &self,
        started_at: &str,
        execute: fn(RepositorySyncJob) -> RepositorySyncJob,
    ) -> Result<Option<RepositorySyncJob>> {
        let mut transaction = self.pool.begin().await?;
        let select_sql = format!(
            "SELECT {REPOSITORY_SYNC_JOB_SELECT_COLUMNS} FROM repository_sync_jobs
             WHERE status = 'queued'
             ORDER BY queued_at, id
             FOR UPDATE SKIP LOCKED
             LIMIT 1"
        );
        let Some(row) = sqlx::query(&select_sql)
            .fetch_optional(&mut *transaction)
            .await?
        else {
            transaction.commit().await?;
            return Ok(None);
        };

        let mut claimed_job = repository_sync_job_from_row(row)?;
        claimed_job.status = RepositorySyncJobStatus::Running;
        claimed_job.started_at = Some(started_at.to_owned());
        claimed_job.finished_at = None;
        claimed_job.error = None;
        let completed_job = execute(claimed_job);
        Self::upsert_repository_sync_job(&mut *transaction, completed_job.clone()).await?;
        transaction.commit().await?;
        Ok(Some(completed_job))
    }

    async fn claim_next_review_agent_run(
        &self,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let mut transaction = self.pool.begin().await?;
        let sql = format!(
            "UPDATE review_agent_runs SET status = 'claimed'
             WHERE id = (
                SELECT id FROM review_agent_runs
                WHERE status = 'queued'
                ORDER BY created_at, id
                FOR UPDATE SKIP LOCKED
                LIMIT 1
             )
             RETURNING {REVIEW_AGENT_RUN_SELECT_COLUMNS}"
        );
        let row = sqlx::query(&sql).fetch_optional(&mut *transaction).await?;
        transaction.commit().await?;
        row.map(review_agent_run_from_row).transpose()
    }

    async fn complete_review_agent_run(
        &self,
        run_id: &str,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let sql = format!(
            "UPDATE review_agent_runs SET status = 'completed'
             WHERE id = $1 AND status = 'claimed'
             RETURNING {REVIEW_AGENT_RUN_SELECT_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(review_agent_run_from_row).transpose()
    }

    async fn fail_review_agent_run(
        &self,
        run_id: &str,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let sql = format!(
            "UPDATE review_agent_runs SET status = 'failed'
             WHERE id = $1 AND status = 'claimed'
             RETURNING {REVIEW_AGENT_RUN_SELECT_COLUMNS}"
        );
        let row = sqlx::query(&sql)
            .bind(run_id)
            .fetch_optional(&self.pool)
            .await?;
        row.map(review_agent_run_from_row).transpose()
    }
}

#[async_trait]
impl OrganizationStore for FileOrganizationStore {
    async fn organization_state(&self) -> Result<OrganizationState> {
        self.read_persisted_state()
    }

    async fn store_organization_state(&self, state: OrganizationState) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        let persisted_state = self.read_persisted_state()?;
        let mut state = state;
        preserve_terminal_review_agent_runs(&persisted_state, &mut state);
        preserve_repository_sync_job_progress(&persisted_state, &mut state);
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }

    async fn store_repository_sync_job(&self, job: RepositorySyncJob) -> Result<()> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        upsert_repository_sync_job(&mut state, job);
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(())
    }

    async fn claim_next_repository_sync_job(
        &self,
        started_at: &str,
    ) -> Result<Option<RepositorySyncJob>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let claimed_job = claim_next_repository_sync_job(&mut state, started_at);

        if claimed_job.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(claimed_job)
    }

    async fn claim_and_complete_next_repository_sync_job(
        &self,
        started_at: &str,
        execute: fn(RepositorySyncJob) -> RepositorySyncJob,
    ) -> Result<Option<RepositorySyncJob>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let Some(claimed_job) = claim_next_repository_sync_job(&mut state, started_at) else {
            return Ok(None);
        };

        let completed_job = execute(claimed_job);
        upsert_repository_sync_job(&mut state, completed_job.clone());
        let payload = serde_json::to_vec_pretty(&state)?;
        write_json_file(&self.state_path, &payload, true)?;
        Ok(Some(completed_job))
    }

    async fn claim_next_review_agent_run(
        &self,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let claimed_run = claim_next_review_agent_run(&mut state);

        if claimed_run.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(claimed_run)
    }

    async fn complete_review_agent_run(
        &self,
        run_id: &str,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let completed_run = complete_review_agent_run(&mut state, run_id);

        if completed_run.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(completed_run)
    }

    async fn fail_review_agent_run(
        &self,
        run_id: &str,
    ) -> Result<Option<sourcebot_models::ReviewAgentRun>> {
        let _lock = self.acquire_write_lock()?;
        let mut state = self.read_persisted_state()?;
        let failed_run = fail_review_agent_run(&mut state, run_id);

        if failed_run.is_some() {
            let payload = serde_json::to_vec_pretty(&state)?;
            write_json_file(&self.state_path, &payload, true)?;
        }

        Ok(failed_run)
    }
}

pub fn try_build_bootstrap_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> Result<DynBootstrapStore> {
    if let Some(database_url) = database_url {
        return Ok(Arc::new(PgBootstrapStore::connect_lazy(database_url)?));
    }

    Ok(Arc::new(FileBootstrapStore::new(state_path)))
}

pub fn build_bootstrap_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> DynBootstrapStore {
    try_build_bootstrap_store(state_path, database_url)
        .expect("bootstrap store DATABASE_URL must be valid when configured")
}

pub fn try_build_local_session_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> Result<DynLocalSessionStore> {
    if let Some(database_url) = database_url {
        return Ok(Arc::new(PgLocalSessionStore::connect_lazy(database_url)?));
    }

    Ok(Arc::new(FileLocalSessionStore::new(state_path)))
}

pub fn build_local_session_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> DynLocalSessionStore {
    try_build_local_session_store(state_path, database_url)
        .expect("local session store DATABASE_URL must be valid")
}

pub fn try_build_organization_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> Result<DynOrganizationStore> {
    if let Some(database_url) = database_url {
        return Ok(Arc::new(PgOrganizationStore::connect_lazy(
            state_path,
            database_url,
        )?));
    }

    Ok(Arc::new(FileOrganizationStore::new(state_path)))
}

pub fn build_organization_store(
    state_path: impl Into<PathBuf>,
    database_url: Option<&str>,
) -> DynOrganizationStore {
    try_build_organization_store(state_path, database_url)
        .expect("organization store DATABASE_URL must be valid when configured")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::catalog_migrator;
    use sourcebot_models::{
        AnalyticsRecord, ApiKey, AuditActor, AuditEvent, Connection, ConnectionConfig,
        ConnectionKind, LocalAccount, OAuthClient, Organization, OrganizationInvite,
        OrganizationMembership, OrganizationRole, RepositoryPermissionBinding, RepositorySyncJob,
        RepositorySyncJobStatus, ReviewAgentRun, ReviewAgentRunStatus, ReviewWebhook,
        ReviewWebhookDeliveryAttempt, SearchContext,
    };
    use sqlx::{postgres::PgPoolOptions, Row};
    use std::{
        env, fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn unique_test_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sourcebot-bootstrap-{name}-{nanos}.json"))
    }

    #[tokio::test]
    async fn file_bootstrap_store_requires_bootstrap_when_state_file_is_missing() {
        let path = unique_test_path("missing");
        let store = FileBootstrapStore::new(&path);

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);
    }

    #[tokio::test]
    async fn file_bootstrap_store_requires_bootstrap_when_state_file_is_invalid_json() {
        let path = unique_test_path("invalid-json");
        fs::write(&path, b"{\"initialized_at\":").unwrap();
        let store = FileBootstrapStore::new(&path);

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_disables_bootstrap_when_state_file_exists() {
        let path = unique_test_path("present");
        fs::write(
            &path,
            serde_json::to_vec(&BootstrapState {
                initialized_at: "2026-04-16T17:00:00Z".into(),
                admin_email: "admin@example.com".into(),
                admin_name: "Admin User".into(),
                password_hash: "$argon2id$example".into(),
            })
            .unwrap(),
        )
        .unwrap();

        let store = FileBootstrapStore::new(&path);

        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_requires_bootstrap_when_path_is_a_directory() {
        let path = unique_test_path("directory");
        fs::create_dir(&path).unwrap();

        let store = FileBootstrapStore::new(&path);

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);

        fs::remove_dir(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_persists_initial_admin_state() {
        let path = unique_test_path("persisted-state");
        let store = FileBootstrapStore::new(&path);
        let state = BootstrapState {
            initialized_at: "2026-04-16T17:00:00Z".into(),
            admin_email: "admin@example.com".into(),
            admin_name: "Admin User".into(),
            password_hash: "$argon2id$example".into(),
        };

        store.initialize_bootstrap(state.clone()).await.unwrap();

        let persisted: BootstrapState = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted, state);
        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_bootstrap_store_reads_bootstrap_state_only_when_persisted_state_is_valid() {
        let missing_path = unique_test_path("read-missing");
        let missing_store = FileBootstrapStore::new(&missing_path);
        assert_eq!(missing_store.bootstrap_state().await.unwrap(), None);

        let invalid_path = unique_test_path("read-invalid");
        fs::write(&invalid_path, b"{\"initialized_at\":").unwrap();
        let invalid_store = FileBootstrapStore::new(&invalid_path);
        assert_eq!(invalid_store.bootstrap_state().await.unwrap(), None);
        fs::remove_file(&invalid_path).unwrap();

        let valid_path = unique_test_path("read-valid");
        let valid_store = FileBootstrapStore::new(&valid_path);
        let expected_state = BootstrapState {
            initialized_at: "2026-04-16T17:00:00Z".into(),
            admin_email: "admin@example.com".into(),
            admin_name: "Admin User".into(),
            password_hash: "$argon2id$example".into(),
        };
        valid_store
            .initialize_bootstrap(expected_state.clone())
            .await
            .unwrap();

        assert_eq!(
            valid_store.bootstrap_state().await.unwrap(),
            Some(expected_state)
        );

        fs::remove_file(valid_path).unwrap();
    }

    #[tokio::test]
    async fn pg_bootstrap_store_requires_bootstrap_when_admin_row_is_missing() {
        let pool = bootstrap_test_pool().await;
        let store = PgBootstrapStore { pool: pool.clone() };

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), None);
        assert_eq!(persisted_bootstrap_row(&pool).await, None);
    }

    #[tokio::test]
    async fn pg_bootstrap_store_treats_legacy_bootstrap_row_without_password_hash_as_still_requiring_bootstrap(
    ) {
        let pool = bootstrap_test_pool().await;
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ($1, $2, $3, $4::timestamptz)",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind("legacy@example.com")
        .bind("Legacy Admin")
        .bind("2026-04-22T15:00:00Z")
        .execute(&pool)
        .await
        .unwrap();
        let store = PgBootstrapStore { pool: pool.clone() };

        assert!(store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), None);

        let upgraded_state = BootstrapState {
            initialized_at: "2026-04-22T16:00:00Z".into(),
            admin_email: "bootstrap@example.com".into(),
            admin_name: "Bootstrap Admin".into(),
            password_hash: "$argon2id$bootstrap-hash".into(),
        };
        store
            .initialize_bootstrap(upgraded_state.clone())
            .await
            .unwrap();

        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(
            store.bootstrap_state().await.unwrap(),
            Some(upgraded_state.clone())
        );
        assert_eq!(
            persisted_bootstrap_row(&pool).await,
            Some((
                "bootstrap@example.com".into(),
                "Bootstrap Admin".into(),
                "$argon2id$bootstrap-hash".into(),
                "2026-04-22T16:00:00Z".into(),
            ))
        );
    }

    #[tokio::test]
    async fn pg_bootstrap_store_initializes_bootstrap_admin_once() {
        let pool = bootstrap_test_pool().await;
        let store = PgBootstrapStore { pool: pool.clone() };
        let state = BootstrapState {
            initialized_at: "2026-04-22T16:00:00Z".into(),
            admin_email: "bootstrap@example.com".into(),
            admin_name: "Bootstrap Admin".into(),
            password_hash: "$argon2id$bootstrap-hash".into(),
        };

        store.initialize_bootstrap(state.clone()).await.unwrap();

        let error = store
            .initialize_bootstrap(BootstrapState {
                initialized_at: "2026-04-22T16:01:00Z".into(),
                admin_email: "other@example.com".into(),
                admin_name: "Other Admin".into(),
                password_hash: "$argon2id$other-hash".into(),
            })
            .await
            .unwrap_err();

        assert!(error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io_error| io_error.kind() == ErrorKind::AlreadyExists));
        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(store.bootstrap_state().await.unwrap(), Some(state.clone()));
        assert_eq!(
            persisted_bootstrap_row(&pool).await,
            Some((
                "bootstrap@example.com".into(),
                "Bootstrap Admin".into(),
                "$argon2id$bootstrap-hash".into(),
                "2026-04-22T16:00:00Z".into(),
            ))
        );
    }

    #[tokio::test]
    async fn pg_bootstrap_store_reads_persisted_bootstrap_state() {
        let pool = bootstrap_test_pool().await;
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ($1, $2, $3, $4, $5::timestamptz)",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind("persisted@example.com")
        .bind("Persisted Admin")
        .bind("$argon2id$persisted-hash")
        .bind("2026-04-22T15:30:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let store = PgBootstrapStore { pool: pool.clone() };

        assert!(!store.bootstrap_status().await.unwrap().bootstrap_required);
        assert_eq!(
            store.bootstrap_state().await.unwrap(),
            Some(BootstrapState {
                initialized_at: "2026-04-22T15:30:00Z".into(),
                admin_email: "persisted@example.com".into(),
                admin_name: "Persisted Admin".into(),
                password_hash: "$argon2id$persisted-hash".into(),
            })
        );
        assert_eq!(
            persisted_bootstrap_row(&pool).await,
            Some((
                "persisted@example.com".into(),
                "Persisted Admin".into(),
                "$argon2id$persisted-hash".into(),
                "2026-04-22T15:30:00Z".into(),
            ))
        );
    }

    #[tokio::test]
    async fn file_local_session_store_returns_none_when_session_file_is_missing() {
        let path = unique_test_path("session-missing");
        let store = FileLocalSessionStore::new(&path);

        assert_eq!(store.local_session("session_1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn file_local_session_store_persists_and_reads_local_sessions() {
        let path = unique_test_path("session-persist");
        let store = FileLocalSessionStore::new(&path);
        let session = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-secret".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };

        store.store_local_session(session.clone()).await.unwrap();

        let persisted: LocalSessionState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.sessions, vec![session.clone()]);
        assert_eq!(
            store.local_session(&session.id).await.unwrap(),
            Some(session)
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_rewrites_existing_session_with_same_id() {
        let path = unique_test_path("session-update");
        let store = FileLocalSessionStore::new(&path);

        store
            .store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$original".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            })
            .await
            .unwrap();

        let updated = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$rotated".into(),
            created_at: "2026-04-16T19:00:00Z".into(),
        };

        store.store_local_session(updated.clone()).await.unwrap();

        let persisted: LocalSessionState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.sessions, vec![updated.clone()]);
        assert_eq!(
            store.local_session("session_1").await.unwrap(),
            Some(updated)
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_rejects_invalid_json_without_overwriting_existing_file() {
        let path = unique_test_path("session-invalid-json");
        fs::write(&path, b"{\"sessions\":").unwrap();
        let store = FileLocalSessionStore::new(&path);

        let error = store
            .store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$session-secret".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("EOF while parsing"));
        assert_eq!(fs::read(&path).unwrap(), b"{\"sessions\":".to_vec());

        fs::remove_file(path).unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn file_local_session_store_preserves_distinct_sessions_across_concurrent_writes() {
        let path = unique_test_path("session-concurrent");
        let store_a = FileLocalSessionStore::new(&path);
        let store_b = FileLocalSessionStore::new(&path);
        let session_a = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-a".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };
        let session_b = LocalSession {
            id: "session_2".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-b".into(),
            created_at: "2026-04-16T18:01:00Z".into(),
        };

        let writer_a = {
            let session = session_a.clone();
            tokio::spawn(async move { store_a.store_local_session(session).await.unwrap() })
        };
        let writer_b = {
            let session = session_b.clone();
            tokio::spawn(async move { store_b.store_local_session(session).await.unwrap() })
        };

        writer_a.await.unwrap();
        writer_b.await.unwrap();

        let mut persisted = serde_json::from_slice::<LocalSessionState>(&fs::read(&path).unwrap())
            .unwrap()
            .sessions;
        persisted.sort_by(|left, right| left.id.cmp(&right.id));

        assert_eq!(persisted, vec![session_a, session_b]);
        assert!(!path
            .with_file_name(format!(
                ".{}.lock",
                path.file_name().unwrap().to_string_lossy()
            ))
            .exists());

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_times_out_when_lock_file_is_stale() {
        let path = unique_test_path("session-stale-lock");
        let lock_path = path.with_file_name(format!(
            ".{}.lock",
            path.file_name().unwrap().to_string_lossy()
        ));
        open_new_private_file(&lock_path).unwrap();
        let store = FileLocalSessionStore::new(&path);

        let result = tokio::time::timeout(
            Duration::from_millis(150),
            store.store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$session-secret".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            }),
        )
        .await
        .expect("stale lock should fail instead of hanging forever")
        .expect_err("stale lock should return an error");

        assert!(result.to_string().contains("timed out"));
        assert!(lock_path.exists());
        assert!(!path.exists());

        fs::remove_file(lock_path).unwrap();
    }

    #[tokio::test]
    async fn file_local_session_store_deletes_only_the_requested_session() {
        let path = unique_test_path("session-delete");
        let store = FileLocalSessionStore::new(&path);
        let retained = LocalSession {
            id: "session_keep".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$keep".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };
        let deleted = LocalSession {
            id: "session_drop".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$drop".into(),
            created_at: "2026-04-16T18:01:00Z".into(),
        };

        store.store_local_session(retained.clone()).await.unwrap();
        store.store_local_session(deleted.clone()).await.unwrap();

        assert!(store.delete_local_session(&deleted.id).await.unwrap());
        assert_eq!(store.local_session(&deleted.id).await.unwrap(), None);
        assert_eq!(
            store.local_session(&retained.id).await.unwrap(),
            Some(retained.clone())
        );

        let persisted: LocalSessionState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.sessions, vec![retained]);

        fs::remove_file(path).unwrap();
    }

    async fn session_test_pool() -> sqlx::PgPool {
        let database_url = env::var("DATABASE_URL")
            .or_else(|_| env::var("TEST_DATABASE_URL"))
            .expect(
                "DATABASE_URL or TEST_DATABASE_URL must be set for Postgres session-store tests",
            );
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("connect to Postgres test database");
        catalog_migrator()
            .run(&pool)
            .await
            .expect("apply catalog migrations");
        sqlx::query(
            "TRUNCATE TABLE oauth_clients, api_keys, sessions, local_accounts, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset local session test tables");
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ($1, $2, $3)")
            .bind("org_acme")
            .bind("acme")
            .bind("Acme")
            .execute(&pool)
            .await
            .expect("seed organization");
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ($1, $2, $3, $4::timestamptz)",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .bind("admin@example.com")
        .bind("Bootstrap Admin")
        .bind("2026-04-16T17:59:00Z")
        .execute(&pool)
        .await
        .expect("seed local account");
        pool
    }

    async fn bootstrap_test_pool() -> sqlx::PgPool {
        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for Postgres bootstrap-store tests");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("connect to Postgres test database");
        catalog_migrator()
            .run(&pool)
            .await
            .expect("apply catalog migrations");
        sqlx::query(
            "TRUNCATE TABLE oauth_clients, api_keys, sessions, local_accounts, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset bootstrap test tables");
        pool
    }

    async fn persisted_bootstrap_row(
        pool: &sqlx::PgPool,
    ) -> Option<(String, String, String, String)> {
        sqlx::query(
            "SELECT email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1",
        )
        .bind(LOCAL_BOOTSTRAP_ADMIN_USER_ID)
        .fetch_optional(pool)
        .await
        .expect("read persisted bootstrap account row")
        .map(|row| {
            (
                row.get("email"),
                row.get("name"),
                row.get("password_hash"),
                row.get("created_at"),
            )
        })
    }

    async fn persisted_sessions(pool: &sqlx::PgPool) -> Vec<LocalSession> {
        let rows = sqlx::query(
            "SELECT id, user_id, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM sessions ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .expect("read persisted sessions");

        rows.into_iter()
            .map(|row| LocalSession {
                id: row.get("id"),
                user_id: row.get("user_id"),
                secret_hash: row.get("secret_hash"),
                created_at: row.get("created_at"),
            })
            .collect()
    }

    async fn org_auth_metadata_test_pool() -> sqlx::PgPool {
        let database_url = env::var("TEST_DATABASE_URL")
            .expect("TEST_DATABASE_URL must be set for Postgres org-auth metadata tests");
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .expect("connect to Postgres test database");
        catalog_migrator()
            .run(&pool)
            .await
            .expect("apply catalog migrations");
        sqlx::query(
            "TRUNCATE TABLE oauth_clients, api_keys, sessions, local_accounts, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset org auth metadata test tables");
        pool
    }

    #[tokio::test]
    async fn pg_org_auth_metadata_reads_local_account_by_email_and_id() {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ($1, $2, $3)")
            .bind("org_acme")
            .bind("acme")
            .bind("Acme")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ($1, $2, $3, $4, $5::timestamptz)",
        )
        .bind("local_user_alice")
        .bind("alice@example.com")
        .bind("Alice Admin")
        .bind("$argon2id$alice-hash")
        .bind("2026-04-22T12:00:00Z")
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };

        assert_eq!(
            store
                .local_account_by_email(" ALICE@example.com ")
                .await
                .unwrap(),
            Some(LocalAccount {
                id: "local_user_alice".into(),
                email: "alice@example.com".into(),
                name: "Alice Admin".into(),
                password_hash: Some("$argon2id$alice-hash".into()),
                created_at: "2026-04-22T12:00:00Z".into(),
            })
        );
        assert_eq!(
            store.local_account_by_id("local_user_alice").await.unwrap(),
            Some(LocalAccount {
                id: "local_user_alice".into(),
                email: "alice@example.com".into(),
                name: "Alice Admin".into(),
                password_hash: Some("$argon2id$alice-hash".into()),
                created_at: "2026-04-22T12:00:00Z".into(),
            })
        );
        assert_eq!(
            store
                .local_account_by_email("missing@example.com")
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store.local_account_by_id("missing_user").await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn pg_org_auth_metadata_fails_closed_for_case_insensitive_duplicate_local_account_emails()
    {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES
            ('local_user_alice_lower', 'alice@example.com', 'Alice Lower', '$argon2id$alice-lower', '2026-04-22T12:00:00Z'::timestamptz),
            ('local_user_alice_upper', 'ALICE@example.com', 'Alice Upper', '$argon2id$alice-upper', '2026-04-22T12:05:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };

        assert_eq!(
            store
                .local_account_by_email("alice@example.com")
                .await
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .local_account_by_email(" ALICE@example.com ")
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn pg_org_auth_metadata_lists_admin_organizations_and_membership_rosters() {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme'), ('org_tools', 'tools', 'Tools')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES
            ('local_user_admin', 'admin@example.com', 'Admin User', '2026-04-22T09:00:00Z'::timestamptz),
            ('local_user_member', 'member@example.com', 'Member User', '2026-04-22T09:05:00Z'::timestamptz),
            ('local_user_inviter', 'inviter@example.com', 'Inviter User', '2026-04-22T08:55:00Z'::timestamptz)"
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, joined_at) VALUES
            ('org_acme', 'local_user_admin', 'admin', '2026-04-22T09:10:00Z'::timestamptz),
            ('org_acme', 'local_user_member', 'viewer', '2026-04-22T09:11:00Z'::timestamptz),
            ('org_tools', 'local_user_admin', 'viewer', '2026-04-22T09:12:00Z'::timestamptz)"
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_invites (id, organization_id, email, role, invited_by_user_id, created_at, expires_at, accepted_by_user_id, accepted_at) VALUES
            ('invite_pending', 'org_acme', 'pending@example.com', 'viewer', 'local_user_inviter', '2026-04-22T10:00:00Z'::timestamptz, '2026-04-29T10:00:00Z'::timestamptz, NULL, NULL),
            ('invite_accepted', 'org_acme', 'accepted@example.com', 'admin', 'local_user_inviter', '2026-04-22T10:05:00Z'::timestamptz, '2026-04-29T10:05:00Z'::timestamptz, 'local_user_member', '2026-04-22T11:00:00Z'::timestamptz)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };
        let snapshot = store
            .admin_organization_auth_metadata("local_user_admin")
            .await
            .unwrap();

        assert_eq!(snapshot.organizations.len(), 1);
        assert_eq!(snapshot.organizations[0].id, "org_acme");
        assert_eq!(snapshot.memberships.len(), 2);
        assert_eq!(snapshot.invites.len(), 2);
        assert_eq!(
            snapshot
                .memberships
                .iter()
                .map(|membership| membership.user_id.as_str())
                .collect::<Vec<_>>(),
            vec!["local_user_admin", "local_user_member"]
        );
        assert_eq!(
            snapshot
                .accounts
                .iter()
                .map(|account| account.email.as_str())
                .collect::<Vec<_>>(),
            vec!["admin@example.com", "member@example.com"]
        );
    }

    #[tokio::test]
    async fn pg_org_auth_metadata_user_snapshot_includes_repo_permissions_for_member_organizations()
    {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme'), ('org_tools', 'tools', 'Tools')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO connections (id, name, kind) VALUES ('conn_github', 'GitHub', 'github')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state) VALUES
            ('repo_sourcebot_rewrite', 'sourcebot-rewrite', 'main', 'conn_github', 'ready'),
            ('repo_tools', 'tools', 'main', 'conn_github', 'ready')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES ('local_user_member', 'member@example.com', 'Member User', '2026-04-22T09:05:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, joined_at) VALUES
            ('org_acme', 'local_user_member', 'viewer', '2026-04-22T09:10:00Z'::timestamptz),
            ('org_tools', 'local_user_member', 'viewer', '2026-04-22T09:11:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO repository_permission_bindings (organization_id, repository_id, synced_at) VALUES
            ('org_acme', 'repo_sourcebot_rewrite', '2026-04-22T09:15:00Z'::timestamptz),
            ('org_tools', 'repo_tools', '2026-04-22T09:16:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };
        let snapshot = store
            .user_organization_auth_metadata("local_user_member")
            .await
            .unwrap()
            .expect("member auth metadata");

        assert_eq!(
            snapshot.repo_permissions,
            vec![
                RepositoryPermissionBinding {
                    organization_id: "org_acme".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    synced_at: "2026-04-22T09:15:00Z".into(),
                },
                RepositoryPermissionBinding {
                    organization_id: "org_tools".into(),
                    repository_id: "repo_tools".into(),
                    synced_at: "2026-04-22T09:16:00Z".into(),
                },
            ]
        );
    }

    #[tokio::test]
    async fn pg_org_auth_metadata_accepts_invite_and_persists_account_membership_and_invite_acceptance(
    ) {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query(
            "INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, created_at) VALUES
            ('local_user_inviter', 'inviter@example.com', 'Inviter User', '2026-04-22T08:55:00Z'::timestamptz),
            ('local_user_pending', 'invitee@example.com', '', '2026-04-22T09:30:00Z'::timestamptz)"
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_invites (id, organization_id, email, role, invited_by_user_id, created_at, expires_at, accepted_by_user_id, accepted_at) VALUES
            ('invite_pending', 'org_acme', 'invitee@example.com', 'viewer', 'local_user_inviter', '2026-04-22T10:00:00Z'::timestamptz, '2026-04-29T10:00:00Z'::timestamptz, NULL, NULL)"
        )
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };
        let accepted = store
            .redeem_invite(
                "invite_pending",
                "Invitee@example.com",
                "Invited User",
                "$argon2id$invite-hash",
                "2026-04-22T12:30:00Z",
                "local_user_generated",
            )
            .await
            .unwrap()
            .expect("invite accepted");

        assert_eq!(accepted.account.id, "local_user_pending");
        assert_eq!(accepted.account.name, "Invited User");
        assert_eq!(
            accepted.account.password_hash,
            Some("$argon2id$invite-hash".into())
        );
        assert_eq!(accepted.membership.organization_id, "org_acme");
        assert_eq!(accepted.membership.user_id, "local_user_pending");
        assert_eq!(
            accepted.invite.accepted_by_user_id,
            Some("local_user_pending".into())
        );
        assert_eq!(
            accepted.invite.accepted_at,
            Some("2026-04-22T12:30:00Z".into())
        );

        let persisted_account = sqlx::query(
            "SELECT id, email, name, password_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE email = $1",
        )
        .bind("invitee@example.com")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            persisted_account.get::<String, _>("id"),
            "local_user_pending"
        );
        assert_eq!(persisted_account.get::<String, _>("name"), "Invited User");
        assert_eq!(
            persisted_account.get::<String, _>("password_hash"),
            "$argon2id$invite-hash"
        );
        assert_eq!(
            persisted_account.get::<String, _>("created_at"),
            "2026-04-22T09:30:00Z"
        );

        let persisted_membership = sqlx::query(
            "SELECT organization_id, user_id, role, to_char(joined_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS joined_at FROM organization_memberships WHERE organization_id = $1 AND user_id = $2",
        )
        .bind("org_acme")
        .bind("local_user_pending")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(persisted_membership.get::<String, _>("role"), "viewer");
        assert_eq!(
            persisted_membership.get::<String, _>("joined_at"),
            "2026-04-22T12:30:00Z"
        );

        let persisted_invite = sqlx::query(
            "SELECT accepted_by_user_id, to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS accepted_at FROM organization_invites WHERE id = $1",
        )
        .bind("invite_pending")
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(
            persisted_invite.get::<String, _>("accepted_by_user_id"),
            "local_user_pending"
        );
        assert_eq!(
            persisted_invite.get::<String, _>("accepted_at"),
            "2026-04-22T12:30:00Z"
        );
    }

    async fn persisted_api_keys(pool: &sqlx::PgPool) -> Vec<ApiKey> {
        sqlx::query(
            "SELECT id, user_id, name, secret_hash, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, CASE WHEN revoked_at IS NULL THEN NULL ELSE to_char(revoked_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS revoked_at, repo_scope FROM api_keys ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| ApiKey {
            id: row.get("id"),
            user_id: row.get("user_id"),
            name: row.get("name"),
            secret_hash: row.get("secret_hash"),
            created_at: row.get("created_at"),
            revoked_at: row.get("revoked_at"),
            repo_scope: row.get("repo_scope"),
        })
        .collect()
    }

    async fn persisted_oauth_clients(pool: &sqlx::PgPool) -> Vec<OAuthClient> {
        sqlx::query(
            "SELECT id, organization_id, name, client_id, client_secret_hash, redirect_uris, created_by_user_id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, CASE WHEN revoked_at IS NULL THEN NULL ELSE to_char(revoked_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') END AS revoked_at FROM oauth_clients ORDER BY id",
        )
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row| OAuthClient {
            id: row.get("id"),
            organization_id: row.get("organization_id"),
            name: row.get("name"),
            client_id: row.get("client_id"),
            client_secret_hash: row.get("client_secret_hash"),
            redirect_uris: row.get("redirect_uris"),
            created_by_user_id: row.get("created_by_user_id"),
            created_at: row.get("created_at"),
            revoked_at: row.get("revoked_at"),
        })
        .collect()
    }

    #[tokio::test]
    async fn pg_api_key_lists_one_users_keys_from_postgres() {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query(
            "INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES
            ('local_user_admin', 'admin@example.com', 'Admin User', '$argon2id$admin', '2026-04-22T09:00:00Z'::timestamptz),
            ('local_user_other', 'other@example.com', 'Other User', '$argon2id$other', '2026-04-22T09:05:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO api_keys (id, user_id, name, secret_hash, created_at, revoked_at, repo_scope) VALUES
            ('api_key_active', 'local_user_admin', 'CLI key', '$argon2id$cli', '2026-04-22T10:00:00Z'::timestamptz, NULL, ARRAY['repo_sourcebot_rewrite']::text[]),
            ('api_key_revoked', 'local_user_admin', 'Revoked key', '$argon2id$revoked', '2026-04-22T10:05:00Z'::timestamptz, '2026-04-23T00:00:00Z'::timestamptz, ARRAY[]::text[]),
            ('api_key_other', 'local_user_other', 'Other key', '$argon2id$other-key', '2026-04-22T10:10:00Z'::timestamptz, NULL, ARRAY['repo_other']::text[])",
        )
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };
        let keys = store.api_keys_for_user("local_user_admin").await.unwrap();

        assert_eq!(
            keys,
            vec![
                ApiKey {
                    id: "api_key_active".into(),
                    user_id: "local_user_admin".into(),
                    name: "CLI key".into(),
                    secret_hash: "$argon2id$cli".into(),
                    created_at: "2026-04-22T10:00:00Z".into(),
                    revoked_at: None,
                    repo_scope: vec!["repo_sourcebot_rewrite".into()],
                },
                ApiKey {
                    id: "api_key_revoked".into(),
                    user_id: "local_user_admin".into(),
                    name: "Revoked key".into(),
                    secret_hash: "$argon2id$revoked".into(),
                    created_at: "2026-04-22T10:05:00Z".into(),
                    revoked_at: Some("2026-04-23T00:00:00Z".into()),
                    repo_scope: vec![],
                },
            ]
        );
    }

    #[tokio::test]
    async fn pg_api_key_authenticates_visible_repo_scope_and_revokes_durably() {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query(
            "INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme')",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES ('local_user_member', 'member@example.com', 'Member User', '$argon2id$member', '2026-04-22T09:00:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, joined_at) VALUES ('org_acme', 'local_user_member', 'viewer', '2026-04-22T09:05:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id) VALUES ('repo_sourcebot_rewrite', 'sourcebot-rewrite', 'main', 'conn_placeholder')",
        )
        .execute(&pool)
        .await
        .unwrap_err();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };
        let created = store
            .create_api_key(ApiKey {
                id: "api_key_member".into(),
                user_id: "local_user_member".into(),
                name: "Member CLI".into(),
                secret_hash: "$argon2id$member-key".into(),
                created_at: "2026-04-22T10:00:00Z".into(),
                revoked_at: None,
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
            })
            .await
            .unwrap();
        assert_eq!(created.id, "api_key_member");

        let fetched = store.api_key_by_id("api_key_member").await.unwrap();
        assert_eq!(fetched, Some(created.clone()));

        store
            .revoke_api_key(
                "api_key_member",
                "local_user_member",
                "2026-04-23T00:00:00Z",
            )
            .await
            .unwrap();

        let persisted = persisted_api_keys(&pool).await;
        assert_eq!(persisted.len(), 1);
        assert_eq!(
            persisted[0].revoked_at.as_deref(),
            Some("2026-04-23T00:00:00Z")
        );
    }

    #[tokio::test]
    async fn pg_oauth_client_lists_visible_org_clients_and_persists_redirect_uris() {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query("INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme'), ('org_hidden', 'hidden', 'Hidden')")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO local_accounts (id, email, name, password_hash, created_at) VALUES
            ('local_user_admin', 'admin@example.com', 'Admin User', '$argon2id$admin', '2026-04-22T09:00:00Z'::timestamptz),
            ('local_user_other', 'other@example.com', 'Other User', '$argon2id$other', '2026-04-22T09:05:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO organization_memberships (organization_id, user_id, role, joined_at) VALUES
            ('org_acme', 'local_user_admin', 'admin', '2026-04-22T09:10:00Z'::timestamptz),
            ('org_hidden', 'local_user_other', 'admin', '2026-04-22T09:15:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO oauth_clients (id, organization_id, name, client_id, client_secret_hash, redirect_uris, created_by_user_id, created_at, revoked_at) VALUES
            ('oauth_client_visible', 'org_acme', 'Acme Web App', 'client_visible', '$argon2id$visible', ARRAY['https://acme.example.com/callback', 'http://localhost:3000/callback']::text[], 'local_user_admin', '2026-04-22T10:00:00Z'::timestamptz, NULL),
            ('oauth_client_hidden', 'org_hidden', 'Hidden App', 'client_hidden', '$argon2id$hidden', ARRAY['https://hidden.example.com/callback']::text[], 'local_user_other', '2026-04-22T10:05:00Z'::timestamptz, NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let store = PgOrganizationAuthMetadataStore { pool: pool.clone() };
        let visible = store
            .oauth_clients_for_organizations(&["org_acme".to_string()])
            .await
            .unwrap();
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "oauth_client_visible");
        assert_eq!(
            visible[0].redirect_uris,
            vec![
                "https://acme.example.com/callback".to_string(),
                "http://localhost:3000/callback".to_string(),
            ]
        );

        let created = store
            .create_oauth_client(OAuthClient {
                id: "oauth_client_created".into(),
                organization_id: "org_acme".into(),
                name: "Acme CLI".into(),
                client_id: "client_created".into(),
                client_secret_hash: "$argon2id$created".into(),
                redirect_uris: vec![
                    "https://cli.acme.example.com/callback".into(),
                    "http://localhost:4000/callback".into(),
                ],
                created_by_user_id: "local_user_admin".into(),
                created_at: "2026-04-22T11:00:00Z".into(),
                revoked_at: None,
            })
            .await
            .unwrap();
        assert_eq!(created.id, "oauth_client_created");

        let persisted = persisted_oauth_clients(&pool).await;
        assert_eq!(persisted.len(), 3);
        assert_eq!(persisted[0].id, "oauth_client_created");
        assert_eq!(persisted[2].id, "oauth_client_visible");
        assert!(persisted.iter().any(|client| {
            client.id == "oauth_client_created"
                && client.redirect_uris
                    == vec![
                        "https://cli.acme.example.com/callback".to_string(),
                        "http://localhost:4000/callback".to_string(),
                    ]
        }));
    }

    #[tokio::test]
    async fn pg_local_session_store_persists_and_reads_local_sessions() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };
        let session = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$session-secret".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };

        store.store_local_session(session.clone()).await.unwrap();

        assert_eq!(
            store.local_session(&session.id).await.unwrap(),
            Some(session.clone())
        );
        assert_eq!(persisted_sessions(&pool).await, vec![session]);
    }

    #[tokio::test]
    async fn pg_local_session_store_persists_truthful_local_account_rows() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };

        store
            .persist_local_session_account(LocalAccount {
                id: "local_user_bootstrap_admin".into(),
                email: "admin@example.com".into(),
                name: "Admin User".into(),
                password_hash: None,
                created_at: "2026-04-16T17:00:00Z".into(),
            })
            .await
            .unwrap();

        let row = sqlx::query(
            "SELECT email, name, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at FROM local_accounts WHERE id = $1",
        )
        .bind("local_user_bootstrap_admin")
        .fetch_one(&pool)
        .await
        .expect("read persisted local account row");

        assert_eq!(row.get::<String, _>("email"), "admin@example.com");
        assert_eq!(row.get::<String, _>("name"), "Admin User");
        assert_eq!(row.get::<String, _>("created_at"), "2026-04-16T17:00:00Z");
    }

    #[test]
    fn try_build_local_session_store_rejects_invalid_database_url() {
        let result = try_build_local_session_store(
            unique_test_path("invalid-database-url"),
            Some("not a database url"),
        );

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pg_local_session_store_rewrites_existing_session_with_same_id() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };

        store
            .store_local_session(LocalSession {
                id: "session_1".into(),
                user_id: "local_user_bootstrap_admin".into(),
                secret_hash: "$argon2id$original".into(),
                created_at: "2026-04-16T18:00:00Z".into(),
            })
            .await
            .unwrap();

        let updated = LocalSession {
            id: "session_1".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$rotated".into(),
            created_at: "2026-04-16T19:00:00Z".into(),
        };

        store.store_local_session(updated.clone()).await.unwrap();

        assert_eq!(
            store.local_session("session_1").await.unwrap(),
            Some(updated.clone())
        );
        assert_eq!(persisted_sessions(&pool).await, vec![updated]);
    }

    #[tokio::test]
    async fn pg_local_session_store_deletes_only_requested_session() {
        let pool = session_test_pool().await;
        let store = PgLocalSessionStore { pool: pool.clone() };
        let retained = LocalSession {
            id: "session_keep".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$keep".into(),
            created_at: "2026-04-16T18:00:00Z".into(),
        };
        let deleted = LocalSession {
            id: "session_drop".into(),
            user_id: "local_user_bootstrap_admin".into(),
            secret_hash: "$argon2id$drop".into(),
            created_at: "2026-04-16T18:01:00Z".into(),
        };

        store.store_local_session(retained.clone()).await.unwrap();
        store.store_local_session(deleted.clone()).await.unwrap();

        assert!(store.delete_local_session(&deleted.id).await.unwrap());
        assert_eq!(store.local_session(&deleted.id).await.unwrap(), None);
        assert_eq!(
            store.local_session(&retained.id).await.unwrap(),
            Some(retained.clone())
        );
        assert_eq!(persisted_sessions(&pool).await, vec![retained]);
    }

    #[tokio::test]
    async fn file_organization_store_persists_and_reads_organization_state() {
        let path = unique_test_path("organization-persist");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            organizations: vec![Organization {
                id: "org_acme".into(),
                name: "Acme".into(),
                slug: "acme".into(),
            }],
            connections: vec![Connection {
                id: "conn_github".into(),
                name: "GitHub Cloud".into(),
                kind: ConnectionKind::GitHub,
                config: Some(ConnectionConfig::GitHub {
                    base_url: "https://github.com".into(),
                }),
            }],
            memberships: vec![OrganizationMembership {
                organization_id: "org_acme".into(),
                user_id: "local_user_bootstrap_admin".into(),
                role: OrganizationRole::Admin,
                joined_at: "2026-04-16T20:00:00Z".into(),
            }],
            accounts: vec![LocalAccount {
                id: "local_user_bootstrap_admin".into(),
                email: "admin@example.com".into(),
                name: "Bootstrap Admin".into(),
                password_hash: None,
                created_at: "2026-04-16T19:58:00Z".into(),
            }],
            invites: vec![OrganizationInvite {
                id: "invite_member".into(),
                organization_id: "org_acme".into(),
                email: "member@example.com".into(),
                role: OrganizationRole::Viewer,
                invited_by_user_id: "local_user_bootstrap_admin".into(),
                created_at: "2026-04-16T20:05:00Z".into(),
                expires_at: "2026-04-23T20:05:00Z".into(),
                accepted_by_user_id: Some("local_user_member".into()),
                accepted_at: Some("2026-04-17T08:00:00Z".into()),
            }],
            api_keys: vec![ApiKey {
                id: "key_ci".into(),
                user_id: "local_user_bootstrap_admin".into(),
                name: "CI key".into(),
                secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$ci$hash".into(),
                created_at: "2026-04-18T09:45:00Z".into(),
                revoked_at: Some("2026-04-19T09:45:00Z".into()),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
            }],
            oauth_clients: vec![OAuthClient {
                id: "oauth_client_acme_web".into(),
                organization_id: "org_acme".into(),
                name: "Acme Web App".into(),
                client_id: "acme-web-client".into(),
                client_secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$oauth$hash".into(),
                redirect_uris: vec!["https://app.acme.test/callback".into()],
                created_by_user_id: "local_user_bootstrap_admin".into(),
                created_at: "2026-04-18T09:46:00Z".into(),
                revoked_at: Some("2026-04-20T09:46:00Z".into()),
            }],
            search_contexts: vec![SearchContext {
                id: "ctx_backend".into(),
                user_id: "local_user_bootstrap_admin".into(),
                name: "Backend repos".into(),
                repo_scope: vec!["repo_sourcebot_rewrite".into()],
                created_at: "2026-04-18T09:50:00Z".into(),
                updated_at: "2026-04-19T09:50:00Z".into(),
            }],
            audit_events: vec![AuditEvent {
                id: "audit_key_ci_created".into(),
                organization_id: "org_acme".into(),
                actor: AuditActor {
                    user_id: Some("local_user_bootstrap_admin".into()),
                    api_key_id: Some("key_ci".into()),
                },
                action: "auth.api_key.created".into(),
                target_type: "api_key".into(),
                target_id: "key_ci".into(),
                occurred_at: "2026-04-18T09:45:00Z".into(),
                metadata: serde_json::json!({
                    "name": "CI key",
                    "repo_scope": ["repo_sourcebot_rewrite"]
                }),
            }],
            analytics_records: vec![AnalyticsRecord {
                id: "analytics_api_key_count".into(),
                organization_id: "org_acme".into(),
                metric: "auth.api_key.count".into(),
                recorded_at: "2026-04-19T10:00:00Z".into(),
                value: serde_json::json!({
                    "count": 1
                }),
                dimensions: serde_json::json!({
                    "source": "migration_seed"
                }),
            }],
            review_webhooks: vec![ReviewWebhook {
                id: "webhook_review_1".into(),
                organization_id: "org_acme".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                events: vec!["pull_request".into()],
                secret_hash: "$argon2id$v=19$m=19456,t=2,p=1$review$hash".into(),
                created_by_user_id: "local_user_bootstrap_admin".into(),
                created_at: "2026-04-19T10:05:00Z".into(),
            }],
            review_webhook_delivery_attempts: vec![ReviewWebhookDeliveryAttempt {
                id: "delivery_attempt_1".into(),
                webhook_id: "webhook_review_1".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                event_type: "pull_request_review".into(),
                review_id: "review_123".into(),
                external_event_id: "evt_123".into(),
                accepted_at: "2026-04-25T00:10:00Z".into(),
            }],
            review_agent_runs: vec![ReviewAgentRun {
                id: "review_agent_run_1".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_1".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_123".into(),
                status: ReviewAgentRunStatus::Queued,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            repo_permissions: vec![RepositoryPermissionBinding {
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                synced_at: "2026-04-18T09:30:00Z".into(),
            }],
            repository_sync_jobs: vec![RepositorySyncJob {
                id: "sync_job_1".into(),
                organization_id: "org_acme".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                connection_id: "conn_github".into(),
                status: RepositorySyncJobStatus::Failed,
                queued_at: "2026-04-26T10:00:00Z".into(),
                started_at: Some("2026-04-26T10:01:00Z".into()),
                finished_at: Some("2026-04-26T10:02:00Z".into()),
                error: Some("remote rejected fetch".into()),
            }],
        };

        store.store_organization_state(state.clone()).await.unwrap();

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted, state);
        assert_eq!(store.organization_state().await.unwrap(), state);

        fs::remove_file(path).unwrap();
    }

    fn repository_sync_job(
        id: &str,
        status: RepositorySyncJobStatus,
        queued_at: &str,
    ) -> RepositorySyncJob {
        RepositorySyncJob {
            id: id.into(),
            organization_id: "org_acme".into(),
            repository_id: format!("repo_{id}"),
            connection_id: "conn_github".into(),
            status,
            queued_at: queued_at.into(),
            started_at: None,
            finished_at: None,
            error: None,
        }
    }

    async fn repository_sync_job_test_pool() -> sqlx::PgPool {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query(
            "TRUNCATE TABLE repository_sync_jobs, repository_permission_bindings, repositories, connections, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset repository sync job test tables");
        sqlx::query(
            "INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme')",
        )
        .execute(&pool)
        .await
        .expect("insert organization");
        sqlx::query(
            "INSERT INTO connections (id, name, kind) VALUES ('conn_github', 'GitHub', 'github')",
        )
        .execute(&pool)
        .await
        .expect("insert connection");
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state) VALUES
            ('repo_sync_job_queued', 'queued', 'main', 'conn_github', 'ready'),
            ('repo_sync_job_updated', 'updated', 'main', 'conn_github', 'ready'),
            ('repo_sync_job_oldest', 'oldest', 'main', 'conn_github', 'ready'),
            ('repo_sync_job_newer', 'newer', 'main', 'conn_github', 'ready')",
        )
        .execute(&pool)
        .await
        .expect("insert repositories");
        pool
    }

    #[tokio::test]
    async fn pg_organization_store_upserts_repository_sync_jobs_and_merges_them_into_state() {
        let pool = repository_sync_job_test_pool().await;
        let path = unique_test_path("pg-organization-store-upsert-repository-sync-job");
        let store = PgOrganizationStore::new(FileOrganizationStore::new(&path), pool.clone());
        store
            .store_organization_state(OrganizationState {
                organizations: vec![Organization {
                    id: "org_acme".into(),
                    slug: "acme".into(),
                    name: "Acme".into(),
                }],
                ..OrganizationState::default()
            })
            .await
            .unwrap();
        let queued = repository_sync_job(
            "sync_job_queued",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:00:00Z",
        );
        let updated = RepositorySyncJob {
            status: RepositorySyncJobStatus::Succeeded,
            started_at: Some("2026-04-26T10:01:00Z".into()),
            finished_at: Some("2026-04-26T10:02:00Z".into()),
            ..queued.clone()
        };

        store.store_repository_sync_job(queued).await.unwrap();
        store
            .store_repository_sync_job(updated.clone())
            .await
            .unwrap();

        let state = store.organization_state().await.unwrap();
        assert_eq!(state.organizations.len(), 1);
        assert_eq!(state.repository_sync_jobs, vec![updated]);
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn pg_organization_store_claims_oldest_queued_repository_sync_job_atomically() {
        let pool = repository_sync_job_test_pool().await;
        let path = unique_test_path("pg-organization-store-claim-repository-sync-job");
        let store = PgOrganizationStore::new(FileOrganizationStore::new(&path), pool.clone());
        let older = repository_sync_job(
            "sync_job_oldest",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );
        let newer = repository_sync_job(
            "sync_job_newer",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:02:00Z",
        );
        store.store_repository_sync_job(newer).await.unwrap();
        store.store_repository_sync_job(older).await.unwrap();

        let claimed = store
            .claim_next_repository_sync_job("2026-04-26T10:03:00Z")
            .await
            .unwrap()
            .expect("claim queued job");

        assert_eq!(claimed.id, "sync_job_oldest");
        assert_eq!(claimed.status, RepositorySyncJobStatus::Running);
        assert_eq!(claimed.started_at.as_deref(), Some("2026-04-26T10:03:00Z"));
        let state = store.organization_state().await.unwrap();
        assert_eq!(state.repository_sync_jobs[0], claimed);
        assert_eq!(state.repository_sync_jobs[1].id, "sync_job_newer");
        assert_eq!(
            state.repository_sync_jobs[1].status,
            RepositorySyncJobStatus::Queued
        );
        fs::remove_file(path).unwrap_err();
    }

    #[tokio::test]
    async fn pg_organization_store_claim_and_complete_persists_only_completed_state() {
        let pool = repository_sync_job_test_pool().await;
        let path = unique_test_path("pg-organization-store-claim-complete-repository-sync-job");
        let store = PgOrganizationStore::new(FileOrganizationStore::new(&path), pool.clone());
        store
            .store_repository_sync_job(repository_sync_job(
                "sync_job_queued",
                RepositorySyncJobStatus::Queued,
                "2026-04-26T10:00:00Z",
            ))
            .await
            .unwrap();

        let completed = store
            .claim_and_complete_next_repository_sync_job("2026-04-26T10:01:00Z", |job| {
                RepositorySyncJob {
                    status: RepositorySyncJobStatus::Failed,
                    started_at: Some("2026-04-26T10:01:00Z".into()),
                    finished_at: Some("2026-04-26T10:02:00Z".into()),
                    error: Some("fetch failed".into()),
                    ..job
                }
            })
            .await
            .unwrap()
            .expect("claim and complete queued job");

        assert_eq!(completed.status, RepositorySyncJobStatus::Failed);
        assert_eq!(completed.error.as_deref(), Some("fetch failed"));
        assert_eq!(
            store
                .organization_state()
                .await
                .unwrap()
                .repository_sync_jobs,
            vec![completed]
        );
        let running_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM repository_sync_jobs WHERE status = 'running'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(running_count, 0);
        fs::remove_file(path).unwrap_err();
    }

    #[tokio::test]
    async fn pg_organization_store_preserves_completed_repository_sync_job_when_stale_state_is_written(
    ) {
        let pool = repository_sync_job_test_pool().await;
        let path = unique_test_path("pg-organization-store-preserve-completed-repository-sync-job");
        let store = PgOrganizationStore::new(FileOrganizationStore::new(&path), pool.clone());
        let queued = repository_sync_job(
            "sync_job_queued",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:00:00Z",
        );
        let completed = RepositorySyncJob {
            status: RepositorySyncJobStatus::Succeeded,
            started_at: Some("2026-04-26T10:01:00Z".into()),
            finished_at: Some("2026-04-26T10:02:00Z".into()),
            ..queued.clone()
        };

        store
            .store_repository_sync_job(completed.clone())
            .await
            .unwrap();
        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![queued],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        assert_eq!(
            store
                .organization_state()
                .await
                .unwrap()
                .repository_sync_jobs,
            vec![completed]
        );
        fs::remove_file(path).unwrap();
    }

    fn review_agent_run(
        id: &str,
        status: ReviewAgentRunStatus,
        created_at: &str,
    ) -> ReviewAgentRun {
        ReviewAgentRun {
            id: id.into(),
            organization_id: "org_acme".into(),
            webhook_id: "webhook_review_1".into(),
            delivery_attempt_id: format!("delivery_attempt_{id}"),
            connection_id: "conn_github".into(),
            repository_id: "repo_sourcebot_rewrite".into(),
            review_id: format!("review_{id}"),
            status,
            created_at: created_at.into(),
        }
    }

    async fn review_agent_run_test_pool() -> sqlx::PgPool {
        let pool = org_auth_metadata_test_pool().await;
        sqlx::query(
            "TRUNCATE TABLE review_agent_runs, repository_permission_bindings, repositories, connections, organizations RESTART IDENTITY CASCADE",
        )
        .execute(&pool)
        .await
        .expect("reset review agent run test tables");
        sqlx::query(
            "INSERT INTO organizations (id, slug, name) VALUES ('org_acme', 'acme', 'Acme')",
        )
        .execute(&pool)
        .await
        .expect("insert organization");
        sqlx::query(
            "INSERT INTO connections (id, name, kind) VALUES ('conn_github', 'GitHub', 'github')",
        )
        .execute(&pool)
        .await
        .expect("insert connection");
        sqlx::query(
            "INSERT INTO repositories (id, name, default_branch, connection_id, sync_state) VALUES ('repo_sourcebot_rewrite', 'sourcebot-rewrite', 'main', 'conn_github', 'ready')",
        )
        .execute(&pool)
        .await
        .expect("insert repository");
        sqlx::query(
            "INSERT INTO repository_permission_bindings (organization_id, repository_id, synced_at) VALUES ('org_acme', 'repo_sourcebot_rewrite', '2026-04-26T10:00:00Z'::timestamptz)",
        )
        .execute(&pool)
        .await
        .expect("insert repository permission binding");
        pool
    }

    #[tokio::test]
    async fn pg_organization_store_review_agent_runs_store_and_merge_postgres_runs() {
        let pool = review_agent_run_test_pool().await;
        let path = unique_test_path("pg-organization-store-review-agent-runs-state");
        let store = PgOrganizationStore::new(FileOrganizationStore::new(&path), pool.clone());
        let fallback_run = review_agent_run(
            "run_fallback",
            ReviewAgentRunStatus::Queued,
            "2026-04-25T00:09:00Z",
        );
        FileOrganizationStore::new(&path)
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![fallback_run],
                organizations: vec![Organization {
                    id: "org_acme".into(),
                    slug: "fallback".into(),
                    name: "Fallback".into(),
                }],
                ..OrganizationState::default()
            })
            .await
            .unwrap();
        let pg_run = review_agent_run(
            "run_postgres",
            ReviewAgentRunStatus::Claimed,
            "2026-04-25T00:10:00Z",
        );
        sqlx::query(
            "INSERT INTO review_agent_runs (id, organization_id, webhook_id, delivery_attempt_id, connection_id, repository_id, review_id, status, created_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'claimed', $8::timestamptz)",
        )
        .bind(&pg_run.id)
        .bind(&pg_run.organization_id)
        .bind(&pg_run.webhook_id)
        .bind(&pg_run.delivery_attempt_id)
        .bind(&pg_run.connection_id)
        .bind(&pg_run.repository_id)
        .bind(&pg_run.review_id)
        .bind(&pg_run.created_at)
        .execute(&pool)
        .await
        .unwrap();

        let state = store.organization_state().await.unwrap();
        assert_eq!(state.organizations[0].slug, "fallback");
        assert_eq!(state.review_agent_runs, vec![pg_run.clone()]);

        let stored_run = review_agent_run(
            "run_stored",
            ReviewAgentRunStatus::Queued,
            "2026-04-25T00:11:00Z",
        );
        store
            .store_organization_state(OrganizationState {
                organizations: state.organizations,
                review_agent_runs: vec![stored_run.clone()],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let persisted_file: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(persisted_file.review_agent_runs.is_empty());
        assert_eq!(
            store.organization_state().await.unwrap().review_agent_runs,
            vec![pg_run, stored_run]
        );
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn pg_organization_store_review_agent_runs_claim_complete_fail_lifecycle_and_preserve_terminal(
    ) {
        let pool = review_agent_run_test_pool().await;
        let path = unique_test_path("pg-organization-store-review-agent-runs-lifecycle");
        let store = PgOrganizationStore::new(FileOrganizationStore::new(&path), pool.clone());
        let newer = review_agent_run(
            "run_newer",
            ReviewAgentRunStatus::Queued,
            "2026-04-25T00:10:06Z",
        );
        let older = review_agent_run(
            "run_oldest",
            ReviewAgentRunStatus::Queued,
            "2026-04-25T00:10:05Z",
        );
        let claimed_for_fail = review_agent_run(
            "run_claimed_for_fail",
            ReviewAgentRunStatus::Claimed,
            "2026-04-25T00:10:04Z",
        );
        store
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![newer.clone(), older.clone(), claimed_for_fail.clone()],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let claimed = store
            .claim_next_review_agent_run()
            .await
            .unwrap()
            .expect("claim queued review agent run");
        assert_eq!(claimed.id, "run_oldest");
        assert_eq!(claimed.status, ReviewAgentRunStatus::Claimed);
        assert_eq!(
            store.complete_review_agent_run("run_newer").await.unwrap(),
            None
        );
        assert_eq!(
            store.complete_review_agent_run("missing").await.unwrap(),
            None
        );

        let completed = store
            .complete_review_agent_run("run_oldest")
            .await
            .unwrap()
            .expect("complete claimed run");
        assert_eq!(completed.status, ReviewAgentRunStatus::Completed);
        assert_eq!(
            store.fail_review_agent_run("run_oldest").await.unwrap(),
            None
        );

        let failed = store
            .fail_review_agent_run("run_claimed_for_fail")
            .await
            .unwrap()
            .expect("fail claimed run");
        assert_eq!(failed.status, ReviewAgentRunStatus::Failed);

        store
            .store_organization_state(OrganizationState {
                review_agent_runs: vec![
                    ReviewAgentRun {
                        status: ReviewAgentRunStatus::Queued,
                        ..completed.clone()
                    },
                    ReviewAgentRun {
                        status: ReviewAgentRunStatus::Completed,
                        ..failed.clone()
                    },
                ],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let runs = store.organization_state().await.unwrap().review_agent_runs;
        assert_eq!(
            runs.iter()
                .find(|run| run.id == "run_oldest")
                .map(|run| &run.status),
            Some(&ReviewAgentRunStatus::Completed)
        );
        assert_eq!(
            runs.iter()
                .find(|run| run.id == "run_claimed_for_fail")
                .map(|run| &run.status),
            Some(&ReviewAgentRunStatus::Failed)
        );
        assert_eq!(
            runs.iter()
                .find(|run| run.id == "run_newer")
                .map(|run| &run.status),
            Some(&ReviewAgentRunStatus::Queued)
        );
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_creates_new_repository_sync_job() {
        let path = unique_test_path("organization-store-new-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        let job = repository_sync_job(
            "sync_job_1",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:00:00Z",
        );

        store.store_repository_sync_job(job.clone()).await.unwrap();

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.repository_sync_jobs, vec![job.clone()]);
        assert_eq!(store.organization_state().await.unwrap(), persisted);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_updates_repository_sync_job_in_place_by_id() {
        let path = unique_test_path("organization-store-update-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        let original = repository_sync_job(
            "sync_job_1",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:00:00Z",
        );
        let updated = RepositorySyncJob {
            status: RepositorySyncJobStatus::Succeeded,
            started_at: Some("2026-04-26T10:01:00Z".into()),
            finished_at: Some("2026-04-26T10:02:00Z".into()),
            ..original.clone()
        };

        store.store_repository_sync_job(original).await.unwrap();
        store
            .store_repository_sync_job(updated.clone())
            .await
            .unwrap();

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs, vec![updated]);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_unrelated_repository_sync_jobs_when_upserting() {
        let path = unique_test_path("organization-store-preserve-unrelated-repository-sync-jobs");
        let store = FileOrganizationStore::new(&path);
        let retained = repository_sync_job(
            "sync_job_keep",
            RepositorySyncJobStatus::Failed,
            "2026-04-26T10:00:00Z",
        );
        let original = repository_sync_job(
            "sync_job_update",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );
        let updated = RepositorySyncJob {
            status: RepositorySyncJobStatus::Running,
            started_at: Some("2026-04-26T10:02:00Z".into()),
            ..original.clone()
        };

        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![retained.clone(), original],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        store
            .store_repository_sync_job(updated.clone())
            .await
            .unwrap();

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs, vec![retained, updated]);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_claims_oldest_queued_repository_sync_job_and_persists_running_status(
    ) {
        let path = unique_test_path("organization-store-claim-repository-sync-job");
        let store = FileOrganizationStore::new(&path);
        let retained_running = RepositorySyncJob {
            status: RepositorySyncJobStatus::Running,
            queued_at: "2026-04-26T10:00:00Z".into(),
            started_at: Some("2026-04-26T10:01:00Z".into()),
            ..repository_sync_job(
                "sync_job_running",
                RepositorySyncJobStatus::Running,
                "2026-04-26T10:00:00Z",
            )
        };
        let queued_newer = repository_sync_job(
            "sync_job_newer",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:02:00Z",
        );
        let queued_oldest = repository_sync_job(
            "sync_job_oldest",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );

        store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![retained_running.clone(), queued_newer, queued_oldest],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let claimed_job = store
            .claim_next_repository_sync_job("2026-04-26T10:03:00Z")
            .await
            .unwrap()
            .expect("queued repository sync job to be claimed");

        assert_eq!(claimed_job.id, "sync_job_oldest");
        assert_eq!(claimed_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(
            claimed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );
        assert_eq!(claimed_job.finished_at, None);
        assert_eq!(claimed_job.error, None);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs.len(), 3);
        assert_eq!(persisted.repository_sync_jobs[0], retained_running);
        assert_eq!(
            persisted.repository_sync_jobs[1].status,
            RepositorySyncJobStatus::Queued
        );
        assert_eq!(persisted.repository_sync_jobs[1].started_at, None);
        assert_eq!(persisted.repository_sync_jobs[2], claimed_job);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_running_repository_sync_job_when_stale_state_is_written(
    ) {
        let path = unique_test_path("organization-store-preserves-running-repository-sync-job");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);

        writer_store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![repository_sync_job(
                    "sync_job_queued",
                    RepositorySyncJobStatus::Queued,
                    "2026-04-26T10:01:00Z",
                )],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let claimed_job = worker_store
            .claim_next_repository_sync_job("2026-04-26T10:03:00Z")
            .await
            .unwrap()
            .expect("queued repository sync job to be claimed");
        assert_eq!(claimed_job.status, RepositorySyncJobStatus::Running);
        assert_eq!(
            claimed_job.started_at.as_deref(),
            Some("2026-04-26T10:03:00Z")
        );

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.repository_sync_jobs.len(), 1);
        assert_eq!(persisted.repository_sync_jobs[0], claimed_job);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_new_repository_sync_job_when_stale_state_is_written()
    {
        let path = unique_test_path("organization-store-preserves-new-repository-sync-job");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);

        let original_job = repository_sync_job(
            "sync_job_original",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:01:00Z",
        );
        writer_store
            .store_organization_state(OrganizationState {
                repository_sync_jobs: vec![original_job.clone()],
                ..OrganizationState::default()
            })
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let inserted_job = repository_sync_job(
            "sync_job_inserted",
            RepositorySyncJobStatus::Queued,
            "2026-04-26T10:02:00Z",
        );
        worker_store
            .store_repository_sync_job(inserted_job.clone())
            .await
            .unwrap();

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(
            persisted.repository_sync_jobs,
            vec![original_job, inserted_job]
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_claims_one_oldest_queued_review_agent_run() {
        let path = unique_test_path("organization-claim-review-agent-run");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            review_agent_runs: vec![
                ReviewAgentRun {
                    id: "run_newer".into(),
                    organization_id: "org_acme".into(),
                    webhook_id: "webhook_review_1".into(),
                    delivery_attempt_id: "delivery_attempt_newer".into(),
                    connection_id: "conn_github".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    review_id: "review_newer".into(),
                    status: ReviewAgentRunStatus::Queued,
                    created_at: "2026-04-25T00:10:06Z".into(),
                },
                ReviewAgentRun {
                    id: "run_oldest".into(),
                    organization_id: "org_acme".into(),
                    webhook_id: "webhook_review_1".into(),
                    delivery_attempt_id: "delivery_attempt_oldest".into(),
                    connection_id: "conn_github".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    review_id: "review_oldest".into(),
                    status: ReviewAgentRunStatus::Queued,
                    created_at: "2026-04-25T00:10:05Z".into(),
                },
                ReviewAgentRun {
                    id: "run_already_claimed".into(),
                    organization_id: "org_acme".into(),
                    webhook_id: "webhook_review_1".into(),
                    delivery_attempt_id: "delivery_attempt_claimed".into(),
                    connection_id: "conn_github".into(),
                    repository_id: "repo_sourcebot_rewrite".into(),
                    review_id: "review_claimed".into(),
                    status: ReviewAgentRunStatus::Claimed,
                    created_at: "2026-04-25T00:10:04Z".into(),
                },
            ],
            ..OrganizationState::default()
        };

        store.store_organization_state(state).await.unwrap();

        let claimed_run = store
            .claim_next_review_agent_run()
            .await
            .unwrap()
            .expect("queued run to be claimed");

        assert_eq!(claimed_run.id, "run_oldest");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);

        let persisted: OrganizationState =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 3);
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Queued
        );
        assert_eq!(
            persisted.review_agent_runs[1].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(
            persisted.review_agent_runs[2].status,
            ReviewAgentRunStatus::Claimed
        );
        assert_eq!(store.organization_state().await.unwrap(), persisted);

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_claimed_review_agent_run_when_stale_state_is_written(
    ) {
        let path = unique_test_path("organization-store-preserves-claimed-review-agent-run");
        let writer_store = FileOrganizationStore::new(&path);
        let claimer_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_oldest".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_oldest".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_oldest".into(),
                status: ReviewAgentRunStatus::Queued,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let claimed_run = claimer_store
            .claim_next_review_agent_run()
            .await
            .unwrap()
            .expect("queued run to be claimed");
        assert_eq!(claimed_run.status, ReviewAgentRunStatus::Claimed);

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_oldest");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Claimed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_completes_a_claimed_review_agent_run() {
        let path = unique_test_path("organization-complete-review-agent-run");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        store.store_organization_state(state).await.unwrap();

        let completed_run = store
            .complete_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be completed");

        assert_eq!(completed_run.id, "run_claimed");
        assert_eq!(completed_run.status, ReviewAgentRunStatus::Completed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Completed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_fails_a_claimed_review_agent_run() {
        let path = unique_test_path("organization-fail-review-agent-run");
        let store = FileOrganizationStore::new(&path);
        let state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        store.store_organization_state(state).await.unwrap();

        let failed_run = store
            .fail_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be failed");

        assert_eq!(failed_run.id, "run_claimed");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        let persisted = store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_completed_review_agent_run_when_stale_state_is_written(
    ) {
        let path = unique_test_path("organization-store-preserves-completed-review-agent-run");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let completed_run = worker_store
            .complete_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be completed");
        assert_eq!(completed_run.status, ReviewAgentRunStatus::Completed);

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Completed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_failed_review_agent_run_when_stale_state_is_written()
    {
        let path = unique_test_path("organization-store-preserves-failed-review-agent-run");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_claimed".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_claimed".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_claimed".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let stale_state = writer_store.organization_state().await.unwrap();

        let failed_run = worker_store
            .fail_review_agent_run("run_claimed")
            .await
            .unwrap()
            .expect("claimed run to be failed");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_claimed");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn file_organization_store_preserves_persisted_failed_run_against_stale_completed_write()
    {
        let path = unique_test_path("organization-store-preserves-failed-over-completed");
        let writer_store = FileOrganizationStore::new(&path);
        let worker_store = FileOrganizationStore::new(&path);
        let initial_state = OrganizationState {
            review_agent_runs: vec![ReviewAgentRun {
                id: "run_terminal".into(),
                organization_id: "org_acme".into(),
                webhook_id: "webhook_review_1".into(),
                delivery_attempt_id: "delivery_attempt_terminal".into(),
                connection_id: "conn_github".into(),
                repository_id: "repo_sourcebot_rewrite".into(),
                review_id: "review_terminal".into(),
                status: ReviewAgentRunStatus::Claimed,
                created_at: "2026-04-25T00:10:05Z".into(),
            }],
            ..OrganizationState::default()
        };

        writer_store
            .store_organization_state(initial_state)
            .await
            .unwrap();

        let mut stale_state = writer_store.organization_state().await.unwrap();

        let failed_run = worker_store
            .fail_review_agent_run("run_terminal")
            .await
            .unwrap()
            .expect("claimed run to be failed");
        assert_eq!(failed_run.status, ReviewAgentRunStatus::Failed);

        stale_state.review_agent_runs[0].status = ReviewAgentRunStatus::Completed;
        writer_store
            .store_organization_state(stale_state)
            .await
            .unwrap();

        let persisted = writer_store.organization_state().await.unwrap();
        assert_eq!(persisted.review_agent_runs.len(), 1);
        assert_eq!(persisted.review_agent_runs[0].id, "run_terminal");
        assert_eq!(
            persisted.review_agent_runs[0].status,
            ReviewAgentRunStatus::Failed
        );

        fs::remove_file(path).unwrap();
    }
}
