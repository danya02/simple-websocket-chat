use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize)]
pub enum ChatMessage {
    TextMessage {
        username: String,
        content: String,
        signature: Option<String>,
    },
    SystemMessage {
        content: String,
    },

    /// This message is sent by the client sometime at the start of the conversation
    /// so that the server knows what user the connection is associated with.
    /// When the server receives this, it will also emit a message saying that this client is connected.
    ConnectionUsername {
        username: String,
    }
}
