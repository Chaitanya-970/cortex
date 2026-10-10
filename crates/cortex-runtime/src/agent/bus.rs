//! Asynchronous MessageBus with bounded inboxes, routing keys, deduplication, and dead-letter queues.

use super::{MessageType, RoutingKey};
use chrono::{DateTime, Utc};
use cortex_core::{AgentId, CortexError, Result, RunId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::mpsc::{channel, Receiver, Sender};

const DEFAULT_DEDUP_CAPACITY: usize = 10_000;
const DEFAULT_DLQ_CAPACITY: usize = 1_024;
const DEFAULT_INBOX_CAPACITY: usize = 256;

/// Standard inter-agent message envelope carried over the [`MessageBus`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BusMessage {
    /// Unique message identifier.
    pub id: String,
    /// Originating agent identifier.
    pub sender: AgentId,
    /// Targeted recipient agent identifier, or broadcast/wildcard recipient.
    pub recipient: AgentId,
    /// Correlated run identifier.
    pub run_id: RunId,
    /// Correlated task identifier (if bound to a specific subtask).
    pub task_id: Option<String>,
    /// UTC timestamp of message generation.
    pub timestamp: DateTime<Utc>,
    /// Routing key specifying topic or channel.
    pub routing_key: RoutingKey,
    /// Category of message (Request, Response, Notification, Error).
    pub message_type: MessageType,
    /// Arbitrary JSON structured payload.
    pub payload: serde_json::Value,
    /// Optional correlation identifier linking requests and responses.
    pub correlation_id: Option<String>,
}

impl BusMessage {
    /// Construct a new inter-agent message envelope.
    pub fn new(
        id: impl Into<String>,
        sender: AgentId,
        recipient: AgentId,
        run_id: RunId,
        routing_key: RoutingKey,
        message_type: MessageType,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: id.into(),
            sender,
            recipient,
            run_id,
            task_id: None,
            timestamp: Utc::now(),
            routing_key,
            message_type,
            payload,
            correlation_id: None,
        }
    }

    /// Set correlated task identifier.
    pub fn with_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.task_id = Some(task_id.into());
        self
    }

    /// Set correlation identifier.
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }
}

/// Reason a message was relegated to the Dead-Letter Queue (DLQ).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeadLetterReason {
    /// Targeted recipient agent is not currently registered on the bus.
    RecipientNotFound(AgentId),
    /// Recipient's bounded inbox is saturated and cannot accept new messages.
    InboxFull {
        /// Targeted agent identifier whose inbox was full.
        recipient: AgentId,
        /// Maximum capacity limit of the recipient inbox.
        capacity: usize,
    },
    /// No routing key subscription matches the specified destination.
    UnroutableKey(RoutingKey),
    /// Recipient agent has been unregistered or revoked.
    RecipientUnregistered(AgentId),
}

/// A captured message envelope stored inside the Dead-Letter Queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadLetterEnvelope {
    /// The undelivered message.
    pub message: BusMessage,
    /// Root cause reason for dead-lettering.
    pub reason: DeadLetterReason,
    /// UTC timestamp when message was routed to DLQ.
    pub dropped_at: DateTime<Utc>,
}

/// Receiving endpoint handle for an individual registered agent.
pub struct BusReceiver {
    agent_id: AgentId,
    rx: Receiver<BusMessage>,
}

impl BusReceiver {
    /// Retrieve the owner agent identifier.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Asynchronously await the next incoming message on this inbox.
    pub async fn recv(&mut self) -> Option<BusMessage> {
        self.rx.recv().await
    }

    /// Non-blocking poll for the next available message.
    pub fn try_recv(
        &mut self,
    ) -> std::result::Result<BusMessage, tokio::sync::mpsc::error::TryRecvError> {
        self.rx.try_recv()
    }
}

struct RegisteredInbox {
    sender: Sender<BusMessage>,
    capacity: usize,
    routes: HashSet<RoutingKey>,
}

