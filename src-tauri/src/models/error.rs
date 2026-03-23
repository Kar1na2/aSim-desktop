use serde::Serialize;

#[derive(Debug, Serialize)]
pub enum AuthError {
    Client(String), 
    Internal(String)
}
