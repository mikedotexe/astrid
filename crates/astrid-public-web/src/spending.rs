//! Conservative, shared gross-cost accounting. A reserved request is never
//! refunded automatically: a timeout or crash cannot establish non-billing.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::Path;

use astrid_minime_protocol::public_web::{BRAVE_REQUEST_MICROUSD, BraveSearchPolicyV1};
use chrono::{Datelike as _, Utc};
use serde::{Deserialize, Serialize};

use crate::{PublicWebErrorCode as E, Result, digest, guard};

const SCHEMA: &str = "astrid.brave_account.v1";
const STATE_FILE: &str = "spending.json";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SpendingState {
    schema: String,
    account_id: String,
    monthly_limit_microusd: u64,
    request_cost_microusd: u64,
    last_month: String,
    months: BTreeMap<String, u64>,
}

#[derive(Debug, Serialize)]
pub struct SearchAccountStatus {
    pub schema: &'static str,
    pub month_utc: String,
    pub reserved_requests: u64,
    pub reserved_microusd: u64,
    pub monthly_limit_microusd: u64,
    pub scope: &'static str,
}

fn month_now() -> String {
    let now = Utc::now();
    format!("{:04}-{:02}", now.year(), now.month())
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 256 && id.trim() == id && !id.chars().any(char::is_control)
}

fn lease(path: &Path) -> Result<guard::OwnerLease> {
    guard::OwnerLease::acquire(path, &digest(b"brave-account-spending-v1"))
}

fn load(path: &Path, id: &str) -> Result<SpendingState> {
    let bytes = guard::read_private(&path.join(STATE_FILE)).map_err(|_| E::SearchSpendingState)?;
    let account: SpendingState =
        serde_json::from_slice(&bytes).map_err(|_| E::SearchSpendingState)?;
    if guard::read_private(&path.join("initialized")).map_err(|_| E::SearchSpendingState)?
        != b"astrid.brave_account.v1\n"
    {
        return Err(E::SearchSpendingState);
    }
    if account.schema != SCHEMA
        || account.account_id != id
        || account.request_cost_microusd != BRAVE_REQUEST_MICROUSD
        || account.monthly_limit_microusd < BRAVE_REQUEST_MICROUSD
        || account.months.is_empty()
        || account.months.keys().any(|month| !valid_month(month))
        || account
            .months
            .last_key_value()
            .is_none_or(|(month, _)| month != &account.last_month)
        || account.months.values().any(|count| {
            count
                .checked_mul(BRAVE_REQUEST_MICROUSD)
                .is_none_or(|cost| cost > account.monthly_limit_microusd)
        })
    {
        return Err(E::SearchSpendingState);
    }
    Ok(account)
}

fn valid_month(value: &str) -> bool {
    value.len() == 7
        && value.as_bytes().get(4) == Some(&b'-')
        && value
            .get(..4)
            .is_some_and(|year| year.bytes().all(|b| b.is_ascii_digit()))
        && value
            .get(5..)
            .and_then(|month| month.parse::<u8>().ok())
            .is_some_and(|month| (1..=12).contains(&month))
}

fn persist(path: &Path, account: &SpendingState, create: bool) -> Result<()> {
    let bytes = serde_json::to_vec(account).map_err(|_| E::SearchSpendingState)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > guard::MAX_AUTHORITY_BYTES {
        return Err(E::SearchSpendingState);
    }
    let mut temp = tempfile::NamedTempFile::new_in(path).map_err(|_| E::SearchSpendingState)?;
    temp.write_all(&bytes).map_err(|_| E::SearchSpendingState)?;
    temp.as_file()
        .sync_all()
        .map_err(|_| E::SearchSpendingState)?;
    if create {
        temp.persist_noclobber(path.join(STATE_FILE))
            .map_err(|_| E::SearchSpendingState)?;
    } else {
        temp.persist(path.join(STATE_FILE))
            .map_err(|_| E::SearchSpendingState)?;
    }
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| E::SearchSpendingState)
}