/// Asynchronous, concurrency-safe message bus for inter-agent communication.
///
/// Enforces bounded FIFO inboxes, exact and topic routing, duplicate message
/// rejection, and dead-letter queue (DLQ) containment.
#[derive(Clone)]
pub struct MessageBus {
    inboxes: Arc<RwLock<HashMap<AgentId, RegisteredInbox>>>,
    topic_routes: Arc<RwLock<Vec<(String, AgentId)>>>,
    dedup_ids: Arc<Mutex<(HashSet<String>, VecDeque<String>)>>,
    dlq: Arc<Mutex<VecDeque<DeadLetterEnvelope>>>,
    dedup_capacity: usize,
    dlq_capacity: usize,
    message_counter: Arc<AtomicU64>,
}

impl Default for MessageBus {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageBus {
    /// Create a new [`MessageBus`] with default capacities.
    pub fn new() -> Self {
        Self::with_capacities(DEFAULT_DEDUP_CAPACITY, DEFAULT_DLQ_CAPACITY)
    }

    /// Create a new [`MessageBus`] with customized deduplication and DLQ sizes.
    pub fn with_capacities(dedup_capacity: usize, dlq_capacity: usize) -> Self {
        Self {
            inboxes: Arc::new(RwLock::new(HashMap::new())),
            topic_routes: Arc::new(RwLock::new(Vec::new())),
            dedup_ids: Arc::new(Mutex::new((
                HashSet::with_capacity(dedup_capacity),
                VecDeque::with_capacity(dedup_capacity),
            ))),
            dlq: Arc::new(Mutex::new(VecDeque::with_capacity(dlq_capacity))),
            dedup_capacity: dedup_capacity.max(1),
            dlq_capacity: dlq_capacity.max(1),
            message_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Generate a sequential, concurrency-safe message ID.
    pub fn next_message_id(&self) -> String {
        let count = self.message_counter.fetch_add(1, Ordering::SeqCst);
        format!("msg_{count:06}")
    }

    /// Register an agent with a dedicated bounded inbox and optional accepted routes.
    pub fn register_agent(
        &self,
        agent_id: AgentId,
        capacity: Option<usize>,
        routes: Vec<RoutingKey>,
    ) -> Result<BusReceiver> {
        let cap = capacity.unwrap_or(DEFAULT_INBOX_CAPACITY).clamp(1, 4096);
        let mut inboxes = self.inboxes.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire inboxes write lock: {e}"))
        })?;

        if inboxes.contains_key(&agent_id) {
            return Err(CortexError::Validation(format!(
                "agent '{}' is already registered on the message bus",
                agent_id
            )));
        }

        let (tx, rx) = channel(cap);
        inboxes.insert(
            agent_id.clone(),
            RegisteredInbox {
                sender: tx,
                capacity: cap,
                routes: routes.into_iter().collect(),
            },
        );

        Ok(BusReceiver { agent_id, rx })
    }

    /// Unregister an agent from the message bus, closing its inbox.
    pub fn unregister_agent(&self, agent_id: &AgentId) -> Result<()> {
        let mut inboxes = self.inboxes.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire inboxes write lock: {e}"))
        })?;
        inboxes.remove(agent_id);

