use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::Argon2;
use chrono::{DateTime, Utc};
use diesel::{dsl, prelude::*};
use diesel_async::RunQueryDsl;
use password_hash::PasswordVerifier;
use rand::rngs::OsRng;
use validator::Validate;

use crate::auth::resource::{OrgId, UserId};
use crate::database::Conn;
use crate::error::QueryError;

use super::org::NewOrg;
use super::schema::users;

#[derive(Debug, Clone, Queryable, AsChangeset, Selectable)]
#[diesel(treat_none_as_null = false)]
pub struct User {
    pub id: UserId,
    pub email: String,
    pub hashword: String,
    pub salt: String,
    pub created_at: DateTime<Utc>,
    pub first_name: String,
    pub last_name: String,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub billing_id: Option<String>,
    // TODO: drop this column again when sc-2322 (RBAC) is ready
    pub is_blockjoy_admin: bool,
}

type NotDeleted = dsl::Filter<users::table, dsl::IsNull<users::deleted_at>>;

impl User {
    pub async fn find_by_id(id: UserId, conn: &mut Conn<'_>) -> crate::Result<Self> {
        User::not_deleted()
            .find(id)
            .get_result(conn)
            .await
            .for_table_id("users", id)
    }

    pub async fn find_all(conn: &mut Conn<'_>) -> crate::Result<Vec<Self>> {
        users::table.get_results(conn).await.for_table("users")
    }

    pub async fn find_by_ids(
        mut user_ids: Vec<UserId>,
        conn: &mut Conn<'_>,
    ) -> crate::Result<Vec<Self>> {
        user_ids.sort();
        user_ids.dedup();
        Self::not_deleted()
            .filter(users::id.eq_any(user_ids))
            .get_results(conn)
            .await
            .for_table("users")
    }

    pub async fn find_by_email(email: &str, conn: &mut Conn<'_>) -> crate::Result<Self> {
        Self::not_deleted()
            .filter(super::lower(users::email).eq(&email.trim().to_lowercase()))
            .get_result(conn)
            .await
            .for_table_id("users", email)
    }

    pub async fn filter(
        org_id: Option<OrgId>,
        email_like: Option<&str>,
        conn: &mut Conn<'_>,
    ) -> crate::Result<Vec<Self>> {
        use crate::models::schema::orgs_users;

        let mut query = Self::not_deleted()
            .left_join(orgs_users::table)
            .into_boxed();

        if let Some(org_id) = org_id {
            query = query.filter(orgs_users::org_id.eq(org_id));
        }
        if let Some(email_like) = email_like {
            query = query.filter(super::lower(users::email).like(email_like.trim().to_lowercase()));
        }

        query
            .select(User::as_select())
            .distinct()
            .get_results(conn)
            .await
            .for_table("users")
    }

    pub fn verify_password(&self, password: &str) -> crate::Result<()> {
        let arg2 = Argon2::default();
        let hash = argon2::PasswordHash {
            algorithm: argon2::Algorithm::default().ident(),
            version: None,
            params: Default::default(),
            salt: Some(password_hash::Salt::from_b64(&self.salt)?),
            hash: Some(self.hashword.parse()?),
        };
        arg2.verify_password(password.as_bytes(), &hash)
            .map_err(|_| crate::Error::invalid_auth("Invalid email or password."))
    }

    pub async fn update(&self, conn: &mut Conn<'_>) -> crate::Result<Self> {
        let updated = diesel::update(users::table.find(self.id))
            .set(self)
            .get_result(conn)
            .await
            .for_table("users")?;
        Ok(updated)
    }

    pub async fn update_password(
        &self,
        password: &str,
        conn: &mut Conn<'_>,
    ) -> crate::Result<Self> {
        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        if let Some(hashword) = argon2.hash_password(password.as_bytes(), &salt)?.hash {
            let user = diesel::update(users::table.find(self.id))
                .set((
                    users::hashword.eq(hashword.to_string()),
                    users::salt.eq(salt.as_str()),
                ))
                .get_result(conn)
                .await
                .for_table("users")?;
            Ok(user)
        } else {
            Err(crate::Error::ValidationError(
                "Invalid password.".to_string(),
            ))
        }
    }

    /// Check if user can be found by email, is confirmed and has provided a valid password
    pub async fn login(email: &str, password: &str, conn: &mut Conn<'_>) -> crate::Result<Self> {
        let user = Self::find_by_email(email, conn)
            .await
            .map_err(|_e| crate::Error::invalid_auth("Email or password is invalid."))?;

        if User::is_confirmed(user.id, conn).await? {
            user.verify_password(password)?;
            Ok(user)
        } else {
            Err(crate::Error::UserConfirmationError)
        }
    }

