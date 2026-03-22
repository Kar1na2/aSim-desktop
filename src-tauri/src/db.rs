use aws_config::BehaviorVersion;
use aws_sdk_dynamodb::{self as dynamodb, operation::create_table::CreateTableError, types::TableStatus, types::AttributeValue};

pub async fn init_client() -> Result<dynamodb::Client, Box<dyn std::error::Error>> {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .test_credentials() // Injects dummy credentials so the SDK doesn't fail
        .endpoint_url("http://localhost:8000") // The default port for local DynamoDB
        .region("us-west-1") // A region is still required by the SDK
        .load()
        .await;
    let client = dynamodb::Client::new(&config);

    let create_test = client
        .create_table()
        .table_name("test_table")
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
            eprintln!("❌ FATAL DYNAMODB ERROR: {:#?}", err);
            
            // 2. Return the actual error description so it bubbles up instead of a generic string
            return Err(format!("Failed to create table. Reason: {}", err).into());
        } else {
            println!("Table already exists, skipping creation.");
        }
    }

    if let Err(err) = table_active(&client).await {
        return Err(err);
    }

    Ok(client)
}

async fn table_active(client: &dynamodb::Client) -> Result<(), Box<dyn std::error::Error>> {
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
                        println!("Table 'test table' is now ACTIVE.");
                        break; // Table is ready, exit the loop
                    }
                }
            }
            Err(err) => {
                eprintln!("Attempt {}: Failed to describe table: {}", attempts + 1, err);
            }
        }

        attempts += 1;
        if attempts >= max_attempts {
            return Err("Timeout: Table did not become ACTIVE in the expected timeframe.".into());
        }

        tokio::time::sleep(std::time::Duration::from_secs(delay_seconds)).await;
    }
    
    Ok(())
}