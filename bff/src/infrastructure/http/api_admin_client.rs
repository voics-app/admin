use async_trait::async_trait;

use super::internal_client::InternalClient;
use crate::application::ports::PortResult;
use crate::application::ports::api_admin::{
    ApiAdmin, ApiStats, GuildSummary, Suspension, UserPage, UserQuery, UserSummary,
};
use crate::infrastructure::contracts::{
    ApiStatsDto, GuildDto, GuildListDto, SuspendRequestDto, UserDto, UserListDto,
};

/// Adapter for the api service's admin endpoints (P4).
pub struct ApiAdminClient {
    client: InternalClient,
}

impl ApiAdminClient {
    pub fn new(client: InternalClient) -> Self {
        Self { client }
    }
}

fn to_user(dto: UserDto) -> UserSummary {
    UserSummary {
        id: dto.id,
        username: dto.username,
        global_name: dto.global_name,
        email: dto.email,
        bot: dto.bot,
        created_at: dto.created_at,
        suspended_until: dto.suspended_until,
    }
}

fn to_guild(dto: GuildDto) -> GuildSummary {
    GuildSummary {
        id: dto.id,
        name: dto.name,
        owner_id: dto.owner_id,
        member_count: dto.member_count,
        voice_region: dto.voice_region,
    }
}

#[async_trait]
impl ApiAdmin for ApiAdminClient {
    async fn stats(&self) -> PortResult<ApiStats> {
        let dto: ApiStatsDto = self
            .client
            .get_json("/internal/v1/admin/stats", &[])
            .await?;
        Ok(ApiStats {
            users_total: dto.users_total,
            guilds_total: dto.guilds_total,
            channels_total: dto.channels_total,
            bots_total: dto.bots_total,
            signups_24h: dto.signups_24h,
        })
    }

    async fn search_users(&self, query: &UserQuery) -> PortResult<UserPage> {
        let mut params = vec![
            ("query", query.query.clone()),
            ("limit", query.limit.to_string()),
        ];
        if let Some(after) = &query.after {
            params.push(("after", after.clone()));
        }
        let dto: UserListDto = self
            .client
            .get_json("/internal/v1/admin/users", &params)
            .await?;
        let (users, next_after) = match dto {
            UserListDto::Page { users, next_after } => (users, next_after),
            UserListDto::Bare(users) => {
                // Without an explicit cursor, a full page implies more results after the last id.
                let next = (users.len() as u32 >= query.limit)
                    .then(|| users.last().map(|u| u.id.clone()))
                    .flatten();
                (users, next)
            }
        };
        Ok(UserPage {
            users: users.into_iter().map(to_user).collect(),
            next_after,
        })
    }

    async fn search_guilds(&self, query: &str) -> PortResult<Vec<GuildSummary>> {
        let dto: GuildListDto = self
            .client
            .get_json("/internal/v1/admin/guilds", &[("query", query.to_owned())])
            .await?;
        let guilds = match dto {
            GuildListDto::Page { guilds } | GuildListDto::Bare(guilds) => guilds,
        };
        Ok(guilds.into_iter().map(to_guild).collect())
    }

    async fn suspend_user(&self, user_id: &str, suspension: &Suspension) -> PortResult<()> {
        let body = SuspendRequestDto {
            reason: &suspension.reason,
            until: suspension.until.as_deref(),
        };
        self.client
            .post(
                &format!("/internal/v1/admin/users/{user_id}/suspend"),
                Some(&body),
            )
            .await
    }
}
