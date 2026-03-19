use amqprs::{BasicProperties, Deliver, channel::{BasicAckArguments, Channel}, consumer::AsyncConsumer};
use async_trait::async_trait;
use revolt_database::events::rabbit::MessageDeletePayload;
use revolt_search::ElasticsearchClient;

pub struct MessageDeleteConsumer(pub ElasticsearchClient);

#[async_trait]
impl AsyncConsumer for MessageDeleteConsumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        let payload = serde_json::from_slice::<MessageDeletePayload>(&content).expect("Failed to decode message");

        if self.0.delete_message(&payload.message_id).await.is_ok() {
            channel.basic_ack(BasicAckArguments::new(deliver.delivery_tag(), false)).await.expect("Failed to ack");
        } else {
            // todo requeue
        }
    }
}