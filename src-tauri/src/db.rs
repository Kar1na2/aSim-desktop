use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::{self as dynamodb, operation::create_table::CreateTableError, types::AttributeValue};
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use rand::Rng;
use tracing::{event, Level};
use sha2::{Sha256, Digest};
use chrono::NaiveDate;
use crate::models::error::AuthError;
use crate::models::user::UserProfile;

pub async fn init_client() -> Result<dynamodb::Client, Box<dyn std::error::Error>> {
    event!(Level::INFO, "Initializing Client");

    let config = aws_config::defaults(BehaviorVersion::latest())
        .test_credentials() // Injects dummy credentials so the SDK doesn't fail
        .endpoint_url("http://localhost:8000") // The default port for local DynamoDB
        .region("us-west-1") // A region is still required by the SDK
        .load()
        .await;
    let client = dynamodb::Client::new(&config);

    Ok(client)
}

async fn table_active(
    client: &dynamodb::Client,
    table_name: &str,
) -> bool {
    let max_attempts = 20;
    let delay_seconds = 2;
    let mut attempts = 0;

    loop {
        let response = client
            .describe_table()
            .table_name(table_name)
            .send()
            .await;

        match response {
            Ok(output) => {
                if let Some(table) = output.table() {
                    if table.table_status() == Some(&dynamodb::types::TableStatus::Active) {
                        event!(Level::INFO, "Table '{}' is now ACTIVE", table_name);
                        break;
                    }
                }
            }
            Err(err) => {
                event!(Level::DEBUG, "Attempt {}: Failed to describe table: {}", attempts + 1, err);
            }
        }

        attempts += 1;
        if attempts >= max_attempts {
            event!(Level::DEBUG, "Timeout: Table did not become ACTIVE in the expected timeframe.");
            return false;
        }

        tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
    }
    
    true
}

pub async fn create_table(
    client: &dynamodb::Client,
    table_name: &str,
    pk_name: &str,
) -> bool {
    let create_test = client
        .create_table()
        .table_name(table_name)
        .attribute_definitions(
            dynamodb::types::AttributeDefinition::builder()
                .attribute_name(pk_name)
                .attribute_type(dynamodb::types::ScalarAttributeType::S)
                .build()
                .expect("Failed to build attribute definition"),
        )
        .key_schema(
            dynamodb::types::KeySchemaElement::builder()
                .attribute_name(pk_name)
                .key_type(dynamodb::types::KeyType::Hash)
                .build()
                .expect("Failed to build key schema"),
        )
        .billing_mode(dynamodb::types::BillingMode::PayPerRequest)
        .send()
        .await;

    if let Err(err) = create_test {
        let table_already_exists = matches!(
            err.as_service_error(),
            Some(CreateTableError::ResourceInUseException(_))
        );

        if !table_already_exists {
            event!(Level::DEBUG, "❌ FATAL DYNAMODB ERROR: {:#?}", err);
            event!(Level::DEBUG, "Failed to create table. Reason: {}", err);
            return false;
        } else {
            event!(Level::INFO, "Table already exists, skipping creation.");
        }
    }

    table_active(client, table_name).await
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        write!(&mut s, "{:02X}", b).unwrap();
    }
    s
}

fn get_hash(password: &str, salt: &str) -> String {
    let salted = format!("{}{}", salt, password);
    let mut hasher = Sha256::new();
    hasher.update(salted.as_bytes());
    let result = hasher.finalize();
    encode_hex(&result)
}

pub async fn register_user(
    client: &dynamodb::Client,
    username: &str,
    password: &str,
) -> Result<String, AuthError> {
    let internal_user_id = uuid::Uuid::new_v4().to_string();
    
    let mut salt_bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let user_salt = encode_hex(&salt_bytes);
    let passhash = get_hash(password, &user_salt);

    let create_user = client
        .put_item()
        .table_name("users_auth")
        .item("username", AttributeValue::S(username.to_string()))
        .item("uuid", AttributeValue::S(internal_user_id.clone()))
        .item("userSalt", AttributeValue::S(user_salt))
        .item("password", AttributeValue::S(passhash))
        .condition_expression("attribute_not_exists(username)")
        .send()
        .await;

    match create_user {
        Ok(_) => {
            event!(Level::INFO, "Successfully registered auth for: {}", username);
            Ok(internal_user_id)
        }
        Err(err) => {
            let user_already_exists = matches!(
                err.as_service_error(),
                Some(PutItemError::ConditionalCheckFailedException(_))
            );

            if user_already_exists {
                Err(AuthError::Client("User already exists".to_string()))
            } else {
                event!(Level::ERROR, "Database error: {:?}", err);
                Err(AuthError::Internal("Failed to register user".to_string()))
            }
        }
    }
}

