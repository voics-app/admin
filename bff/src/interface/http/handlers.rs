//! `/admin-api` handlers: parse input, call one use case, map the result.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::{Extension, extract::rejection::JsonRejection};
use serde::Deserialize;

use super::dto::*;
use super::error::ApiError;
use super::state::AppState;
use crate::application::ports::api_admin::ReportSummary;
use crate::application::ports::platform_metrics::HealthPanel;
use crate::application::use_cases::authenticate_admin::AdminIdentity;
use crate::application::use_cases::upsert_feature_flag::FlagChange;
use crate::application::use_cases::usage_range::UsageRange;

type ApiResult<T> = Result<Json<T>, ApiError>;

fn parse_range(value: Option<&str>, default: UsageRange) -> Result<UsageRange, ApiError> {
    match value {
        None => Ok(default),
        Some(v) => UsageRange::parse(v)
            .ok_or_else(|| ApiError::bad_request("range must be one of 1h, 24h, 7d, 30d, 90d")),
    }
}

fn body<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, ApiError> {
    payload
        .map(|Json(v)| v)
        .map_err(|e| ApiError::bad_request(e.body_text()))
}

pub async fn me(Extension(identity): Extension<AdminIdentity>) -> Json<MeDto> {
    Json(MeDto {
        email: identity.email,
    })
}

pub async fn overview(State(state): State<AppState>) -> Json<OverviewDto> {
    Json(state.overview.execute().await.into())
}

#[derive(Deserialize)]
pub struct RangeQuery {
    range: Option<String>,
}

pub async fn usage(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> ApiResult<UsageDto> {
    let range = parse_range(q.range.as_deref(), UsageRange::Day)?;
    Ok(Json(state.usage.execute(range).await?.into()))
}

pub async fn voice_regions(State(state): State<AppState>) -> ApiResult<VoiceRegionsDto> {
    Ok(Json(state.voice_regions.execute().await?.into()))
}

pub async fn drain_node(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Path(node_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .voice_draining
        .execute(&identity.email, &node_id, true)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn undrain_node(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Path(node_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .voice_draining
        .execute(&identity.email, &node_id, false)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn platform_health(State(state): State<AppState>) -> Json<PlatformHealthDto> {
    Json(state.platform_health.execute().await.into())
}

#[derive(Deserialize)]
pub struct SeriesQuery {
    panel: String,
    range: Option<String>,
}

pub async fn platform_health_series(
    State(state): State<AppState>,
    Query(q): Query<SeriesQuery>,
) -> ApiResult<HealthSeriesDto> {
    let panel = HealthPanel::parse(&q.panel)
        .ok_or_else(|| ApiError::bad_request("unknown health panel"))?;
    let range = parse_range(q.range.as_deref(), UsageRange::Hour)?;
    let series = state.health_series.execute(panel, range).await?;
    Ok(Json(HealthSeriesDto {
        panel: panel.as_str(),
        range: range.as_str(),
        series,
    }))
}

#[derive(Deserialize)]
pub struct UserSearch {
    #[serde(default)]
    query: String,
    limit: Option<u32>,
    after: Option<String>,
}

pub async fn search_users(
    State(state): State<AppState>,
    Query(q): Query<UserSearch>,
) -> ApiResult<UserPageDto> {
    let page = state
        .search_users
        .execute(&q.query, q.limit, q.after)
        .await?;
    Ok(Json(UserPageDto {
        users: page.users,
        next_after: page.next_after,
    }))
}

pub async fn suspend_user(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Path(user_id): Path<String>,
    payload: Result<Json<SuspendRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let request = body(payload)?;
    state
        .suspend_user
        .execute(
            &identity.email,
            &user_id,
            &request.reason,
            request.until.as_deref(),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct ReportsQuery {
    status: Option<String>,
    limit: Option<u32>,
    after: Option<String>,
}

pub async fn list_reports(
    State(state): State<AppState>,
    Query(q): Query<ReportsQuery>,
) -> ApiResult<ReportPageDto> {
    let page = state
        .list_reports
        .execute(q.status.as_deref(), q.limit, q.after)
        .await?;
    Ok(Json(ReportPageDto {
        reports: page.reports,
        next_after: page.next_after,
    }))
}

pub async fn resolve_report(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Path(report_id): Path<String>,
    payload: Result<Json<ResolveReportRequest>, JsonRejection>,
) -> ApiResult<ReportSummary> {
    let request = body(payload)?;
    let report = state
        .resolve_report
        .execute(
            &identity.email,
            &report_id,
            &request.resolution,
            request.note.as_deref(),
        )
        .await?;
    Ok(Json(report))
}

#[derive(Deserialize)]
pub struct GuildSearch {
    #[serde(default)]
    query: String,
}

pub async fn search_guilds(
    State(state): State<AppState>,
    Query(q): Query<GuildSearch>,
) -> ApiResult<GuildListDto> {
    Ok(Json(GuildListDto {
        guilds: state.search_guilds.execute(&q.query).await?,
    }))
}

pub async fn list_flags(State(state): State<AppState>) -> ApiResult<FlagListDto> {
    let flags = state.list_flags.execute().await?;
    Ok(Json(FlagListDto {
        flags: flags.into_iter().map(Into::into).collect(),
    }))
}

pub async fn put_flag(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Path(key): Path<String>,
    payload: Result<Json<FlagRequest>, JsonRejection>,
) -> ApiResult<FlagDto> {
    let request = body(payload)?;
    let change = FlagChange {
        key,
        description: request.description,
        enabled: request.enabled,
        rollout_percent: request.rollout_percent,
    };
    Ok(Json(
        state
            .upsert_flag
            .execute(&identity.email, &change)
            .await?
            .into(),
    ))
}

pub async fn delete_flag(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Path(key): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.delete_flag.execute(&identity.email, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct AuditQuery {
    limit: Option<u32>,
    before: Option<i64>,
}

pub async fn audit_log(
    State(state): State<AppState>,
    Query(q): Query<AuditQuery>,
) -> ApiResult<AuditPageDto> {
    let entries = state.audit_log.execute(q.limit, q.before).await?;
    let limit = q
        .limit
        .unwrap_or(crate::application::use_cases::list_audit_log::DEFAULT_AUDIT_PAGE)
        .max(1) as usize;
    let next_before = (entries.len() >= limit)
        .then(|| entries.last().map(|e| e.id))
        .flatten();
    Ok(Json(AuditPageDto {
        entries: entries.into_iter().map(Into::into).collect(),
        next_before,
    }))
}

pub async fn unknown_api_route() -> ApiError {
    ApiError::not_found()
}
