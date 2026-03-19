use revolt_config::capture_error;
use revolt_database::{AMQP, Database};


pub async fn index_existing_messages(db: Database, amqp: AMQP) {
    let mut generator = db.fetch_all_messages().await.expect("Database query failed");

    while let Some(message) = generator.next().await {
        if let Err(e) = amqp.new_message_search(message).await {
            log::error!("Error pushing message to RabbitMQ: {e}");
            capture_error(&e);
        }
    }
}