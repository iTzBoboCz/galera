use chrono::{NaiveDateTime, Utc};
use diesel::{BoolExpressionMethods, Connection, ExpressionMethods, OptionalExtension, QueryDsl, RunQueryDsl, Table, };
use tracing::error;

use crate::{DbConn, models::{NewOidcIdentity, NewUser, OidcIdentity}, schema::{oidc_identity, user}};

/// Tries to select a user by its ID.
pub async fn get_user_by_oidc_subject(conn: DbConn, oidc_provider: String, oidc_subject: String) -> Result<Option<OidcIdentity>, diesel::result::Error> {
  let result = conn.interact(move |c| {
    oidc_identity::table
      .select(oidc_identity::table::all_columns())
      .filter(oidc_identity::provider_key.eq(oidc_provider).and(oidc_identity::subject.eq(oidc_subject)))
      .first::<OidcIdentity>(c)
      .optional()
  }).await
  .map_err(|e| {
    error!("DB interact failed in get_user_by_oidc_subject: {e}");
    diesel::result::Error::DatabaseError(
      diesel::result::DatabaseErrorKind::Unknown,
      Box::new(format!("interact failed: {e}")),
    )
  })??;

  Ok(result)
}

/// Inserts OIDC-only user account (passwordless), returns user_id
/// Both inserts run in a single transaction
pub async fn insert_oidc_user(conn: DbConn, oidc_provider: String, oidc_subject: String, email: String
) -> Result<i32, diesel::result::Error> {
  let new_user = NewUser::new_oidc(oidc_provider.clone(), oidc_subject.clone(), email);
  conn.interact(move |c| {
    c.transaction::<i32, diesel::result::Error, _>(|c| {
      // 1) insert user
      diesel::insert_into(user::table)
        .values(new_user.clone())
        .execute(c)?;

      // 2) get inserted id (MySQL)
      #[derive(diesel::deserialize::QueryableByName)]
      struct Row {
        #[diesel(sql_type = diesel::sql_types::Integer)]
        id: i32,
      }

      let Row { id: user_id } =
        diesel::sql_query("SELECT LAST_INSERT_ID() AS id")
          .get_result::<Row>(c)?;

      // 3) insert oidc identity
      let oidc = NewOidcIdentity {
        provider_key: oidc_provider,
        subject: oidc_subject,
        user_id,
        created_at: NaiveDateTime::from_timestamp(Utc::now().timestamp(), 0)
      };

      diesel::insert_into(oidc_identity::table)
        .values(oidc)
        .execute(c)?;

      Ok(user_id)
    })
  })
  .await
  .map_err(|_| diesel::result::Error::RollbackTransaction)?
}
