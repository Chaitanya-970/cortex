//! Bounded agent inboxes and runtime-owned delegation relationships.

use super::{AgentManager, AgentMessagePayload, AgentState, RoutingKey};
use chrono::Utc;
use cortex_core::{AgentId, CortexError, ExecutionEvent, Redactor, Result, RunId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::poll_fn;
use std::task::{Poll, Waker};

const MAX_PENDING_EVENTS: usize = 4096;
const MAX_TASKS: usize = 4096;
const MAX_PAYLOAD_BYTES: usize = 65536;

/// A runtime-stamped message delivered to one explicitly addressed agent.
///
/// Deserialization creates data, not an authenticated sender or authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentMessage {
    /// Runtime-generated identifier, unique within this manager.
    pub id: String,
    /// Correlated execution run.
    pub run_id: RunId,
    /// Sender bound to the originating endpoint.
    pub sender: AgentId,
    /// Explicit recipient.
    pub recipient: AgentId,
    /// Exact routing key accepted by the recipient.
    pub routing_key: RoutingKey,
    /// UTC timestamp of enqueueing.
    pub timestamp: String,
    /// Typed data; receiving it does not execute instructions.
    pub payload: AgentMessagePayload,
}

impl From<&AgentMessage> for ExecutionEvent {
    fn from(msg: &AgentMessage) -> Self {
        let redactor = Redactor::new();
        let payload_json = serde_json::to_value(&msg.payload).unwrap_or_default();
        let redacted_payload = redactor.redact_json(&payload_json);
        ExecutionEvent::InterAgentMessage {
            run_id: msg.run_id.clone(),
            message_id: msg.id.clone(),
            sender: msg.sender.clone(),
            recipient: msg.recipient.clone(),
            routing_key: redactor.redact_text(msg.routing_key.as_str()),
            timestamp: msg.timestamp.clone(),
            payload: redacted_payload,
        }
    }
}

impl TryFrom<&ExecutionEvent> for AgentMessage {
    type Error = CortexError;

    fn try_from(event: &ExecutionEvent) -> Result<Self> {
        match event {
            ExecutionEvent::InterAgentMessage {
                run_id,
                message_id,
                sender,
                recipient,
                routing_key,
                timestamp,
                payload,
            } => {
                let rkey = RoutingKey::new(routing_key.as_str())?;
                let payload: AgentMessagePayload = serde_json::from_value(payload.clone())
                    .map_err(|e| {
                        CortexError::Internal(format!("failed to deserialize message payload: {e}"))
                    })?;
                Ok(Self {
                    id: message_id.clone(),
                    run_id: run_id.clone(),
                    sender: sender.clone(),
                    recipient: recipient.clone(),
                    routing_key: rkey,
                    timestamp: timestamp.clone(),
                    payload,
                })
            }
            _ => Err(CortexError::Validation(
                "event is not an InterAgentMessage".into(),
            )),
        }
    }
}

#[derive(Debug)]
struct Inbox {
    epoch: u64,
    capacity: usize,
    routes: HashSet<RoutingKey>,
    queue: VecDeque<AgentMessage>,
    waker: Option<Waker>,
}

#[derive(Debug)]
struct Delegation {
    supervisor: AgentId,
    worker: AgentId,
    pending: bool,
}

#[derive(Default)]
pub(super) struct CoordinationState {
    inboxes: HashMap<AgentId, Inbox>,
    parents: HashMap<AgentId, AgentId>,
    tasks: HashMap<(RunId, String), Delegation>,
    next_id: u64,
    events: VecDeque<ExecutionEvent>,
    redactor: Redactor,
}

impl std::fmt::Debug for CoordinationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoordinationState")
            .field("inboxes", &self.inboxes.len())
            .field("relationships", &self.parents.len())
            .field("tasks", &self.tasks.len())
            .field("pending_events", &self.events.len())
            .finish()
    }
}

impl CoordinationState {
    fn next_id(&mut self) -> Result<u64> {
        self.next_id = self.next_id.checked_add(1).ok_or_else(|| {
            CortexError::Internal("coordination identifier space exhausted".into())
        })?;
        Ok(self.next_id)
    }

    pub(super) fn notify(&mut self, id: &AgentId) -> Option<Waker> {
        self.inboxes
            .get_mut(id)
            .and_then(|inbox| inbox.waker.take())
    }

