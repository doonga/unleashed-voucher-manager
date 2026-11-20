//! The operations the HTTP handlers need, implemented on Unleashed guest
//! passes.

use std::{collections::HashSet, sync::OnceLock};

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use chrono_tz::Tz;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

use crate::{
    environment::Environment,
    models::{
        DeleteResponse, Voucher, VouchersCreateRequest, VouchersCreateResponse, VouchersGetRequest,
        VouchersGetResponse,
    },
    unleashed::{
        create::{CreateOutcome, CreateParams, build_form, parse_create_response},
        guest::{GuestPass, parse_guest_list},
        mapping::{current_rolling, is_expired, is_rolling, rolling_name, to_voucher},
        session::{Session, SessionError},
    },
};

pub static UNLEASHED_API: OnceLock<UnleashedAPI> = OnceLock::new();

const CREATE_PAGE: &str = "mon_createguest.jsp";
/// Rolling vouchers are for one device each.
const ROLLING_SHARE_LIMIT: u64 = 1;

/// A failed operation: the HTTP status for the frontend and a message saying
/// why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.status)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, self.message).into_response()
    }
}

impl From<SessionError> for ApiError {
    fn from(e: SessionError) -> Self {
        error!("{e}");
        Self::new(StatusCode::BAD_GATEWAY, "The controller request failed")
    }
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub url: String,
    pub username: String,
    pub password: String,
    pub ssid: String,
    pub verify_tls: bool,
    pub timezone: Tz,
    pub rolling_voucher_duration_minutes: u64,
}

impl ApiConfig {
    pub fn from_environment(env: &Environment) -> Self {
        Self {
            url: env.unleashed_url.clone(),
            username: env.unleashed_username.clone(),
            password: env.unleashed_password.clone(),
            ssid: env.unleashed_ssid.clone(),
            verify_tls: env.unleashed_has_valid_cert,
            timezone: env.timezone,
            rolling_voucher_duration_minutes: env.rolling_voucher_duration_minutes,
        }
    }
}

pub struct UnleashedAPI {
    session: Session,
    config: ApiConfig,
    /// Held from the list before a create to the list after it, so
    /// concurrent creates cannot claim each other's passes, and across the
    /// rolling voucher's check and create.
    create_lock: Mutex<()>,
}

fn now() -> i64 {
    Utc::now().timestamp()
}

fn listing_xml() -> String {
    format!(
        "<ajax-request action='getconf' DECRYPT_X='true' updater='guest-list.{}' \
         comp='guest-list'><guest self-service='!true'/></ajax-request>",
        Utc::now().timestamp_millis()
    )
}

impl UnleashedAPI {
    pub async fn try_new(config: ApiConfig) -> Result<Self, SessionError> {
        let session = Session::new(
            &config.url,
            &config.username,
            &config.password,
            config.verify_tls,
        )?;
        session.login().await?;
        Ok(Self {
            session,
            config,
            create_lock: Mutex::new(()),
        })
    }

    async fn passes(&self) -> Result<Vec<GuestPass>, ApiError> {
        let xml = self.session.conf(&listing_xml()).await?;
        parse_guest_list(&xml).map_err(|e| {
            error!("{e}");
            ApiError::new(StatusCode::BAD_GATEWAY, "Unreadable guest list")
        })
    }

    /// Vouchers, newest first.
    async fn vouchers(&self) -> Result<Vec<Voucher>, ApiError> {
        let mut passes = self.passes().await?;
        passes.sort_by(|a, b| b.create_time.cmp(&a.create_time).then(b.id.cmp(&a.id)));
        let now = now();
        Ok(passes
            .iter()
            .map(|p| to_voucher(p, self.config.timezone, now))
            .collect())
    }

    pub async fn get_all_vouchers(&self) -> Result<VouchersGetResponse, ApiError> {
        let data = self.vouchers().await?;
        Ok(VouchersGetResponse {
            offset: 0,
            limit: data.len() as u32,
            count: data.len() as u32,
            total_count: data.len() as u64,
            data,
        })
    }

    pub async fn get_vouchers(
        &self,
        request: &VouchersGetRequest,
    ) -> Result<VouchersGetResponse, ApiError> {
        let filter = request
            .filter
            .as_deref()
            .map(str::to_lowercase)
            .filter(|f| !f.is_empty());
        let matching: Vec<Voucher> = self
            .vouchers()
            .await?
            .into_iter()
            .filter(|v| {
                filter
                    .as_ref()
                    .is_none_or(|f| v.name.to_lowercase().contains(f))
            })
            .collect();
        let total_count = matching.len() as u64;
        let data: Vec<Voucher> = matching
            .into_iter()
            .skip(request.offset as usize)
            .take(request.limit as usize)
            .collect();
        Ok(VouchersGetResponse {
            offset: u64::from(request.offset),
            limit: request.limit,
            count: data.len() as u32,
            total_count,
            data,
        })
    }