        let mut topics = self.topic_routes.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire topics write lock: {e}"))
        })?;
        topics.retain(|(_, id)| id != agent_id);

        Ok(())
    }

    /// Subscribe an agent to a topic prefix or pattern (e.g., `role.coder`, `broadcast`).
    pub fn subscribe_topic(&self, agent_id: AgentId, pattern: impl Into<String>) -> Result<()> {
        let inboxes = self.inboxes.read().map_err(|e| {
            CortexError::Internal(format!("failed to acquire inboxes read lock: {e}"))
        })?;
        if !inboxes.contains_key(&agent_id) {
            return Err(CortexError::NotFound(format!(
                "agent '{}' is not registered",
                agent_id
            )));
        }

        let mut topics = self.topic_routes.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire topics write lock: {e}"))
        })?;
        topics.push((pattern.into(), agent_id));
        Ok(())
    }

    /// Check if a specific agent is registered on this bus.
    pub fn is_registered(&self, agent_id: &AgentId) -> bool {
        self.inboxes
            .read()
            .map(|inboxes| inboxes.contains_key(agent_id))
            .unwrap_or(false)
    }

    /// Number of active registered agents.
    pub fn registered_count(&self) -> usize {
        self.inboxes
            .read()
            .map(|inboxes| inboxes.len())
            .unwrap_or(0)
    }

    /// Publish an inter-agent message envelope.
    ///
    /// Validates uniqueness against the deduplication cache, performs exact or topic
    /// routing, and delegates unroutable or overflowed envelopes to the Dead-Letter Queue.
    pub async fn publish(&self, message: BusMessage) -> Result<()> {
        self.check_and_record_dedup(&message.id)?;

        let targets = self.resolve_targets(&message);
        if targets.is_empty() {
            let reason =
                if message.recipient.as_str() == "*" || message.recipient.as_str().is_empty() {
                    DeadLetterReason::UnroutableKey(message.routing_key.clone())
                } else {
                    DeadLetterReason::RecipientNotFound(message.recipient.clone())
                };
            self.send_to_dlq(message, reason);
            return Err(CortexError::Internal(
                "no active recipients for message".into(),
            ));
        }

        for (recipient, tx, _capacity) in targets {
            match tx.send(message.clone()).await {
                Ok(_) => {}
                Err(_) => {
                    self.send_to_dlq(
                        message.clone(),
                        DeadLetterReason::RecipientUnregistered(recipient),
                    );
                }
            }
        }

        Ok(())
    }

    /// Non-blocking publish for synchronous event emission.
    pub fn try_publish(&self, message: BusMessage) -> Result<()> {
        self.check_and_record_dedup(&message.id)?;

        let targets = self.resolve_targets(&message);
        if targets.is_empty() {
            let reason =
                if message.recipient.as_str() == "*" || message.recipient.as_str().is_empty() {
                    DeadLetterReason::UnroutableKey(message.routing_key.clone())
                } else {
                    DeadLetterReason::RecipientNotFound(message.recipient.clone())
                };
            self.send_to_dlq(message, reason);
            return Err(CortexError::Internal(
                "no active recipients for message".into(),
            ));
        }

        for (recipient, tx, capacity) in targets {
            match tx.try_send(message.clone()) {
                Ok(_) => {}
                Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                    self.send_to_dlq(
                        message.clone(),
                        DeadLetterReason::InboxFull {
                            recipient,
                            capacity,
                        },
                    );
                    return Err(CortexError::Internal("inbox buffer overflow".into()));
                }
                Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => {
                    self.send_to_dlq(
                        message.clone(),
                        DeadLetterReason::RecipientUnregistered(recipient),
                    );
                }
            }
        }

        Ok(())
    }

    fn resolve_targets(&self, message: &BusMessage) -> Vec<(AgentId, Sender<BusMessage>, usize)> {
        let inboxes = match self.inboxes.read() {
            Ok(lock) => lock,
            Err(_) => return Vec::new(),
        };

        // 1. Exact recipient match
        if inboxes.contains_key(&message.recipient) {
            let inbox = &inboxes[&message.recipient];
            if inbox.routes.is_empty() || inbox.routes.contains(&message.routing_key) {
                return vec![(
                    message.recipient.clone(),
                    inbox.sender.clone(),
                    inbox.capacity,
                )];
            }
        }

        // 2. Topic/wildcard resolution (when recipient is "*" or matches topic pattern)
        let mut results = Vec::new();
        if let Ok(topics) = self.topic_routes.read() {
            let rkey_str = message.routing_key.as_str();
            for (pattern, agent_id) in topics.iter() {
                if topic_matches(pattern, rkey_str) {
                    if let Some(inbox) = inboxes.get(agent_id) {
                        results.push((agent_id.clone(), inbox.sender.clone(), inbox.capacity));
                    }
                }
            }
        }

        results
    }

    fn check_and_record_dedup(&self, id: &str) -> Result<()> {
        let mut dedup = self
            .dedup_ids
            .lock()
            .map_err(|e| CortexError::Internal(format!("failed to acquire dedup lock: {e}")))?;

        let (set, queue) = &mut *dedup;
        if set.contains(id) {
            return Err(CortexError::Validation(format!(
                "duplicate message rejected: '{id}'"
            )));
        }

        if queue.len() >= self.dedup_capacity {
            if let Some(oldest) = queue.pop_front() {
                set.remove(&oldest);
            }
        }

        set.insert(id.to_string());
        queue.push_back(id.to_string());
        Ok(())
    }

    fn send_to_dlq(&self, message: BusMessage, reason: DeadLetterReason) {
        if let Ok(mut dlq) = self.dlq.lock() {
            if dlq.len() >= self.dlq_capacity {
                dlq.pop_front();
            }
            dlq.push_back(DeadLetterEnvelope {
                message,
                reason,
                dropped_at: Utc::now(),
            });
        }
    }

    /// Inspect current number of entries inside Dead-Letter Queue.
    pub fn dlq_len(&self) -> usize {
        self.dlq.lock().map(|d| d.len()).unwrap_or(0)
    }

    /// View all envelopes in Dead-Letter Queue without draining.
    pub fn dlq_messages(&self) -> Vec<DeadLetterEnvelope> {
        self.dlq
            .lock()
            .map(|d| d.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Drain all dead-lettered envelopes from queue.
    pub fn drain_dlq(&self) -> Vec<DeadLetterEnvelope> {
        self.dlq
            .lock()
            .map(|mut d| d.drain(..).collect())
            .unwrap_or_default()
    }

    /// Clear all entries from the Dead-Letter Queue.
    pub fn clear_dlq(&self) {
        if let Ok(mut d) = self.dlq.lock() {
            d.clear();
        }
    }
}

fn topic_matches(pattern: &str, routing_key: &str) -> bool {
    if pattern == "*" || pattern == "#" || pattern == routing_key {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix(".*") {
        return routing_key.starts_with(prefix);
    }
    if let Some(prefix) = pattern.strip_suffix(".#") {
        return routing_key.starts_with(prefix);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_bus_register_and_exact_delivery() {
        let bus = MessageBus::new();
        let agent_a = AgentId::from("worker_a");
        let route = RoutingKey::new("task.work").unwrap();

        let mut rx_a = bus
            .register_agent(agent_a.clone(), Some(10), vec![route.clone()])
            .unwrap();

        assert!(bus.is_registered(&agent_a));
        assert_eq!(bus.registered_count(), 1);

        let msg = BusMessage::new(
            "msg-01",
            AgentId::from("supervisor"),
            agent_a.clone(),
            RunId::from("run-1"),
            route.clone(),
            MessageType::Request,
            json!({"action": "compute"}),
        );

        bus.publish(msg.clone()).await.unwrap();

        let received = rx_a.recv().await.expect("receive message");
        assert_eq!(received.id, "msg-01");
        assert_eq!(received.sender.as_str(), "supervisor");
        assert_eq!(received.payload["action"], "compute");
    }

    #[tokio::test]
    async fn test_duplicate_message_rejection() {
        let bus = MessageBus::new();
        let agent_a = AgentId::from("agent_a");
        let route = RoutingKey::new("event.log").unwrap();

        let _rx = bus
            .register_agent(agent_a.clone(), Some(10), vec![route.clone()])
            .unwrap();

        let msg1 = BusMessage::new(
            "msg-dup-01",
            AgentId::from("sender"),
            agent_a.clone(),
            RunId::from("run-1"),
            route.clone(),
            MessageType::Notification,
            json!({"n": 1}),
        );

        bus.publish(msg1.clone()).await.unwrap();

        // Second submission with exact same ID must be rejected
        let res = bus.publish(msg1).await;
        assert!(res.is_err());
        assert!(res
            .unwrap_err()
            .to_string()
            .contains("duplicate message rejected"));
    }

    #[tokio::test]
    async fn test_unroutable_message_routes_to_dlq() {
        let bus = MessageBus::new();
        let unknown_agent = AgentId::from("non_existent_worker");
        let route = RoutingKey::new("task.delegate").unwrap();

        let msg = BusMessage::new(
            "msg-dlq-01",
            AgentId::from("lead"),
            unknown_agent.clone(),
            RunId::from("run-1"),
            route,
            MessageType::Request,
            json!({"task": "parse"}),
        );

        let res = bus.publish(msg.clone()).await;
        assert!(res.is_err());

        assert_eq!(bus.dlq_len(), 1);
        let dlq_items = bus.drain_dlq();
        assert_eq!(dlq_items.len(), 1);
        assert_eq!(dlq_items[0].message.id, "msg-dlq-01");
        assert_eq!(
            dlq_items[0].reason,
            DeadLetterReason::RecipientNotFound(unknown_agent)
        );
    }

    #[test]
    fn test_inbox_overflow_routes_to_dlq_on_try_publish() {
        let bus = MessageBus::new();
        let agent = AgentId::from("busy_worker");
        let route = RoutingKey::new("fast.stream").unwrap();

        // Tiny capacity of 1
        let _rx = bus
            .register_agent(agent.clone(), Some(1), vec![route.clone()])
            .unwrap();

        let msg1 = BusMessage::new(
            "m-1",
            AgentId::from("sender"),
            agent.clone(),
            RunId::from("run-1"),
            route.clone(),
            MessageType::Notification,
            json!({}),
        );
        let msg2 = BusMessage::new(
            "m-2",
            AgentId::from("sender"),
            agent.clone(),
            RunId::from("run-1"),
            route.clone(),
            MessageType::Notification,
            json!({}),
        );

        assert!(bus.try_publish(msg1).is_ok());

        // Second message overflows buffer
        let res = bus.try_publish(msg2);
        assert!(res.is_err());

        assert_eq!(bus.dlq_len(), 1);
        let dlq = bus.drain_dlq();
        assert_eq!(
            dlq[0].reason,
            DeadLetterReason::InboxFull {
                recipient: agent,
                capacity: 1,
            }
        );
    }

    #[tokio::test]
    async fn test_topic_pattern_subscription() {
        let bus = MessageBus::new();
        let coder1 = AgentId::from("coder_1");
        let coder2 = AgentId::from("coder_2");

        let mut rx1 = bus
            .register_agent(coder1.clone(), Some(10), vec![])
            .unwrap();
        let mut rx2 = bus
            .register_agent(coder2.clone(), Some(10), vec![])
            .unwrap();

        bus.subscribe_topic(coder1.clone(), "role.coder.*").unwrap();
        bus.subscribe_topic(coder2.clone(), "role.coder.*").unwrap();

        let topic_route = RoutingKey::new("role.coder.backend").unwrap();
        let broadcast_msg = BusMessage::new(
            "topic-01",
            AgentId::from("lead"),
            AgentId::from("*"),
            RunId::from("run-topic"),
            topic_route,
            MessageType::Notification,
            json!({"alert": "deploy"}),
        );

        bus.publish(broadcast_msg).await.unwrap();

        let m1 = rx1.recv().await.expect("coder 1 received");
        let m2 = rx2.recv().await.expect("coder 2 received");
        assert_eq!(m1.id, "topic-01");
        assert_eq!(m2.id, "topic-01");
    }
}