    pub(super) fn revoke(&mut self, id: &AgentId) -> Option<Waker> {
        for task in self.tasks.values_mut() {
            if &task.worker == id || &task.supervisor == id {
                task.pending = false;
            }
        }
        self.inboxes.remove(id).and_then(|inbox| inbox.waker)
    }

    pub(super) fn remove(&mut self, id: &AgentId) -> Option<Waker> {
        self.parents
            .retain(|worker, supervisor| worker != id && supervisor != id);
        self.revoke(id)
    }
}

/// An agent's authenticated inbox and sending handle, issued by the runtime.
///
/// Keep the manager in trusted host code. Do not expose endpoint creation to
/// model output. Endpoints carry no filesystem, shell, or tool capabilities.
#[derive(Debug)]
pub struct AgentEndpoint {
    manager: AgentManager,
    agent_id: AgentId,
    epoch: u64,
}

impl AgentManager {
    /// Issue one endpoint for a running agent, accepting exact routing keys.
    ///
    /// Capacity is 1..=1024 messages. One through 128 distinct keys are allowed.
    /// A live endpoint cannot be replaced; stop/restart revokes it, and dropping
    /// it closes its inbox. This is a trusted runtime operation.
    pub fn connect_agent(
        &self,
        agent_id: &AgentId,
        routes: Vec<RoutingKey>,
        capacity: usize,
    ) -> Result<AgentEndpoint> {
        if !(1..=1024).contains(&capacity) || routes.is_empty() || routes.len() > 128 {
            return Err(CortexError::Validation(
                "invalid inbox capacity or routing-key count".into(),
            ));
        }
        let agents = self.agents.read().map_err(lock_error)?;
        require_running(&agents, agent_id)?;
        let mut state = self.coordination.lock().map_err(lock_error)?;
        if state.inboxes.contains_key(agent_id) {
            return Err(CortexError::Validation(
                "agent already has an endpoint".into(),
            ));
        }
        let epoch = state.next_id()?;
        state.inboxes.insert(
            agent_id.clone(),
            Inbox {
                epoch,
                capacity,
                routes: routes.into_iter().collect(),
                queue: VecDeque::new(),
                waker: None,
            },
        );
        Ok(AgentEndpoint {
            manager: self.clone(),
            agent_id: agent_id.clone(),
            epoch,
        })
    }

    /// Assign a worker to a supervisor without changing either agent's permissions.
    ///
    /// Relationships may form a tree. Cycles and reassignment while the worker
    /// has a pending task are rejected. Agent role strings do not grant authority.
    pub fn assign_worker(&self, supervisor: &AgentId, worker: &AgentId) -> Result<()> {
        let agents = self.agents.read().map_err(lock_error)?;
        for id in [supervisor, worker] {
            if !agents.contains_key(id) {
                return Err(CortexError::NotFound(format!("agent '{id}' not found")));
            }
        }
        let mut state = self.coordination.lock().map_err(lock_error)?;
        let mut ancestor = supervisor;
        loop {
            if ancestor == worker {
                return Err(CortexError::Validation(
                    "supervisor relationship would form a cycle".into(),
                ));
            }
            match state.parents.get(ancestor) {
                Some(parent) => ancestor = parent,
                None => break,
            }
        }
        if state
            .tasks
            .values()
            .any(|task| task.pending && &task.worker == worker)
        {
            return Err(CortexError::Validation(
                "worker has a pending delegation".into(),
            ));
        }
        state.parents.insert(worker.clone(), supervisor.clone());
        Ok(())
    }

    /// Inspect direct specialist workers in stable identifier order.
    pub fn workers(&self, supervisor: &AgentId) -> Result<Vec<AgentId>> {
        let state = self.coordination.lock().map_err(lock_error)?;
        let mut workers: Vec<_> = state
            .parents
            .iter()
            .filter(|(_, parent)| *parent == supervisor)
            .map(|(worker, _)| worker.clone())
            .collect();
        workers.sort();
        Ok(workers)
    }

    /// Drain redacted message events in enqueue order for the execution event sink.
    ///
    /// Wrap these in `EventRecord` using the run's existing sequence allocator
    /// before writing to `RunStore`. The buffer holds 4096 events; sends reject
    /// further messages until drained, rather than silently losing traces.
    pub fn drain_message_events(&self) -> Result<Vec<ExecutionEvent>> {
        Ok(self
            .coordination
            .lock()
            .map_err(lock_error)?
            .events
            .drain(..)
            .collect())
    }

