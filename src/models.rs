use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, postgres::{PgRow, Postgres}};
use sqlx::{FromRow, PgPool, Result, Row, Transaction};
use std::convert::From;
use uuid::Uuid;
use std::net::{IpAddr, Ipv4Addr};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "enum_conn_status", rename_all = "snake_case")]
pub enum ConnectionStatus {
    Online,
    Offline,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "enum_validator_status", rename_all = "snake_case")]
pub enum ValidatorStatus {
    Provisioning,
    Syncing,
    Upgrading,
    Synced,
    Consensus,
    Stopped,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "enum_stake_status", rename_all = "snake_case")]
pub enum StakeStatus {
    Available,
    Staking,
    Staked,
    Delinquent,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub hashword: String,
    pub salt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Host {
    pub id: Uuid,
    pub name: String,
    pub version: Option<String>,
    pub location: Option<String>,
    pub ip_addr: IpNetwork,
    pub val_ip_addr_start: IpNetwork,
    pub val_count: i32,
    pub token: String,
    pub status: ConnectionStatus,
    pub validators: Option<Vec<Validator>>,
    pub created_at: time::PrimitiveDateTime,
}

impl From<PgRow> for Host {
    fn from(row: PgRow) -> Self {
        Host {
            id: row.try_get("id").expect("Couldn't try_get id for host."),
            name: row
                .try_get("name")
                .expect("Couldn't try_get name for host."),
            version: row
                .try_get("version")
                .expect("Couldn't try_get version for host."),
            location: row
                .try_get("location")
                .expect("Couldn't try_get location for host."),
            ip_addr: row
                .try_get("ip_addr")
                .expect("Couldn't try_get ip_addr for host."),
            val_ip_addr_start: row
                .try_get("val_ip_addr_start")
                .expect("Couldn't try_get val_ip_addr_start for host."),
            val_count: row
                .try_get("val_count")
                .expect("Couldn't try_get val_count for host."),
            token: row
                .try_get("token")
                .expect("Couldn't try_get token for host."),
            status: row
                .try_get("status")
                .expect("Couldn't try_get status for host."),
            validators: None,
            created_at: row
                .try_get("created_at")
                .expect("Couldn't try_get created_at for host."),
        }
    }
}

impl Host {
    pub async fn find_all(pool: &PgPool) -> Result<Vec<Self>> {
        sqlx::query("SELECT * FROM hosts")
            .map(|row: PgRow| Self::from(row))
            .fetch_all(pool)
            .await
    }

    pub async fn find_by_id(id: Uuid, pool: &PgPool) -> Result<Self> {
        sqlx::query("SELECT * FROM hosts WHERE id = $1")
            .bind(id)
            .map(|row: PgRow| Self::from(row))
            .fetch_one(pool)
            .await
    }

    pub async fn find_by_token(token: &str, pool: &PgPool) -> Result<Self> {
        sqlx::query("SELECT * FROM hosts WHERE token = $1")
            .bind(token)
            .map(|row: PgRow| Self::from(row))
            .fetch_one(pool)
            .await
    }

    pub async fn create(host: HostRequest, pool: &PgPool) -> Result<Self> {
        let mut tx = pool.begin().await?;
        let mut host = sqlx::query("INSERT INTO hosts (name, version, location, ip_addr, val_ip_addr_start, val_count, token, status) VALUES ($1,$2,$3,$4,$5,$6,$7,$8) RETURNING *")
        .bind(host.name)
        .bind(host.version)
        .bind(host.location)
        .bind(host.ip_addr)
        .bind(host.val_ip_addr_start)
        .bind(host.val_count)
        .bind(host.token)
        .bind(host.status)
        .map(|row: PgRow| {
            Self::from(row)
        })
        .fetch_one(&mut tx)
        .await?;

        let mut vals:Vec<Validator> = vec![];

        // Create and add validators
        for i in 0..host.val_count {

            //TODO: Fix name/ip_addr
            let val = ValidatorRequest {
                name: petname::petname(2, "_"),
                version: None,
                ip_addr: host.val_ip_addr_start,
                host_id: host.id,
                user_id: None,
                address: None,
                swarm_key: None,
                stake_status: StakeStatus::Available,
                status: ValidatorStatus::Provisioning,
                score: 0,
            };

            //TODO add to array
            let val = Validator::create_tx(val, &mut tx).await?;
            vals.push(val.to_owned());
   
        }

        host.validators = Some(vals);

        tx.commit().await?;

        Ok(host)
    }

