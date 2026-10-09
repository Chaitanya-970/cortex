# Multi-Agent Coordination Evaluations

This evaluation suite verifies deterministic, in-process inter-agent coordination, bounded inbox delivery, hierarchy constraints, and lifecycle revocation in `cortex-runtime`.

> **Note**: These evaluations test runtime coordination and invariant enforcement; they are not LLM model quality measurements.

---

## Running the Evaluation Suite

Execute the deterministic integration suite directly with `cargo test`:

```bash
cargo test -p cortex-runtime --test agent_coordination_test
```

All evaluations run in-process against in-memory state machines without external network dependencies, mock servers, or background daemons.

---

## Evaluation Scenarios & Pass Criteria

The integration suite maps directly to the following runtime coordination scenarios:

### 1. Bounded FIFO Routing & Sender Authentication
- **Test**: `bounded_fifo_routing_and_sender_identity`
- **Scenario**: Two agents exchange notifications across registered routing keys.
- **Pass Criteria**:
  - Sender identity is bound to the originating `AgentEndpoint` and cannot be forged.
  - Sends to unknown agents or unregistered routing keys return `CortexError::NotFound`.
  - Accepted messages arrive in strict enqueue order.
  - Saturated inboxes reject additional messages with `CortexError::Validation`.

### 2. Asynchronous Receive Wakeups & Cancellation Safety
- **Test**: `asynchronous_receive_is_woken_by_delivery_and_preserves_cancelled_future_messages`
- **Scenario**: Receiver awaits messages on an empty inbox; subsequent receive futures are dropped before delivery.
- **Pass Criteria**:
  - Asynchronous `recv()` registers a standard `Waker` and yields until a message is enqueued.
  - Dropping or cancelling a `recv()` future leaves queued messages intact for subsequent calls.

### 3. Lifecycle Invalidation & Stale Endpoint Revocation
- **Test**: `lifecycle_changes_wake_receivers_and_revoke_old_endpoints`
- **Scenario**: An agent transitions through `stop`, `fail`, `restart`, `prepare`, and `pause` states while receivers are waiting.
- **Pass Criteria**:
  - Stop, failure, restart, and prepare revoke endpoints and wake pending receivers; pause wakes receivers and preserves inboxes.
  - Stale endpoints cannot send or receive after their agent restarts.
  - Replacement endpoints reconnect cleanly without interference from dropped or stale handles.

### 4. Hierarchy Constraints & Single-Completion Task Correlation
- **Test**: `hierarchy_and_task_correlation_enforce_ownership_and_single_completion`
- **Scenario**: Supervisor/worker task delegation with unauthorized third-party interference.
- **Pass Criteria**:
  - Self-assignment and supervisor cycle formation are strictly rejected.
  - Third-party agents cannot delegate to workers or report results on behalf of workers.
  - Results and failures must match an active delegation, worker, supervisor, and `RunId` exactly once.
  - Re-reporting a completed task or completing an unknown task is rejected.
  - Runs with pending delegations cannot be finalized via `finish_coordination_run`.

### 5. Backpressure Preservation Under Inbox Saturation
- **Test**: `full_inbox_does_not_consume_a_delegation_or_reply`
- **Scenario**: Delegations and task results sent to saturated recipient inboxes.
- **Pass Criteria**:
  - Rejection due to inbox capacity preserves task correlation state without consuming or corrupting the delegation.
  - Retrying the send after draining the inbox succeeds cleanly.

### 6. Worker Teardown & Topology Removal
- **Test**: `stop_cancels_tasks_discards_queue_and_remove_cleans_topology`
- **Scenario**: Stopping an active worker with pending work, followed by removal from the manager.
- **Pass Criteria**:
  - Stopping an agent marks pending delegations as inactive and flushes queued inbox messages.
  - Removing an agent cleanly severs supervisor hierarchy links.

### 7. Execution Event Tracing & Secret Redaction
- **Test**: `redacted_message_events_round_trip_through_existing_run_store`
- **Scenario**: An inter-agent message containing sensitive credentials (`sk-...`) is enqueued and persisted.
- **Pass Criteria**:
  - Enqueueing records an `ExecutionEvent::InterAgentMessage` into the manager's event queue.
  - Drained events sanitize credentials via `Redactor` while the recipient inbox receives the raw payload.
  - Redacted events persist and round-trip cleanly through SQLite `RunStore`.

### 8. Concurrent Race Serialization
- **Test**: `concurrent_send_and_stop_are_serialized_without_stale_delivery`
- **Scenario**: Threads race concurrently between sending messages and stopping destination agents.
- **Pass Criteria**:
  - Thread-safe serialization without panics or lock poisoning.
  - Messages are either accepted and recorded or cleanly rejected with no stale post-stop delivery.

### 9. Trace Buffer Backpressure & Payload Limits
- **Test**: `trace_backpressure_and_payload_limits_preserve_delivery`
- **Scenario**: Enqueueing oversized payloads (>64 KiB) and exhausting the 4096-event trace buffer.
- **Pass Criteria**:
  - Payloads exceeding 64 KiB fail validation before entering queues.
  - Saturated trace buffers reject sends with `CortexError::Validation` until the host drains them, preventing unrecorded message delivery.

### 10. Multi-Tier Hierarchies & Endpoint Drop Cleanup
- **Test**: `nested_supervisors_delegate_independently_and_endpoint_drop_cancels_tasks`
- **Scenario**: Three-tier hierarchy (`root` -> `middle` -> `leaf`) where the intermediate coordinator drops its endpoint.
- **Pass Criteria**:
  - Nested delegations correlate independently.
  - Dropping an intermediate endpoint closes its inbox and revokes associated pending tasks.