    /// Release completed task correlations after a run is permanently finished.
    ///
    /// Pending tasks prevent cleanup. Run IDs must not be reused after cleanup.
    pub fn finish_coordination_run(&self, run_id: &RunId) -> Result<()> {
        let mut state = self.coordination.lock().map_err(lock_error)?;
        if state
            .tasks
            .iter()
            .any(|((run, _), task)| run == run_id && task.pending)
        {
            return Err(CortexError::Validation(
                "run still has pending delegations".into(),
            ));
        }
        state.tasks.retain(|(run, _), _| run != run_id);
        Ok(())
    }
}

impl AgentEndpoint {
    /// Identity established by the trusted runtime when this endpoint was issued.
    pub fn agent_id(&self) -> &AgentId {
        &self.agent_id
    }

    /// Enqueue a message without blocking for recipient processing.
    ///
    /// Returns a runtime message ID. Unknown or inactive recipients, unmatched
    /// routes, full queues, stale endpoints, and invalid delegation replies are
    /// errors. Failure leaves queues and task correlations unchanged. Payloads
    /// are limited to 64 KiB of serialized JSON. Tasks are unique within a run;
    /// a result or failure must come from its assigned worker exactly once.
    pub fn send(
        &self,
        recipient: &AgentId,
        run_id: &RunId,
        routing_key: RoutingKey,
        payload: AgentMessagePayload,
    ) -> Result<String> {
        let encoded = serde_json::to_value(&payload)
            .map_err(|e| CortexError::Internal(format!("message serialization failed: {e}")))?;
        if serde_json::to_vec(&encoded)
            .map_err(|e| CortexError::Internal(e.to_string()))?
            .len()
            > MAX_PAYLOAD_BYTES
        {
            return Err(CortexError::Validation(
                "message payload exceeds 64 KiB".into(),
            ));
        }
        let agents = self.manager.agents.read().map_err(lock_error)?;
        require_running(&agents, &self.agent_id)?;
        require_running(&agents, recipient)?;
        let mut state = self.manager.coordination.lock().map_err(lock_error)?;
        self.validate(&state)?;
        let inbox = state
            .inboxes
            .get(recipient)
            .ok_or_else(|| CortexError::NotFound("recipient has no endpoint".into()))?;
        if !inbox.routes.contains(&routing_key) {
            return Err(CortexError::NotFound(
                "recipient does not accept routing key".into(),
            ));
        }
        if inbox.queue.len() == inbox.capacity || state.events.len() == MAX_PENDING_EVENTS {
            return Err(CortexError::Validation(
                "message inbox or trace buffer is full".into(),
            ));
        }
        let task_key = match &payload {
            AgentMessagePayload::TaskRequest { task_id, .. }
            | AgentMessagePayload::TaskResult { task_id, .. }
            | AgentMessagePayload::TaskFailed { task_id, .. } => {
                if task_id.is_empty() || task_id.len() > 128 {
                    return Err(CortexError::Validation(
                        "task ID must contain 1..=128 bytes".into(),
                    ));
                }
                Some((run_id.clone(), task_id.clone()))
            }
            AgentMessagePayload::Notification { .. } => None,
        };
        if let Some(key) = &task_key {
            match &payload {
                AgentMessagePayload::TaskRequest { .. } => {
                    if state.parents.get(recipient) != Some(&self.agent_id) {
                        return Err(CortexError::PermissionDenied(
                            "recipient is not this supervisor's worker".into(),
                        ));
                    }
                    if state.tasks.contains_key(key) || state.tasks.len() == MAX_TASKS {
                        return Err(CortexError::Validation(
                            "task ID already used or correlation limit reached".into(),
                        ));
                    }
                }
                _ => {
                    let task = state
                        .tasks
                        .get(key)
                        .ok_or_else(|| CortexError::NotFound("delegation not found".into()))?;
                    if !task.pending
                        || task.worker != self.agent_id
                        || &task.supervisor != recipient
                    {
                        return Err(CortexError::PermissionDenied(
                            "reply does not match a pending delegation".into(),
                        ));
                    }
                }
            }
        }
        let id = format!("message_{}", state.next_id()?);
        let timestamp = Utc::now().to_rfc3339();
        let event = ExecutionEvent::InterAgentMessage {
            run_id: run_id.clone(),
            message_id: id.clone(),
            sender: self.agent_id.clone(),
            recipient: recipient.clone(),
            routing_key: state.redactor.redact_text(routing_key.as_str()),
            timestamp: timestamp.clone(),
            payload: state.redactor.redact_json(&encoded),
        };
        if let Some(key) = task_key {
            match &payload {
                AgentMessagePayload::TaskRequest { .. } => {
                    state.tasks.insert(
                        key,
                        Delegation {
                            supervisor: self.agent_id.clone(),
                            worker: recipient.clone(),
                            pending: true,
                        },
                    );
                }
                _ => {
                    state
                        .tasks
                        .get_mut(&key)
                        .expect("validated delegation")
                        .pending = false
                }
            }
        }
        let message = AgentMessage {
            id: id.clone(),
            run_id: run_id.clone(),
            sender: self.agent_id.clone(),
            recipient: recipient.clone(),
            routing_key,
            timestamp,
            payload,
        };
        let inbox = state.inboxes.get_mut(recipient).expect("validated inbox");
        inbox.queue.push_back(message);
        let waker = inbox.waker.take();
        state.events.push_back(event);
        drop(state);
        drop(agents);
        if let Some(waker) = waker {
            waker.wake();
        }
        Ok(id)
    }