pub async fn register_user_profile(
    client: &dynamodb::Client,
    profile: UserProfile,
) -> Result<(), AuthError> {
    let interests_attr: Vec<AttributeValue> = profile
        .interests
        .into_iter()
        .map(AttributeValue::S)
        .collect();

    let create_profile = client
        .put_item()
        .table_name("users_profiles")
        .item("uuid", AttributeValue::S(profile.uuid))
        .item("username", AttributeValue::S(profile.username.clone()))
        .item("name", AttributeValue::S(profile.name))
        .item("gender", AttributeValue::S(profile.gender))
        .item("dob", AttributeValue::S(profile.dob.to_string())) 
        .item("star_sign", AttributeValue::S(profile.star_sign))
        .item("interests", AttributeValue::L(interests_attr))
        // FIX: 'uuid' is a reserved DynamoDB word, so we map it to '#u' for the expression check
        .condition_expression("attribute_not_exists(#u)") 
        .expression_attribute_names("#u", "uuid")
        .send()
        .await;

    match create_profile {
        Ok(_) => {
            tracing::info!("Successfully registered profile for: {}", profile.username);
            Ok(())
        }
        Err(err) => {
            let profile_already_exists = matches!(
                err.as_service_error(),
                Some(PutItemError::ConditionalCheckFailedException(_))
            );

            if profile_already_exists {
                Err(AuthError::Client("Profile already exists for this user".to_string()))
            } else {
                tracing::error!("Database error while creating profile: {:?}", err);
                Err(AuthError::Internal(format!("Database error while creating profile: {:?}", err)))
            }
        }
    }
}

pub async fn authenticate_user(
    client: &dynamodb::Client,
    username: &str,
    password: &str,
) -> Result<UserProfile, AuthError> {
    let auth_req = client
        .get_item()
        .table_name("users_auth")
        .key("username", AttributeValue::S(username.to_string()))
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Auth DB error: {}", e)))?;

    let auth_item = auth_req.item.ok_or_else(|| AuthError::Client("User not found".to_string()))?;

    let stored_salt = auth_item.get("userSalt").and_then(|v| v.as_s().ok())
        .ok_or_else(|| AuthError::Internal("Missing salt in DB".into()))?;
    
    let stored_hash = auth_item.get("password").and_then(|v| v.as_s().ok())
        .ok_or_else(|| AuthError::Internal("Missing password in DB".into()))?;

    let computed_hash = get_hash(password, stored_salt);
    if computed_hash != *stored_hash {
        return Err(AuthError::Client("Incorrect password".to_string()));
    }

    let uuid = auth_item.get("uuid").and_then(|v| v.as_s().ok())
        .ok_or_else(|| AuthError::Internal("Missing UUID in DB".into()))?;

    let profile_req = client
        .get_item()
        .table_name("users_profiles")
        .key("uuid", AttributeValue::S(uuid.to_string()))
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Profile DB error: {}", e)))?;

    let profile_item = profile_req.item.ok_or_else(|| {
        AuthError::Internal("User profile missing for authenticated user".to_string())
    })?;

    let get_str = |key: &str| -> String {
        profile_item.get(key).and_then(|v| v.as_s().ok()).cloned().unwrap_or_default()
    };

    let dob_str = get_str("dob");
    let dob = NaiveDate::parse_from_str(&dob_str, "%Y-%m-%d").unwrap_or_else(|_| {
        NaiveDate::from_ymd_opt(1970, 1, 1).unwrap()
    });

    let interests = profile_item.get("interests").and_then(|v| v.as_l().ok())
        .map(|list| list.iter().filter_map(|attr| attr.as_s().ok().cloned()).collect())
        .unwrap_or_default();

    Ok(UserProfile {
        uuid: uuid.to_string(),
        name: get_str("name"),
        username: username.to_string(),
        gender: get_str("gender"),
        dob,
        star_sign: get_str("star_sign"),
        interests,
    })
}

pub async fn delete_user(
    client: &dynamodb::Client,
    uuid: &str,
) -> bool {
    // 1. Fetch profile to discover the `username` (needed for users_auth table)
    let profile_req = client
        .get_item()
        .table_name("users_profiles")
        .key("uuid", AttributeValue::S(uuid.to_string()))
        .send()
        .await;

    let username = match profile_req {
        Ok(output) => {
            if let Some(item) = output.item {
                if let Some(user_attr) = item.get("username") {
                    if let Ok(u) = user_attr.as_s() {
                        u.clone()
                    } else {
                        event!(Level::ERROR, "Failed to parse username from profile");
                        return false;
                    }
                } else {
                    event!(Level::ERROR, "Profile missing username attribute");
                    return false;
                }
            } else {
                event!(Level::ERROR, "User profile not found for deletion");
                return false;
            }
        }
        Err(e) => {
            event!(Level::ERROR, "Failed to fetch user profile for deletion: {:?}", e);
            return false;
        }
    };

    // 2. Delete from users_profiles
    let delete_profile = client
        .delete_item()
        .table_name("users_profiles")
        .key("uuid", AttributeValue::S(uuid.to_string()))
        .send()
        .await;

    if let Err(e) = delete_profile {
        event!(Level::ERROR, "Failed to delete user profile: {:?}", e);
        return false;
    }

    // 3. Delete from users_auth
    let delete_auth = client
        .delete_item()
        .table_name("users_auth")
        .key("username", AttributeValue::S(username))
        .send()
        .await;

    match delete_auth {
        Ok(_) => {
            event!(Level::INFO, "Successfully deleted user profile and auth for: {}", uuid);
            true
        }
        Err(e) => {
            event!(Level::ERROR, "Failed to delete user auth: {:?}", e);
            false
        }
    }
}

