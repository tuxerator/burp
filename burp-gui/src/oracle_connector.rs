use burp::events::Events;
use tokio::sync::mpsc::Receiver;

pub struct Connector {
    reciever: Receiver<Events>,
}
