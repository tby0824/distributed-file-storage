use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use chrono::{Utc, Duration};

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
    pub role: String,
}

pub fn generate_jwt(user_id: Uuid, role: &str, secret: &str) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = Utc::now() + Duration::hours(24);
    let claims = Claims {
        sub: user_id,
        exp: exp.timestamp() as usize,
        role: role.to_string(),
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))
}

pub fn decode_jwt(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let data = decode::<Claims>(token, &DecodingKey::from_secret(secret.as_ref()), &Validation::default())?;
    Ok(data.claims)
}