/// Explicit local operator initialization. Never called by a fetch. Existing
/// accounting is verified, not reset; no free credits are assumed.
///
/// # Errors
/// Rejects conflicting configuration, unsafe storage or invalid history.
pub fn initialize_search_account(
    path: &Path,
    account_id: &str,
    limit: u64,
) -> Result<SearchAccountStatus> {
    if !valid_id(account_id) || limit < BRAVE_REQUEST_MICROUSD {
        return Err(E::InvalidConfig);
    }
    let _lease = lease(path)?;
    match std::fs::symlink_metadata(path.join(STATE_FILE)) {
        Ok(_) => {
            if load(path, account_id)?.monthly_limit_microusd != limit {
                return Err(E::SearchSpendingState);
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            // A durable marker prevents a missing state from becoming a fresh allowance.
            let marker = path.join("initialized");
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(marker)
                .map_err(|_| E::SearchSpendingState)?;
            file.write_all(b"astrid.brave_account.v1\n")
                .map_err(|_| E::SearchSpendingState)?;
            file.sync_all().map_err(|_| E::SearchSpendingState)?;
            File::open(path)
                .and_then(|dir| dir.sync_all())
                .map_err(|_| E::SearchSpendingState)?;
            let month = month_now();
            persist(
                path,
                &SpendingState {
                    schema: SCHEMA.into(),
                    account_id: account_id.into(),
                    monthly_limit_microusd: limit,
                    request_cost_microusd: BRAVE_REQUEST_MICROUSD,
                    last_month: month.clone(),
                    months: BTreeMap::from([(month, 0)]),
                },
                true,
            )?;
        },
        Err(_) => return Err(E::SearchSpendingState),
    }
    status_at(path, account_id, &month_now())
}

/// Read content-free cost status without resetting or advancing accounting.
///
/// # Errors
/// Missing/corrupt state and backward calendar changes remain explicit.
pub fn search_account_status(path: &Path, account_id: &str) -> Result<SearchAccountStatus> {
    let _lease = lease(path)?;
    status_at(path, account_id, &month_now())
}

fn status_at(path: &Path, id: &str, month: &str) -> Result<SearchAccountStatus> {
    let account = load(path, id)?;
    if month < account.last_month.as_str() {
        return Err(E::SearchSpendingState);
    }
    let count = *account.months.get(month).unwrap_or(&0);
    Ok(SearchAccountStatus {
        schema: SCHEMA,
        month_utc: month.into(),
        reserved_requests: count,
        reserved_microusd: count
            .checked_mul(BRAVE_REQUEST_MICROUSD)
            .ok_or(E::SearchSpendingState)?,
        monthly_limit_microusd: account.monthly_limit_microusd,
        scope: "shared local integration; gross reservations; excludes external API use and credits",
    })
}

pub(crate) fn reserve(path: &Path, policy: &BraveSearchPolicyV1) -> Result<()> {
    reserve_at(path, policy, &month_now())
}

pub(crate) fn reserve_at(path: &Path, policy: &BraveSearchPolicyV1, month: &str) -> Result<()> {
    let _lease = lease(path)?;
    let account = load(path, &policy.account_id)?;
    if !valid_month(month)
        || month < account.last_month.as_str()
        || account.monthly_limit_microusd != policy.monthly_limit_microusd
        || account.request_cost_microusd != policy.request_cost_microusd
    {
        return Err(E::SearchSpendingState);
    }
    debit(path, account, month)
}

pub(crate) fn reserve_operator_check(path: &Path, account_id: &str) -> Result<()> {
    let _lease = lease(path)?;
    debit(path, load(path, account_id)?, &month_now())
}

fn debit(path: &Path, mut account: SpendingState, month: &str) -> Result<()> {
    if !valid_month(month) || month < account.last_month.as_str() {
        return Err(E::SearchSpendingState);
    }
    let count = account
        .months
        .get(month)
        .copied()
        .unwrap_or(0)
        .checked_add(1)
        .ok_or(E::SearchSpendingLimit)?;
    if count
        .checked_mul(BRAVE_REQUEST_MICROUSD)
        .is_none_or(|cost| cost > account.monthly_limit_microusd)
    {
        return Err(E::SearchSpendingLimit);
    }
    account.last_month = month.into();
    account.months.insert(month.into(), count);
    persist(path, &account, false)
}