    pub async fn get_voucher_details(&self, id: String) -> Result<Voucher, ApiError> {
        self.vouchers()
            .await?
            .into_iter()
            .find(|v| v.id == id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "No such voucher"))
    }

    pub async fn get_newest_voucher(&self) -> Result<Voucher, ApiError> {
        self.vouchers().await?.into_iter().next().ok_or_else(|| {
            warn!("No vouchers found when fetching the newest voucher");
            ApiError::new(StatusCode::NOT_FOUND, "No vouchers")
        })
    }

    pub async fn get_rolling_voucher(&self) -> Result<Option<Voucher>, ApiError> {
        let passes = self.passes().await?;
        let now = now();
        Ok(current_rolling(&passes, now).map(|p| to_voucher(p, self.config.timezone, now)))
    }

    /// Create passes and return the ones that appeared. The caller holds the
    /// create lock.
    async fn create_locked(&self, params: &CreateParams) -> Result<Vec<Voucher>, ApiError> {
        let form = build_form(params).map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e))?;
        let before: HashSet<String> = self.passes().await?.into_iter().map(|p| p.id).collect();

        let body = self.session.form(CREATE_PAGE, &form).await?;
        match parse_create_response(&body) {
            CreateOutcome::Accepted => {}
            CreateOutcome::KeyDuplicated(message) => {
                return Err(ApiError::new(StatusCode::CONFLICT, message));
            }
            CreateOutcome::InvalidCharacters(message) => {
                return Err(ApiError::new(StatusCode::BAD_REQUEST, message));
            }
            CreateOutcome::Failed(message) => {
                error!("Creating vouchers failed: {message}");
                return Err(ApiError::new(StatusCode::BAD_GATEWAY, message));
            }
        }

        let now = now();
        let mut created: Vec<GuestPass> = self
            .passes()
            .await?
            .into_iter()
            .filter(|p| !before.contains(&p.id))
            .collect();
        created.sort_by(|a, b| b.create_time.cmp(&a.create_time).then(b.id.cmp(&a.id)));
        if created.is_empty() {
            error!("The controller accepted a create but no new pass appeared");
            return Err(ApiError::new(
                StatusCode::BAD_GATEWAY,
                "The controller did not create the voucher",
            ));
        }
        Ok(created
            .iter()
            .map(|p| to_voucher(p, self.config.timezone, now))
            .collect())
    }

    pub async fn create_voucher(
        &self,
        request: &VouchersCreateRequest,
    ) -> Result<VouchersCreateResponse, ApiError> {
        let params = CreateParams {
            count: request.count,
            name: request.name.clone(),
            minutes: request.time_limit_minutes,
            share_limit: request.authorized_guest_limit,
            key: request.code.clone(),
            remarks: request.remarks.clone(),
            ssid: self.config.ssid.clone(),
        };
        let _guard = self.create_lock.lock().await;
        let vouchers = self.create_locked(&params).await?;
        info!("Created {} voucher(s)", vouchers.len());
        Ok(VouchersCreateResponse { vouchers })
    }

    /// The waiting rolling voucher, creating one only when none is waiting.
    pub async fn create_rolling_voucher(&self) -> Result<Voucher, ApiError> {
        let _guard = self.create_lock.lock().await;
        if let Some(waiting) = self.get_rolling_voucher().await? {
            return Ok(waiting);
        }
        let params = CreateParams {
            count: 1,
            name: rolling_name(Utc::now().with_timezone(&self.config.timezone)),
            minutes: self.config.rolling_voucher_duration_minutes,
            share_limit: Some(ROLLING_SHARE_LIMIT),
            key: None,
            remarks: None,
            ssid: self.config.ssid.clone(),
        };
        let voucher = self.create_locked(&params).await?.remove(0);
        info!("Created rolling voucher {}", voucher.name);
        Ok(voucher)
    }

    async fn delete_passes(&self, ids: &[String]) -> Result<(), ApiError> {
        if ids.is_empty() {
            return Ok(());
        }
        let guests: String = ids
            .iter()
            .map(|id| format!("<guest id='{id}'></guest>"))
            .collect();
        let xml = format!(
            "<ajax-request action='delobj' updater='guest-list.{}' comp='guest-list'>{guests}</ajax-request>",
            Utc::now().timestamp_millis()
        );
        self.session.conf(&xml).await?;
        Ok(())
    }

    pub async fn delete_vouchers_by_ids(
        &self,
        ids: Vec<String>,
    ) -> Result<DeleteResponse, ApiError> {
        if ids.is_empty()
            || ids
                .iter()
                .any(|id| id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Voucher ids must be numbers",
            ));
        }
        let existing: HashSet<String> = self.passes().await?.into_iter().map(|p| p.id).collect();
        let ids: Vec<String> = ids.into_iter().filter(|id| existing.contains(id)).collect();
        self.delete_passes(&ids).await?;
        Ok(DeleteResponse {
            vouchers_deleted: ids.len() as u32,
        })
    }

    async fn delete_expired_where(
        &self,
        keep: impl Fn(&GuestPass) -> bool,
    ) -> Result<DeleteResponse, ApiError> {
        let now = now();
        let ids: Vec<String> = self
            .passes()
            .await?
            .into_iter()
            .filter(|p| is_expired(p, now) && !keep(p))
            .map(|p| p.id)
            .collect();
        self.delete_passes(&ids).await?;
        Ok(DeleteResponse {
            vouchers_deleted: ids.len() as u32,
        })
    }

    pub async fn delete_expired_vouchers(&self) -> Result<DeleteResponse, ApiError> {
        self.delete_expired_where(|_| false).await
    }

    pub async fn delete_expired_rolling_vouchers(&self) -> Result<DeleteResponse, ApiError> {
        self.delete_expired_where(|p| !is_rolling(p)).await
    }
}
