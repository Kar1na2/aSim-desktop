use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::{self as dynamodb, operation::create_table::CreateTableError, types::AttributeValue};
use aws_sdk_dynamodb::operation::put_item::PutItemError;
use rand::Rng;
use tracing::{event, Level};
use sha2::{Sha256, Digest};
use crate::models::error::AuthError;

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
            .table_name("test_table")
            .send()
            .await;

        match response {
            Ok(output) => {
                // Safely extract the table and its status
                if let Some(table) = output.table() {
                    if table.table_status() == Some(&dynamodb::types::TableStatus::Active) {
                        event!(Level::INFO, "Table '{}' is now ACTIVE", table_name);
                        break; // Table is ready, exit the loop
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
) -> bool {
    let create_test = client
        .create_table()
        .table_name(table_name)
        // 1. Define the attributes that will be used as keys
        .attribute_definitions(
            dynamodb::types::AttributeDefinition::builder()
                .attribute_name("id")
                .attribute_type(dynamodb::types::ScalarAttributeType::S) // 'S' for String
                .build()
                .expect("Failed to build attribute definition"),
        )
        // 2. Define the Key Schema (Hash = Partition Key)
        .key_schema(
            dynamodb::types::KeySchemaElement::builder()
                .attribute_name("id")
                .key_type(dynamodb::types::KeyType::Hash)
                .build()
                .expect("Failed to build key schema"),
        )
        // 3. Billing Mode (PayPerRequest is best for local/testing)
        .billing_mode(dynamodb::types::BillingMode::PayPerRequest)
        .send()
        .await;

    if let Err(err) = create_test {
        // Safely check if the error is a service error, and specifically a ResourceInUseException
        let table_already_exists = matches!(
            err.as_service_error(),
            Some(CreateTableError::ResourceInUseException(_))
        );

        if !table_already_exists {
            // 1. Pretty-print the massive, detailed AWS error struct to your terminal
            event!(Level::DEBUG, "❌ FATAL DYNAMODB ERROR: {:#?}", err);
            
            // 2. Return the actual error description so it bubbles up instead of a generic string
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

// Helper method mimicking Java's getHash
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
) -> Result<(), AuthError> {
    // 1. Generate 16 random bytes for the salt
    let mut salt_bytes = [0u8; 16];
    rand::rng().fill_bytes(&mut salt_bytes);
    let user_salt = encode_hex(&salt_bytes);

    // 2. Hash the password with the salt
    let passhash = get_hash(password, &user_salt);

    // 3. Put the item into DynamoDB
    let create_user = client
        .put_item()
        .table_name("test_table")
        .item("id", AttributeValue::S(username.to_string())) // 'id' acts as the username partition key
        .item("userSalt", AttributeValue::S(user_salt))
        .item("password", AttributeValue::S(passhash))
        .condition_expression("attribute_not_exists(id)") // Ensures we don't overwrite an existing user
        .send()
        .await;

    match create_user {
        Ok(_) => {
            event!(Level::INFO, "Successfully registered user: {}", username);
            Ok(())
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

pub async fn authenticate_user(
    client: &dynamodb::Client,
    username: &str,
    password: &str,
) -> Result<(), AuthError> {
    // 1. Fetch the user item, mapping DB errors to an Internal error
    let get_req = client
        .get_item()
        .table_name("test_table")
        .key("id", AttributeValue::S(username.to_string()))
        .send()
        .await
        .map_err(|e| AuthError::Internal(format!("Database error: {}", e)))?;

    // 2. Safely unpack the item, returning a Client error if None
    let item = get_req.item.ok_or_else(|| {
        AuthError::Client("User not found".to_string())
    })?;

    // 3. Extract the stored salt and password hash
    let stored_salt = item
        .get("userSalt")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| {
            AuthError::Internal(format!("Missing salt in DB for User: {}", username))
        })?;
    
    let stored_hash = item
        .get("password")
        .and_then(|v| v.as_s().ok())
        .ok_or_else(|| {
            AuthError::Internal(format!("Missing password in DB for User: {}", username))
        })?;

    // 4. Hash the provided password with the retrieved salt
    let computed_hash = get_hash(password, stored_salt);

    // 5. Compare the hashes securely
    if computed_hash == *stored_hash {
        Ok(()) // Success
    } else {
        Err(AuthError::Client("Incorrect password".to_string()))
    }
}

// TESTING 
// test cases must change so that it can accomodate to a new test username and a new test case for logging in a preexisting user

#[cfg(test)]
mod tests {
    use super::*; // Pulls in authenticate_user, register_user, etc.

    // You will need to create a helper to init a test client.
    // In a real scenario, this should point to a local instance of DynamoDB (like LocalStack or DynamoDB Local) to avoid hitting production.
    async fn setup_test_db() -> aws_sdk_dynamodb::Client {
        let client = init_client().await.unwrap();
        create_table(&client, "test_table").await;
        client
    }

    #[tokio::test]
    async fn test_successful_login() {
        let client = setup_test_db().await;
        
        // Arrange: Register our test user
        let _ = register_user(&client, "test_user", "my_secure_password").await;

        // Act: Attempt to authenticate
        let result = authenticate_user(&client, "test_user", "my_secure_password").await;

        // Assert: Ensure the login was successful
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_failed_login_wrong_password() {
        let client = setup_test_db().await;
        let _ = register_user(&client, "test_user", "my_secure_password").await;

        // Act: Attempt with the wrong password
        let result = authenticate_user(&client, "test_user", "wrong_password").await;

        // Assert: Ensure it fails
        assert!(result.is_err());
    }
}