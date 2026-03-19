use amqprs::{BasicProperties, Deliver, channel::{BasicAckArguments, Channel}, consumer::AsyncConsumer};
use async_trait::async_trait;
use revolt_database::Message;
use revolt_search::ElasticsearchClient;

pub struct MessageEditConsumer(pub ElasticsearchClient);

#[async_trait]
impl AsyncConsumer for MessageEditConsumer {
    async fn consume(
        &mut self,
        channel: &Channel,
        deliver: Deliver,
        _basic_properties: BasicProperties,
        content: Vec<u8>,
    ) {
        let message = serde_json::from_slice::<Message>(&content).expect("Failed to decode message");

        if self.0.edit_message(message).await.is_ok() {
            channel.basic_ack(BasicAckArguments::new(deliver.delivery_tag(), false)).await.expect("Failed to ack");
        } else {
            // todo requeue
        }
    }
}