    pub async fn update(id: Uuid, host: HostRequest, pool: &PgPool) -> Result<Self> {
        let mut tx = pool.begin().await.unwrap();
        let host = sqlx::query(
            r#"UPDATE hosts SET name = $1, version = $2, location = $3, ip_addr = $4, val_ip_addr_start = $5, val_count = $6, token = $7, status = $8  WHERE id = $9 RETURNING *"#
        )
        .bind(host.name)
        .bind(host.version)
        .bind(host.location)
        .bind(host.ip_addr)
        .bind(host.val_ip_addr_start)
        .bind(host.val_count)
        .bind(host.token)
        .bind(host.status)
        .bind(id)
        .map(|row: PgRow| {
            Self::from(row)
        })
        .fetch_one(&mut tx)
        .await?;

        tx.commit().await.unwrap();
        Ok(host)
    }

    pub async fn delete(id: Uuid, pool: &PgPool) -> Result<u64> {
        let mut tx = pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM hosts WHERE id = $1")
            .bind(id)
            .execute(&mut tx)
            .await?;

        tx.commit().await?;
        Ok(deleted.rows_affected())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostRequest {
    pub name: String,
    pub version: Option<String>,
    pub location: Option<String>,
    pub ip_addr: IpNetwork,
    pub val_ip_addr_start: IpNetwork,
    pub val_count: i32,
    pub token: String,
    pub status: ConnectionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Validator {
    pub id: Uuid,
    pub name: String,
    pub version: Option<String>,
    pub ip_addr: IpNetwork,
    pub host_id: Uuid,
    pub user_id: Option<Uuid>,
    pub address: Option<String>,
    pub swarm_key: Option<Vec<u8>>,
    pub stake_status: StakeStatus,
    pub status: ValidatorStatus,
    pub score: i64,
    pub created_at: time::PrimitiveDateTime,
}

impl From<PgRow> for Validator {
    fn from(row: PgRow) -> Self {
        Self {
            id: row
                .try_get("id")
                .expect("Couldn't try_get id for validator."),
            name: row
                .try_get("name")
                .expect("Couldn't try_get name for validator."),
            version: row
                .try_get("version")
                .expect("Couldn't try_get version for validator."),
            ip_addr: row
                .try_get("ip_addr")
                .expect("Couldn't try_get ip_addr for validator."),
            host_id: row
                .try_get("host_id")
                .expect("Couldn't try_get host_id for validator."),
            user_id: row
                .try_get("user)id")
                .expect("Couldn't try_get user_id for validator."),
            address: row
                .try_get("address")
                .expect("Couldn't try_get address for validator."),
            swarm_key: row
                .try_get("swarm_key")
                .expect("Couldn't try_get swarm_key for validator."),
            stake_status: row
                .try_get("stake_status")
                .expect("Couldn't try_get stake_status for validator."),
            status: row
                .try_get("status")
                .expect("Couldn't try_get status for validator."),
            score: row
                .try_get("score")
                .expect("Couldn't try_get score for validator."),
            created_at: row
                .try_get("created_at")
                .expect("Couldn't try_get created_at for validator."),
        }
    }
}

impl Validator {
    pub async fn find_all(pool: &PgPool) -> Result<Vec<Self>> {
        sqlx::query_as::<_, Self>("SELECT * FROM validators")
            .fetch_all(pool)
            .await
    }

    pub async fn find_all_by_host(host_id: Uuid, pool: &PgPool) -> Result<Vec<Self>> {
        sqlx::query_as::<_, Self>("SELECT * FROM validators WHERE host_id = $1 order by created_at")
            .bind(host_id)
            .fetch_all(pool)
            .await
    }

    pub async fn find_by_id(id: Uuid, pool: &PgPool) -> Result<Self> {
        sqlx::query_as::<_, Self>("SELECT * FROM validators WHERE id = $1")
            .bind(id)
            .fetch_one(pool)
            .await
    }

    pub async fn create_tx(validator: ValidatorRequest, tx: &mut PgConnection) -> Result<Self> {
        let validator = sqlx::query_as::<_, Self>("INSERT INTO validators (name, version, ip_addr, host_id, user_id, address, swarm_key, stake_status, status, score) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) RETURNING *")
        .bind(validator.name)
        .bind(validator.version)
        .bind(validator.ip_addr)
        .bind(validator.host_id)
        .bind(validator.user_id)
        .bind(validator.address)
        .bind(validator.swarm_key)
        .bind(validator.stake_status)
        .bind(validator.status)
        .bind(validator.score)
        .fetch_one(tx)
        .await?;

        Ok(validator)
    }

    pub async fn create(validator: ValidatorRequest, pool: &PgPool) -> Result<Self> {
        let mut tx = pool.begin().await?;
        
        let validator = Self::create_tx(validator, &mut tx).await?;

        tx.commit().await?;

        Ok(validator)
    }

    pub async fn update(id: Uuid, validator: ValidatorRequest, pool: &PgPool) -> Result<Self> {
        let mut tx = pool.begin().await.unwrap();
        let validator = sqlx::query_as::<_, Self>(
            r#"UPDATE validators SET name=$1, version=$2, ip_addr=$3, host_id=$4, user_id=$5, address=$6, swarm_key=$7, stake_status=$8, status=$9, score=$10  WHERE id = $11 RETURNING *"#
        )
        .bind(validator.name)
        .bind(validator.version)
        .bind(validator.ip_addr)
        .bind(validator.host_id)
        .bind(validator.user_id)
        .bind(validator.address)
        .bind(validator.swarm_key)
        .bind(validator.stake_status)
        .bind(validator.status)
        .bind(validator.score)
        .bind(id)
        .fetch_one(&mut tx)
        .await?;

        tx.commit().await.unwrap();
        Ok(validator)
    }

    pub async fn update_status(
        id: Uuid,
        validator: ValidatorStatusRequest,
        pool: &PgPool,
    ) -> Result<Self> {
        let mut tx = pool.begin().await.unwrap();
        let validator = sqlx::query_as::<_, Self>(
            r#"UPDATE validators SET version=$1, stake_status=$2, status=$3, score=$4  WHERE id = $5 RETURNING *"#
        )
        .bind(validator.version)
        .bind(validator.stake_status)
        .bind(validator.status)
        .bind(validator.score)
        .bind(id)
        .fetch_one(&mut tx)
        .await?;

        tx.commit().await.unwrap();
        Ok(validator)
    }

    pub async fn delete(id: Uuid, pool: &PgPool) -> Result<u64> {
        let mut tx = pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM validators WHERE id = $1")
            .bind(id)
            .execute(&mut tx)
            .await?;

        tx.commit().await?;
        Ok(deleted.rows_affected())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorRequest {
    pub name: String,
    pub version: Option<String>,
    pub ip_addr: IpNetwork,
    pub host_id: Uuid,
    pub user_id: Option<Uuid>,
    pub address: Option<String>,
    pub swarm_key: Option<Vec<u8>>,
    pub stake_status: StakeStatus,
    pub status: ValidatorStatus,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatorStatusRequest {
    pub version: Option<String>,
    pub stake_status: StakeStatus,
    pub status: ValidatorStatus,
    pub score: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reward {
    pub id: Uuid,
    pub block: i64,
    pub transaction_hash: String,
    pub time: i64,
    pub validator_id: Uuid,
    pub account: String,
    pub amount: i64,
}