    pub async fn confirm(user_id: UserId, conn: &mut Conn<'_>) -> crate::Result<()> {
        let target_user = Self::not_deleted()
            .find(user_id)
            .filter(users::confirmed_at.is_null());
        let n_updated = diesel::update(target_user)
            .set(users::confirmed_at.eq(chrono::Utc::now()))
            .execute(conn)
            .await
            .for_table("users")?;
        if n_updated == 0 {
            // This is the slow path, now we find out what went wrong. We either propagate the
            // NotFound error generated by is_confirmed or we handle the cases where the row user
            // was already cofirmed.
            if Self::is_confirmed(user_id, conn).await? {
                Err(crate::Error::validation("user was already confirmed"))
            } else {
                Err(crate::Error::unexpected("could not update row"))
            }
        } else {
            Ok(())
        }
    }

    pub async fn is_confirmed(id: UserId, conn: &mut Conn<'_>) -> crate::Result<bool> {
        Self::not_deleted()
            .find(id)
            .select(users::confirmed_at.is_not_null())
            .get_result(conn)
            .await
            .for_table_id("users", id)
    }

    /// Mark user deleted if no more nodes belong to it
    pub async fn delete(id: UserId, conn: &mut Conn<'_>) -> crate::Result<()> {
        diesel::update(users::table.find(id))
            .set(users::deleted_at.eq(chrono::Utc::now()))
            .execute(conn)
            .await?;
        Ok(())
    }

    pub async fn delete_billing(&self, conn: &mut Conn<'_>) -> crate::Result<()> {
        let no_billing: Option<String> = None;
        diesel::update(users::table)
            .set(users::billing_id.eq(no_billing))
            .execute(conn)
            .await?;
        Ok(())
    }

    pub async fn is_blockjoy_admin(user_id: UserId, conn: &mut Conn<'_>) -> crate::Result<bool> {
        Self::find_by_id(user_id, conn)
            .await
            .map(|user| user.is_blockjoy_admin)
    }

    pub fn preferred_language(&self) -> &str {
        // Needs to be done later, but we want to have some stub in place so we keep our code aware
        // of language differences.
        "en"
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    fn not_deleted() -> NotDeleted {
        users::table.filter(users::deleted_at.is_null())
    }
}

#[derive(Debug, Clone, Validate, Insertable)]
#[diesel(table_name = users)]
pub struct NewUser<'a> {
    #[validate(email)]
    email: String,
    first_name: &'a str,
    last_name: &'a str,
    hashword: String,
    salt: String,
}

impl<'a> NewUser<'a> {
    pub fn new(
        email: &'a str,
        first_name: &'a str,
        last_name: &'a str,
        password: &'a str,
    ) -> crate::Result<Self> {
        let argon2 = Argon2::default();
        let salt = SaltString::generate(&mut OsRng);
        if let Some(hashword) = argon2.hash_password(password.as_bytes(), &salt)?.hash {
            let create_user = Self {
                email: email.trim().to_lowercase(),
                first_name,
                last_name,
                hashword: hashword.to_string(),
                salt: salt.as_str().to_owned(),
            };

            create_user
                .validate()
                .map_err(|e| crate::Error::ValidationError(e.to_string()))?;
            Ok(create_user)
        } else {
            Err(crate::Error::ValidationError(
                "Invalid password.".to_string(),
            ))
        }
    }

    pub async fn create(self, conn: &mut Conn<'_>) -> crate::Result<User> {
        let user: User = diesel::insert_into(users::table)
            .values(self)
            .get_result(conn)
            .await?;

        let org = NewOrg {
            name: "Personal",
            is_personal: true,
        };
        org.create(user.id, conn).await?;

        Ok(user)
    }
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateUser<'a> {
    pub id: UserId,
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
    pub is_blockjoy_admin: Option<bool>,
}

impl<'a> UpdateUser<'a> {
    pub async fn update(self, conn: &mut Conn<'_>) -> crate::Result<User> {
        let user = diesel::update(users::table.find(self.id))
            .set(self)
            .get_result(conn)
            .await?;

        Ok(user)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_password_is_backwards_compatible() {
        let user = User {
            id: Uuid::new_v4().into(),
            email: "shitballer@joe.com".to_string(),
            hashword: "8reOLS3bLZB4vQvqy8Xqoa+mS82d9qidx7j1KTtmICY".to_string(),
            salt: "s2UTzLjLAz4xzhDBTFQtcg".to_string(),
            created_at: chrono::Utc::now(),
            first_name: "Joe".to_string(),
            last_name: "Ballington".to_string(),
            confirmed_at: Some(chrono::Utc::now()),
            deleted_at: None,
            billing_id: None,
            is_blockjoy_admin: false,
        };
        user.verify_password("A password that cannot be hacked!1")
            .unwrap()
    }
}