// TESTING 
#[cfg(test)]
mod tests {
    use super::*; 
    use chrono::NaiveDate;

    async fn setup_test_db() -> aws_sdk_dynamodb::Client {
        let client = init_client().await.expect("Failed to init client");
        create_table(&client, "users_auth", "username").await;
        create_table(&client, "users_profiles", "uuid").await;
        client
    }

    fn create_dummy_profile(uuid: String, username: String) -> UserProfile {
        UserProfile {
            uuid,
            username,
            name: "Test User".to_string(),
            gender: "Non-binary".to_string(),
            dob: NaiveDate::from_ymd_opt(2000, 1, 1).unwrap(),
            star_sign: "Capricorn".to_string(),
            interests: vec!["Rust".to_string(), "DynamoDB".to_string()],
        }
    }

    #[tokio::test]
    async fn test_successful_login() {
        let client = setup_test_db().await;
        // Generate a random username to prevent local state pollution between tests
        let test_id = uuid::Uuid::new_v4().to_string();
        let username = format!("test_user_success_{}", test_id);
        let password = "my_secure_password";
        
        let uuid = register_user(&client, &username, password).await.expect("Failed to register user");
        let profile = create_dummy_profile(uuid, username.clone());
        register_user_profile(&client, profile).await.expect("Failed to register profile");

        let result = authenticate_user(&client, &username, password).await;

        assert!(result.is_ok());
        let returned_profile = result.unwrap();
        assert_eq!(returned_profile.username, username);
        assert_eq!(returned_profile.interests.len(), 2);
    }

    #[tokio::test]
    async fn test_login_preexisting_user() {
        let client = setup_test_db().await;
        let test_id = uuid::Uuid::new_v4().to_string();
        let username = format!("test_user_preexisting_{}", test_id);
        let password = "preexisting_password";

        // Setup: Register the user and profile beforehand
        let uuid = register_user(&client, &username, password).await.unwrap();
        let profile = create_dummy_profile(uuid, username.clone());
        register_user_profile(&client, profile).await.unwrap();

        // Act: Attempt to log in with the preexisting user credentials
        let result = authenticate_user(&client, &username, password).await;

        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap().username, username);
    }

    #[tokio::test]
    async fn test_failed_login_wrong_password() {
        let client = setup_test_db().await;
        let test_id = uuid::Uuid::new_v4().to_string();
        let username = format!("test_user_wrong_pw_{}", test_id);
        let password = "my_secure_password";

        let uuid = register_user(&client, &username, password).await.unwrap();
        let profile = create_dummy_profile(uuid, username.clone());
        register_user_profile(&client, profile).await.unwrap();

        let result = authenticate_user(&client, &username, "wrong_password").await;

        assert!(result.is_err());
        if let Err(AuthError::Client(msg)) = result {
            assert_eq!(msg, "Incorrect password");
        } else {
            panic!("Expected AuthError::Client for incorrect password");
        }
    }

    #[tokio::test]
    async fn test_failed_login_missing_profile() {
        let client = setup_test_db().await;
        let test_id = uuid::Uuid::new_v4().to_string();
        let username = format!("test_user_no_profile_{}", test_id);
        let password = "my_secure_password";

        // Register auth ONLY
        let _uuid = register_user(&client, &username, password).await.unwrap();

        let result = authenticate_user(&client, &username, password).await;

        assert!(result.is_err());
        if let Err(AuthError::Internal(msg)) = result {
            assert!(msg.contains("User profile missing"));
        } else {
            panic!("Expected AuthError::Internal for missing profile data");
        }
    }

    #[tokio::test]
    async fn test_delete_user() {
        let client = setup_test_db().await;
        let test_id = uuid::Uuid::new_v4().to_string();
        let username = format!("test_user_delete_{}", test_id);
        let password = "delete_password";

        // Arrange: Register user and profile
        let uuid = register_user(&client, &username, password).await.unwrap();
        let profile = create_dummy_profile(uuid.clone(), username.clone());
        register_user_profile(&client, profile).await.unwrap();

        // Act: Delete user
        let deleted = delete_user(&client, &uuid).await;
        assert!(deleted);

        // Assert: Ensure login fails because user was deleted
        let result = authenticate_user(&client, &username, password).await;
        assert!(result.is_err());
        if let Err(AuthError::Client(msg)) = result {
            assert_eq!(msg, "User not found");
        } else {
            panic!("Expected AuthError::Client for missing user");
        }
    }
}