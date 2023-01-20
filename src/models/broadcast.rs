use crate::errors::{ApiError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BroadcastFilter {
    pub id: Uuid,
    pub blockchain_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    pub addresses: Option<Json<Vec<String>>>,
    pub callback_url: String,
    pub auth_token: String,
    pub txn_types: Json<Vec<String>>,
    pub is_active: bool,
    pub last_processed_height: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl BroadcastFilter {
    pub async fn find_by_id(id: &Uuid, db: &mut sqlx::PgConnection) -> Result<Self> {
        sqlx::query_as("SELECT * FROM broadcast_filters where id = $1")
            .bind(id)
            .fetch_one(db)
            .await
            .map_err(ApiError::from)
    }

    pub async fn find_all_by_org_id(
        org_id: &Uuid,
        db: &mut sqlx::PgConnection,
    ) -> Result<Vec<Self>> {
        sqlx::query_as("SELECT * FROM broadcast_filters where org_id = $1")
            .bind(org_id)
            .fetch_all(db)
            .await
            .map_err(ApiError::from)
    }

    pub async fn create(req: &BroadcastFilterRequest, tx: &mut super::DbTrx<'_>) -> Result<Self> {
        req.validate()
            .map_err(|e| ApiError::ValidationError(e.to_string()))?;
        sqlx::query_as(
            r##"
            INSERT INTO broadcast_filters
                (blockchain_id, org_id, name, addresses, callback_url, auth_token, txn_types, is_active)
            VALUES
                ($1,$2,$3,$4,$5,$6,$7,$8)
            RETURNING *
            "##)
        .bind(req.blockchain_id)
        .bind(req.org_id)
        .bind(&req.name)
        .bind(req.addresses.as_ref().map(Json))
        .bind(&req.callback_url)
        .bind(&req.auth_token)
        .bind(Json(&req.txn_types))
        .bind(req.is_active)
        .fetch_one(tx)
        .await
        .map_err(ApiError::from)
    }

    pub async fn update(
        id: &Uuid,
        req: &BroadcastFilterRequest,
        tx: &mut super::DbTrx<'_>,
    ) -> Result<Self> {
        req.validate()
            .map_err(|e| ApiError::ValidationError(e.to_string()))?;
        sqlx::query_as(
            r##"
            UPDATE broadcast_filters
                SET blockchain_id=$1, org_id=$2, name=$3, addresses=$4, callback_url=$5, auth_token=$6, txn_types=$7, is_active=$8
            WHERE
                id=$9
            RETURNING *
            "##)
        .bind(req.blockchain_id)
        .bind(req.org_id)
        .bind(&req.name)
        .bind(req.addresses.as_ref().map(Json))
        .bind(&req.callback_url)
        .bind(&req.auth_token)
        .bind(Json(&req.txn_types))
        .bind(req.is_active)
        .bind(id)
        .fetch_one(tx)
        .await
        .map_err(ApiError::from)
    }

    pub async fn delete(id: &Uuid, tx: &mut super::DbTrx<'_>) -> Result<()> {
        sqlx::query("DELETE FROM broadcast_filters WHERE id = $1")
            .bind(id)
            .execute(tx)
            .await?;

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct BroadcastFilterRequest {
    pub blockchain_id: Uuid,
    pub org_id: Uuid,
    pub name: String,
    #[validate(required)]
    pub addresses: Option<Vec<String>>,
    pub callback_url: String,
    pub auth_token: String,
    pub txn_types: Vec<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct BroadcastLog {
    pub id: Uuid,
    pub blockchain_id: Uuid,
    pub org_id: Uuid,
    pub broadcast_filter_id: Uuid,
    pub address_count: i64,
    pub txn_count: i64,
    pub event_type: String,
    pub event_msg: Option<String>,
    pub created_at: DateTime<Utc>,
}
