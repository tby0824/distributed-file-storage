use crate::db::users;
use crate::utils::{hashing::verify_password, jwt::generate_jwt};
use sqlx::Pool;
use sqlx::Postgres;

use tonic::{Status};

pub async fn authenticate(pool: &Pool<Postgres>, secret: &str, username: &str, password: &str) -> Result<String, Status> {
    let user = users::get_user_by_username(pool, username).await.map_err(|_| Status::unauthenticated("User not found"))?;
    if verify_password(&user.password, password) {
        let token = generate_jwt(user.user_id, user.role.as_deref().unwrap_or("user"), secret)
            .map_err(|_| Status::internal("JWT generation failed"))?;
        Ok(token)
    } else {
        Err(Status::unauthenticated("Invalid credentials"))
    }
}
