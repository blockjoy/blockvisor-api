use std::collections::{HashSet, VecDeque};

use argon2::password_hash::{PasswordHasher, SaltString};
use argon2::{Algorithm, Argon2, PasswordHash};
use chrono::{DateTime, Utc};
use diesel::dsl::{self, LeftJoinQuerySource};
use diesel::expression::expression_types::NotSelectable;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::result::DatabaseErrorKind::UniqueViolation;
use diesel::result::Error::{DatabaseError, NotFound};
use diesel::sql_types::Bool;
use diesel_async::RunQueryDsl;
use displaydoc::Display;
use password_hash::{PasswordVerifier, Salt};
use rand::rngs::OsRng;
use thiserror::Error;
use tonic::Status;
use validator::Validate;

use crate::auth::resource::{OrgId, UserId};
use crate::database::Conn;
use crate::email::Language;
use crate::util::{SearchOperator, SortOrder};

use super::org::NewOrg;
use super::schema::{user_roles, users};
use super::Paginate;

pub mod setting;

type NotDeleted = dsl::Filter<users::table, dsl::IsNull<users::deleted_at>>;

#[derive(Debug, Display, Error)]
pub enum Error {
    /// User is already confirmed.
    AlreadyConfirmed,
    /// Failed to create new user: {0}
    Create(diesel::result::Error),
    /// Failed to confirm user: {0}
    Confirm(diesel::result::Error),
    /// No user was found to confirm.
    ConfirmNone,
    /// Failed to mark user as deleted: {0}
    Delete(diesel::result::Error),
    /// Failed to delete user billing: {0}
    DeleteBilling(diesel::result::Error),
    /// Failed to find users: {0}
    FindAll(diesel::result::Error),
    /// Failed to find user for email `{0}`: {1}
    FindByEmail(String, diesel::result::Error),
    /// Failed to find user for id `{0}`: {1}
    FindById(UserId, diesel::result::Error),
    /// Failed to find users by ids `{0:?}`: {1}
    FindByIds(HashSet<UserId>, diesel::result::Error),
    /// Failed to check if user `{0}` is confirmed: {1}
    IsConfirmed(UserId, diesel::result::Error),
    /// Login failed because no email was found.
    LoginEmail,
    /// Missing password hash.
    MissingHash,
    /// User is not confirmed.
    NotConfirmed,
    /// User org model error: {0}
    Org(#[from] crate::models::org::Error),
    /// User pagination: {0}
    Paginate(#[from] crate::models::paginate::Error),
    /// Failed to parse password hash: {0}
    ParseHash(password_hash::Error),
    /// Failed to parse Salt: {0}
    ParseSalt(password_hash::Error),
    /// User RBAC error: {0}
    Rbac(#[from] crate::models::rbac::Error),
    /// Failed to update user: {0}
    Update(diesel::result::Error),
    /// Failed to update user `{0}`: {1}
    UpdateId(UserId, diesel::result::Error),
    /// Failed to update password: {0}
    UpdatePassword(diesel::result::Error),
    /// Failed to validate new user: {0}
    ValidateNew(validator::ValidationErrors),
    /// Failed to verify password: {0}
    VerifyPassword(argon2::password_hash::Error),
}

impl From<Error> for Status {
    fn from(err: Error) -> Self {
        use Error::*;
        match err {
            Create(DatabaseError(UniqueViolation, _)) => Status::already_exists("Already exists."),
            ConfirmNone
            | Delete(NotFound)
            | DeleteBilling(NotFound)
            | FindAll(NotFound)
            | FindByEmail(_, NotFound)
            | FindById(_, NotFound)
            | FindByIds(_, NotFound) => Status::not_found("Not found."),
            AlreadyConfirmed => Status::failed_precondition("Already confirmed."),
            NotConfirmed => Status::failed_precondition("User is not confirmed."),
            LoginEmail | VerifyPassword(_) => Status::unauthenticated("Invalid email or password."),
            Paginate(err) => err.into(),
            Org(err) => err.into(),
            Rbac(err) => err.into(),
            _ => Status::internal("Internal error."),
        }
    }
}

#[derive(Clone, Debug, AsChangeset, Queryable, Selectable)]
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
    pub chargebee_billing_id: Option<String>,
    pub stripe_customer_id: Option<String>,
}

impl User {
    pub async fn by_id(id: UserId, conn: &mut Conn<'_>) -> Result<Self, Error> {
        User::not_deleted()
            .find(id)
            .get_result(conn)
            .await
            .map_err(|err| Error::FindById(id, err))
    }

    pub async fn by_ids(
        user_ids: HashSet<UserId>,
        conn: &mut Conn<'_>,
    ) -> Result<Vec<Self>, Error> {
        Self::not_deleted()
            .filter(users::id.eq_any(user_ids.iter()))
            .get_results(conn)
            .await
            .map_err(|err| Error::FindByIds(user_ids, err))
    }

    pub async fn by_email(email: &str, conn: &mut Conn<'_>) -> Result<Self, Error> {
        Self::not_deleted()
            .filter(super::lower(users::email).eq(&email.trim().to_lowercase()))
            .get_result(conn)
            .await
            .map_err(|err| Error::FindByEmail(email.to_lowercase(), err))
    }

    pub fn verify_password(&self, password: &str) -> Result<(), Error> {
        let hash = PasswordHash {
            algorithm: Algorithm::default().ident(),
            version: None,
            params: Default::default(),
            salt: Some(Salt::from_b64(&self.salt).map_err(Error::ParseSalt)?),
            hash: Some(self.hashword.parse().map_err(Error::ParseHash)?),
        };

        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .map_err(Error::VerifyPassword)
    }

    pub async fn update(&self, conn: &mut Conn<'_>) -> Result<Self, Error> {
        diesel::update(users::table.find(self.id))
            .set(self)
            .get_result(conn)
            .await
            .map_err(Error::Update)
    }

    pub async fn update_password(
        &self,
        password: &str,
        conn: &mut Conn<'_>,
    ) -> Result<Self, Error> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(Error::VerifyPassword)
            .and_then(|h| h.hash.ok_or(Error::MissingHash))?;

        diesel::update(users::table.find(self.id))
            .set((
                users::hashword.eq(hash.to_string()),
                users::salt.eq(salt.as_str()),
            ))
            .get_result(conn)
            .await
            .map_err(Error::UpdatePassword)
    }

    /// Check if user can be found by email, is confirmed and has provided a valid password
    pub async fn login(email: &str, password: &str, conn: &mut Conn<'_>) -> Result<Self, Error> {
        let user = match Self::by_email(email, conn).await {
            Ok(user) => Ok(user),
            Err(Error::FindByEmail(_, NotFound)) => Err(Error::LoginEmail),
            Err(err) => Err(err),
        }?;

        if User::is_confirmed(user.id, conn).await? {
            user.verify_password(password)?;
            Ok(user)
        } else {
            Err(Error::NotConfirmed)
        }
    }

    pub async fn confirm(user_id: UserId, conn: &mut Conn<'_>) -> Result<(), Error> {
        let target_user = Self::not_deleted()
            .find(user_id)
            .filter(users::confirmed_at.is_null());
        let updated = diesel::update(target_user)
            .set(users::confirmed_at.eq(chrono::Utc::now()))
            .execute(conn)
            .await
            .map_err(Error::Confirm)?;

        if updated == 0 && Self::is_confirmed(user_id, conn).await? {
            Err(Error::AlreadyConfirmed)
        } else if updated == 0 {
            Err(Error::ConfirmNone)
        } else {
            Ok(())
        }
    }

    pub async fn is_confirmed(id: UserId, conn: &mut Conn<'_>) -> Result<bool, Error> {
        Self::not_deleted()
            .find(id)
            .select(users::confirmed_at.is_not_null())
            .get_result(conn)
            .await
            .map_err(|err| Error::IsConfirmed(id, err))
    }

    /// Mark user deleted if no more nodes belong to it
    pub async fn delete(id: UserId, conn: &mut Conn<'_>) -> Result<(), Error> {
        diesel::update(users::table.find(id))
            .set(users::deleted_at.eq(chrono::Utc::now()))
            .execute(conn)
            .await
            .map(|_| ())
            .map_err(Error::Delete)
    }

    pub async fn delete_billing(&self, conn: &mut Conn<'_>) -> Result<(), Error> {
        diesel::update(users::table)
            .set(users::chargebee_billing_id.eq(None::<String>))
            .execute(conn)
            .await
            .map(|_| ())
            .map_err(Error::DeleteBilling)
    }

    // TODO: support other languages
    pub const fn preferred_language(&self) -> Language {
        Language::En
    }

    pub fn name(&self) -> String {
        format!("{} {}", self.first_name, self.last_name)
    }

    fn not_deleted() -> NotDeleted {
        users::table.filter(users::deleted_at.is_null())
    }
}

pub struct UserSearch {
    pub operator: SearchOperator,
    pub id: Option<String>,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub enum UserSort {
    Email(SortOrder),
    FirstName(SortOrder),
    LastName(SortOrder),
    CreatedAt(SortOrder),
}

impl UserSort {
    fn into_expr<T>(self) -> Box<dyn BoxableExpression<T, Pg, SqlType = NotSelectable>>
    where
        users::email: SelectableExpression<T>,
        users::first_name: SelectableExpression<T>,
        users::last_name: SelectableExpression<T>,
        users::created_at: SelectableExpression<T>,
    {
        use SortOrder::*;
        use UserSort::*;

        match self {
            Email(Asc) => Box::new(users::email.asc()),
            Email(Desc) => Box::new(users::email.desc()),

            FirstName(Asc) => Box::new(users::first_name.asc()),
            FirstName(Desc) => Box::new(users::first_name.desc()),

            LastName(Asc) => Box::new(users::last_name.asc()),
            LastName(Desc) => Box::new(users::last_name.desc()),

            CreatedAt(Asc) => Box::new(users::created_at.asc()),
            CreatedAt(Desc) => Box::new(users::created_at.desc()),
        }
    }
}

pub struct UserFilter {
    pub org_id: Option<OrgId>,
    pub offset: u64,
    pub limit: u64,
    pub search: Option<UserSearch>,
    pub sort: VecDeque<UserSort>,
}

impl UserFilter {
    pub async fn query(mut self, conn: &mut Conn<'_>) -> Result<(Vec<User>, u64), Error> {
        let mut query = users::table.left_join(user_roles::table).into_boxed();

        if let Some(search) = self.search {
            query = query.filter(search.into_expression());
        }

        if let Some(org_id) = self.org_id {
            query = query.filter(user_roles::org_id.eq(org_id));
        }

        if let Some(sort) = self.sort.pop_front() {
            query = query.order_by(sort.into_expr());
        } else {
            query = query.order_by(users::created_at.desc());
        }

        while let Some(sort) = self.sort.pop_front() {
            query = query.then_order_by(sort.into_expr());
        }

        query
            .filter(users::deleted_at.is_null())
            .select(User::as_select())
            .distinct()
            .paginate(self.limit, self.offset)?
            .count_results(conn)
            .await
            .map_err(Into::into)
    }
}

type UsersAndRoles = LeftJoinQuerySource<users::table, user_roles::table>;

impl UserSearch {
    fn into_expression(self) -> Box<dyn BoxableExpression<UsersAndRoles, Pg, SqlType = Bool>> {
        let user_name = users::first_name.concat(" ").concat(users::last_name);
        match self.operator {
            SearchOperator::Or => {
                let mut predicate: Box<dyn BoxableExpression<UsersAndRoles, Pg, SqlType = Bool>> =
                    Box::new(false.into_sql::<Bool>());
                if let Some(id) = self.id {
                    predicate = Box::new(predicate.or(super::text(users::id).like(id)));
                }
                if let Some(name) = self.name {
                    predicate = Box::new(predicate.or(super::lower(user_name).like(name)));
                }
                if let Some(email) = self.email {
                    predicate = Box::new(predicate.or(super::lower(users::email).like(email)));
                }
                predicate
            }
            SearchOperator::And => {
                let mut predicate: Box<dyn BoxableExpression<UsersAndRoles, Pg, SqlType = Bool>> =
                    Box::new(true.into_sql::<Bool>());
                if let Some(id) = self.id {
                    predicate = Box::new(predicate.and(super::text(users::id).like(id)));
                }
                if let Some(name) = self.name {
                    predicate = Box::new(predicate.and(super::lower(user_name).like(name)));
                }
                if let Some(email) = self.email {
                    predicate = Box::new(predicate.and(super::lower(users::email).like(email)));
                }
                predicate
            }
        }
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
    ) -> Result<Self, Error> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(Error::VerifyPassword)
            .and_then(|h| h.hash.ok_or(Error::MissingHash))?;

        let create_user = Self {
            email: email.trim().to_lowercase(),
            first_name,
            last_name,
            hashword: hash.to_string(),
            salt: salt.as_str().to_owned(),
        };

        create_user
            .validate()
            .map(|()| create_user)
            .map_err(Error::ValidateNew)
    }

    pub async fn create(self, conn: &mut Conn<'_>) -> Result<User, Error> {
        let user: User = diesel::insert_into(users::table)
            .values(self)
            .get_result(conn)
            .await
            .map_err(Error::Create)?;

        NewOrg::personal().create(user.id, conn).await?;

        Ok(user)
    }
}

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = users)]
pub struct UpdateUser<'a> {
    pub id: UserId,
    pub first_name: Option<&'a str>,
    pub last_name: Option<&'a str>,
}

impl<'a> UpdateUser<'a> {
    pub async fn update(self, conn: &mut Conn<'_>) -> Result<User, Error> {
        let user_id = self.id;
        diesel::update(users::table.find(user_id))
            .set(self)
            .get_result(conn)
            .await
            .map_err(|err| Error::UpdateId(user_id, err))
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
            chargebee_billing_id: None,
            stripe_customer_id: None,
        };
        user.verify_password("A password that cannot be hacked!1")
            .unwrap();
    }
}
