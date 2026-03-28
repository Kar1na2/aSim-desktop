use serde::Serialize;
use chrono::NaiveDate;

#[derive(Debug, Serialize)]
#[warn(dead_code)]
pub struct LoginRequest {
    pub username: String, 
    pub password: String,
}

#[derive(Debug, Serialize)]
#[warn(dead_code)]
pub struct LoginResponse {
    pub uuid: String
}

#[derive(Debug, Serialize, Clone)]
pub struct UserProfile {
    pub uuid: String, 
    pub name: String, 
    pub username: String, 
    pub gender: String, 
    pub dob: NaiveDate, 
    pub star_sign: String, 
    pub interests: Vec<String>,
}