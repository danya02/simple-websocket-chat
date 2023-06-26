use common::ChatMessage;
use tokio::sync::{broadcast, mpsc};

pub async fn manage_messages(
    mut message_manager_rx: mpsc::Receiver<ChatMessage>,
    message_broadcaster_tx: broadcast::Sender<ChatMessage>,
) {
    // Loop waiting for new messages
    loop {
        let new_message = message_manager_rx
            .recv()
            .await
            .expect("Message channel is closing");
        // Once a message is received, broadcast it to the channel
        match message_broadcaster_tx.send(new_message) {
            Ok(_) => {}
            Err(_) => {
                eprintln!("Error sending message into the broadcaster transmitter (all message receivers are down?!)")
            }
        }
    }
}
