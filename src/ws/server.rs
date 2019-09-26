use super::session::SessionMessage;
use crate::metrics::aggregator::NodeMetrics;
use crate::metrics::hub::MetricHub;
use crate::ws::session::{Connect, Disconnect};
use actix::prelude::*;
use log::info;
use rand::prelude::*;
use serde_derive::{Deserialize, Serialize};
use serde_json;
use std::cell::RefCell;
use std::collections::HashMap;

#[derive(Message)]
pub struct InboundMessage {
    pub session_id: usize,
    pub subscribe_to: Subscription,
}

#[derive(Message, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum MessageData {
    NodeMetrics(Vec<NodeMetrics>),
    ClusterMetrics(Vec<NodeMetrics>),
}

#[derive(Message, Clone, Serialize)]
pub enum Receiver {
    Everyone,
    SubscribersOf(Subscription),
    Only(usize),
}

#[derive(Message, Clone, Serialize)]
pub struct OutboundMessage {
    #[serde(skip)]
    pub receiver: Receiver,
    #[serde(flatten)]
    pub data: MessageData,
}

// TODO: Create timeframe enum? Implement Copy?
#[derive(Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum Subscription {
    OverviewOneSecond,
    OverviewFiveSeconds,
}

struct Client {
    address: Recipient<SessionMessage>,
    subscription: Subscription,
}

// TODO: No longer needed?
#[derive(Message)]
pub struct ClientJoined {
    pub session_id: usize,
    pub ws_server: Addr<WsServer>,
    pub subscribe_to: Subscription,
}

/// WebSocket Server
///
/// Manages communication with Clients
pub struct WsServer {
    hub: Addr<MetricHub>,
    sessions: HashMap<usize, Client>,
    rng: RefCell<ThreadRng>,
}

impl WsServer {
    pub fn new(hub: Addr<MetricHub>) -> Self {
        Self {
            hub,
            sessions: HashMap::new(),
            rng: RefCell::new(rand::thread_rng()),
        }
    }

    fn broadcast(&mut self, message: &str) {
        self.sessions.values().for_each(|c| {
            let _ = c.address.do_send(SessionMessage(message.to_owned()));
        })
    }

    fn multicast(&mut self, message: &str, subscription: Subscription) {
        self.sessions
            .values()
            .filter(|&c| c.subscription == subscription)
            .for_each(|c| {
                let _ = c.address.do_send(SessionMessage(message.to_owned()));
            })
    }

    fn unicast(&mut self, message: &str, receiver: usize) {
        if let Some(client) = self.sessions.get(&receiver) {
            let _ = client.address.do_send(SessionMessage(message.to_owned()));
        }
    }
}

impl Actor for WsServer {
    type Context = Context<Self>;
}

impl Handler<Connect> for WsServer {
    type Result = usize;

    fn handle(&mut self, msg: Connect, ctx: &mut Context<Self>) -> Self::Result {
        let id = self.rng.borrow_mut().gen::<usize>();
        self.sessions.insert(
            id,
            Client {
                address: msg.addr.clone(),
                subscription: Subscription::OverviewOneSecond,
            },
        );

        info!(
            "Client {} connected. Active sessions: {}",
            msg.ip,
            self.sessions.len()
        );

        id
    }
}

impl Handler<Disconnect> for WsServer {
    type Result = ();

    fn handle(&mut self, msg: Disconnect, _: &mut Context<Self>) {
        self.sessions
            .remove(&msg.sender_id)
            .expect("There is a bug in handling of WS Disconnect messages");

        info!(
            "Client {} disconnected. Active sessions: {}",
            msg.ip,
            self.sessions.len()
        );
    }
}

impl Handler<OutboundMessage> for WsServer {
    type Result = ();

    fn handle(&mut self, msg: OutboundMessage, _: &mut Context<Self>) {
        let message = serde_json::to_string(&msg).unwrap();
        match msg.receiver {
            Receiver::Everyone => self.broadcast(message.as_str()),
            Receiver::SubscribersOf(s) => self.multicast(message.as_str(), s),
            Receiver::Only(id) => self.unicast(message.as_str(), id),
        }
    }
}

impl Handler<InboundMessage> for WsServer {
    type Result = ();

    fn handle(&mut self, msg: InboundMessage, ctx: &mut Context<Self>) {
        if let Some(client) = self.sessions.get_mut(&msg.session_id) {
            client.subscription = msg.subscribe_to.clone();
            self.hub.do_send(ClientJoined {
                ws_server: ctx.address(),
                session_id: msg.session_id,
                subscribe_to: msg.subscribe_to,
            });
        }
    }
}