    fn validate(&self, state: &CoordinationState) -> Result<()> {
        if state.inboxes.get(&self.agent_id).map(|inbox| inbox.epoch) != Some(self.epoch) {
            return Err(CortexError::Cancelled("agent endpoint was revoked".into()));
        }
        Ok(())
    }

    /// Receive the next FIFO message, yielding until delivery or lifecycle change.
    ///
    /// Requires a running agent. No executor dependency is required. The mutable
    /// borrow enforces a single receiver. Cancelling the future leaves queued
    /// messages intact. Stop/restart discards queued messages and wakes the receiver.
    pub async fn recv(&mut self) -> Result<AgentMessage> {
        poll_fn(|cx| {
            let agents = match self.manager.agents.read().map_err(lock_error) {
                Ok(agents) => agents,
                Err(error) => return Poll::Ready(Err(error)),
            };
            if let Err(error) = require_running(&agents, &self.agent_id) {
                return Poll::Ready(Err(error));
            }
            let mut state = match self.manager.coordination.lock().map_err(lock_error) {
                Ok(state) => state,
                Err(error) => return Poll::Ready(Err(error)),
            };
            if let Err(error) = self.validate(&state) {
                return Poll::Ready(Err(error));
            }
            let inbox = state
                .inboxes
                .get_mut(&self.agent_id)
                .expect("validated inbox");
            if let Some(message) = inbox.queue.pop_front() {
                return Poll::Ready(Ok(message));
            }
            inbox.waker = Some(cx.waker().clone());
            Poll::Pending
        })
        .await
    }

    /// Try receiving one message without waiting. Returns `None` for an empty inbox.
    pub fn try_recv(&mut self) -> Result<Option<AgentMessage>> {
        let agents = self.manager.agents.read().map_err(lock_error)?;
        require_running(&agents, &self.agent_id)?;
        let mut state = self.manager.coordination.lock().map_err(lock_error)?;
        self.validate(&state)?;
        Ok(state
            .inboxes
            .get_mut(&self.agent_id)
            .expect("validated inbox")
            .queue
            .pop_front())
    }
}

impl Drop for AgentEndpoint {
    fn drop(&mut self) {
        let waker = match self.manager.coordination.lock() {
            Ok(mut state) => {
                if self.validate(&state).is_ok() {
                    state.revoke(&self.agent_id)
                } else {
                    None
                }
            }
            Err(error) => {
                tracing::error!("coordination lock poisoned on endpoint drop: {error}");
                None
            }
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

fn require_running(agents: &HashMap<AgentId, super::Agent>, id: &AgentId) -> Result<()> {
    let agent = agents
        .get(id)
        .ok_or_else(|| CortexError::NotFound(format!("agent '{id}' not found")))?;
    if agent.state != AgentState::Running {
        return Err(CortexError::Validation(format!(
            "agent '{id}' is not running"
        )));
    }
    Ok(())
}

pub(super) fn lock_error<T: std::fmt::Display>(error: T) -> CortexError {
    CortexError::Internal(format!("coordination lock poisoned: {error}"))
}
