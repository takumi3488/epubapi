use core::fmt;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use utoipa::ToSchema;

/// DBのEnumとして定義したInvitationStateを使う
#[derive(Debug, PartialEq, sqlx::Type)]
#[sqlx(type_name = "invitation_state", rename_all = "lowercase")]
pub enum InvitationState {
    Unused,
    Using,
    Used,
}

impl fmt::Display for InvitationState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Unused => write!(f, "unused"),
            Self::Using => write!(f, "using"),
            Self::Used => write!(f, "used"),
        }
    }
}

/// `POST /check_invitation` のリクエストボディ
#[derive(Serialize, Deserialize, ToSchema)]
pub struct CheckInvitationRequest {
    pub invitation_code: String,
}

/// `POST /check_invitation` のレスポンス
#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CheckInvitationResponse {
    pub state: String,
}

/// 招待コードのDBクエリの結果
#[derive(PartialEq)]
struct InvitationStateQueryResult {
    pub state: InvitationState,
    pub used_at: Option<chrono::NaiveDateTime>,
}

/// 招待コードの状態を確認する
pub async fn check_invitation_state(
    db: &PgPool,
    invitation_code: &str,
) -> Result<CheckInvitationResponse, sqlx::Error> {
    // 招待コードの状態を取得
    let invitation_state_query_result = sqlx::query_as!(
        InvitationStateQueryResult,
        r#"SELECT state as "state: InvitationState", used_at FROM invitations WHERE code = $1"#,
        invitation_code
    )
    .fetch_one(db)
    .await?;

    match invitation_state_query_result.state {
        InvitationState::Unused => {
            // 招待コードの状態が未使用の場合
            // 招待コードの状態を using に更新
            sqlx::query!(
                r#"UPDATE invitations SET state = 'using', used_at = $1 WHERE code = $2"#,
                Utc::now().naive_utc(),
                invitation_code
            )
            .execute(db)
            .await?;
            Ok(CheckInvitationResponse {
                state: InvitationState::Unused.to_string(),
            })
        }
        InvitationState::Using => {
            // 招待コードの状態が使用中の場合
            if invitation_state_query_result.used_at.is_none()
                || invitation_state_query_result.used_at.unwrap() + Duration::minutes(5)
                    < Utc::now().naive_utc()
            {
                // 使用中の招待コードが5分以上経過している場合
                // 招待コードの状態を unused に更新
                sqlx::query!(
                    r#"UPDATE invitations SET state = 'unused', used_at = null WHERE code = $1"#,
                    invitation_code
                )
                .execute(db)
                .await?;
                Ok(CheckInvitationResponse {
                    state: InvitationState::Unused.to_string(),
                })
            } else {
                // 使用中の招待コードが5分以内の場合
                Ok(CheckInvitationResponse {
                    state: InvitationState::Using.to_string(),
                })
            }
        }
        InvitationState::Used => {
            // 招待コードの状態が使用済みの場合
            Ok(CheckInvitationResponse {
                state: InvitationState::Used.to_string(),
            })
        }
    }
}
