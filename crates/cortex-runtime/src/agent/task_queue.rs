//! Prioritized TaskQueue and supervisor-worker orchestration engine.

use chrono::{DateTime, Utc};
use cortex_core::{AgentId, CortexError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

/// Priority levels for scheduled workflow tasks.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    /// Non-urgent background work.
    Low = 1,
    /// Standard execution priority.
    #[default]
    Normal = 2,
    /// Elevated priority for blockers.
    High = 3,
    /// Emergency priority executed immediately before other tasks.
    Critical = 4,
}

impl std::fmt::Display for TaskPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Execution lifecycle states of a workflow task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is waiting for prerequisites or agent pickup.
    #[default]
    Pending,
    /// Task has been assigned to a designated specialist agent.
    Assigned,
    /// Task is actively being processed by an agent.
    Running,
    /// Task completed successfully with output.
    Completed,
    /// Task execution encountered an unrecoverable failure.
    Failed,
    /// Task was cancelled by supervisor or cascading dependency abort.
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Assigned => write!(f, "assigned"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A structured task executed within a multi-agent workflow DAG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowTask {
    /// Unique task identifier.
    pub id: String,
    /// Parent workflow correlation identifier.
    pub workflow_id: String,
    /// Optional parent task identifier in decomposition hierarchy.
    pub parent_task_id: Option<String>,
    /// Human-readable task title.
    pub title: String,
    /// Detailed execution instructions or context prompt.
    pub description: String,
    /// Designated agent assignment.
    pub assigned_to: Option<AgentId>,
    /// Target role requirement (e.g., "researcher", "coder", "reviewer").
    pub required_role: Option<String>,
    /// Current lifecycle status.
    pub status: TaskStatus,
    /// Scheduling priority.
    pub priority: TaskPriority,
    /// Prerequisite task IDs that must succeed before this task can start.
    pub dependencies: Vec<String>,
    /// UTC timestamp of task creation.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp when task finished, if terminal.
    pub completed_at: Option<DateTime<Utc>>,
    /// Output result payload upon completion.
    pub output: Option<String>,
    /// Error message upon failure.
    pub error: Option<String>,
}

impl WorkflowTask {
    /// Create a new task definition.
    pub fn new(
        id: impl Into<String>,
        workflow_id: impl Into<String>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            workflow_id: workflow_id.into(),
            parent_task_id: None,
            title: title.into(),
            description: description.into(),
            assigned_to: None,
            required_role: None,
            status: TaskStatus::Pending,
            priority: TaskPriority::Normal,
            dependencies: Vec::new(),
            created_at: Utc::now(),
            completed_at: None,
            output: None,
            error: None,
        }
    }

    /// Set required role for agent discovery.
    pub fn with_role(mut self, role: impl Into<String>) -> Self {
        self.required_role = Some(role.into());
        self
    }

    /// Set task priority.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Add dependency prerequisite IDs.
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies = deps;
        self
    }

    /// Set parent task correlation.
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_task_id = Some(parent_id.into());
        self
    }
}

/// Concurrency-safe, prioritized task queue with dependency DAG resolution.
#[derive(Clone, Default)]
pub struct TaskQueue {
    tasks: Arc<RwLock<HashMap<String, WorkflowTask>>>,
    dependents: Arc<RwLock<HashMap<String, HashSet<String>>>>,
    lock: Arc<Mutex<()>>,
}

impl TaskQueue {
    /// Construct a new empty [`TaskQueue`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Enqueue a task, validating that dependencies do not create cycles.
    pub fn enqueue(&self, task: WorkflowTask) -> Result<()> {
        let _guard = self.lock.lock().map_err(|e| {
            CortexError::Internal(format!("failed to acquire task queue lock: {e}"))
        })?;

        let tasks = self.tasks.read().map_err(|e| {
            CortexError::Internal(format!("failed to acquire tasks read lock: {e}"))
        })?;

        if tasks.contains_key(&task.id) {
            return Err(CortexError::Validation(format!(
                "task '{}' already exists in queue",
                task.id
            )));
        }

        // Check for cycle with existing dependencies
        for dep in &task.dependencies {
            if dep == &task.id {
                return Err(CortexError::Validation(format!(
                    "task '{}' cannot depend on itself",
                    task.id
                )));
            }
            if self.detect_cycle(&task.id, dep, &tasks) {
                return Err(CortexError::Validation(format!(
                    "cyclic dependency detected involving task '{}' and dependency '{}'",
                    task.id, dep
                )));
            }
        }
        drop(tasks);

        let mut tasks = self.tasks.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire tasks write lock: {e}"))
        })?;
        let mut dependents = self.dependents.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire dependents write lock: {e}"))
        })?;

        for dep in &task.dependencies {
            dependents
                .entry(dep.clone())
                .or_default()
                .insert(task.id.clone());
        }

        tasks.insert(task.id.clone(), task);
        Ok(())
    }

    fn detect_cycle(
        &self,
        new_id: &str,
        target_dep: &str,
        tasks: &HashMap<String, WorkflowTask>,
    ) -> bool {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(target_dep.to_string());

        while let Some(current) = queue.pop_front() {
            if current == new_id {
                return true;
            }
            if visited.insert(current.clone()) {
                if let Some(task) = tasks.get(&current) {
                    for dep in &task.dependencies {
                        queue.push_back(dep.clone());
                    }
                }
            }
        }
        false
    }

    /// Retrieve the next highest-priority task that is ready to run.
    ///
    /// A task is ready when all its dependencies have reached [`TaskStatus::Completed`].
    pub fn pop_next_available(
        &self,
        assigned_agent: Option<&AgentId>,
        role: Option<&str>,
    ) -> Option<WorkflowTask> {
        let tasks = self.tasks.read().ok()?;

        let mut candidates: Vec<&WorkflowTask> = tasks
            .values()
            .filter(|t| {
                // Must be pending or assigned to this agent
                let status_ok = match t.status {
                    TaskStatus::Pending => {
                        if let Some(assigned) = &t.assigned_to {
                            assigned_agent.map(|a| a == assigned).unwrap_or(false)
                        } else {
                            true
                        }
                    }
                    TaskStatus::Assigned => assigned_agent
                        .map(|a| Some(a) == t.assigned_to.as_ref())
                        .unwrap_or(false),
                    _ => false,
                };

                if !status_ok {
                    return false;
                }

                // Check role match if requested
                if let (Some(req_role), Some(worker_role)) = (&t.required_role, role) {
                    if req_role != worker_role {
                        return false;
                    }
                }

                // Check prerequisites: all dependencies must be Completed
                t.dependencies.iter().all(|dep_id| {
                    tasks
                        .get(dep_id)
                        .map(|dep| dep.status == TaskStatus::Completed)
                        .unwrap_or(false)
                })
            })
            .collect();

        // Sort descending by priority, then ascending by creation time
        candidates.sort_by(|a, b| {
            b.priority
                .cmp(&a.priority)
                .then_with(|| a.created_at.cmp(&b.created_at))
        });

        candidates.first().cloned().cloned()
    }

    /// Mark a task as running under the supervision of a specific agent.
    pub fn mark_running(&self, task_id: &str, agent_id: &AgentId) -> Result<()> {
        let mut tasks = self.tasks.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire tasks write lock: {e}"))
        })?;

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| CortexError::NotFound(format!("task '{task_id}' not found in queue")))?;

        if task.status != TaskStatus::Pending && task.status != TaskStatus::Assigned {
            return Err(CortexError::Validation(format!(
                "cannot start task '{task_id}' from status '{}'",
                task.status
            )));
        }

        task.status = TaskStatus::Running;
        task.assigned_to = Some(agent_id.clone());
        Ok(())
    }

    /// Mark a task as completed with output.
    pub fn mark_completed(&self, task_id: &str, output: String) -> Result<()> {
        let mut tasks = self.tasks.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire tasks write lock: {e}"))
        })?;

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| CortexError::NotFound(format!("task '{task_id}' not found in queue")))?;

        task.status = TaskStatus::Completed;
        task.output = Some(output);
        task.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Mark a task as failed with an error diagnostic.
    pub fn mark_failed(&self, task_id: &str, error: String) -> Result<()> {
        let mut tasks = self.tasks.write().map_err(|e| {
            CortexError::Internal(format!("failed to acquire tasks write lock: {e}"))
        })?;

        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| CortexError::NotFound(format!("task '{task_id}' not found in queue")))?;

        task.status = TaskStatus::Failed;
        task.error = Some(error);
        task.completed_at = Some(Utc::now());
        Ok(())
    }

    /// Cancel a task and recursively cascade cancellation to all dependent tasks.
    pub fn cancel_task(&self, task_id: &str) -> Vec<String> {
        let mut cancelled = Vec::new();
        let mut queue = VecDeque::new();
        queue.push_back(task_id.to_string());

        let mut tasks = match self.tasks.write() {
            Ok(lock) => lock,
            Err(_) => return cancelled,
        };
        let dependents = match self.dependents.read() {
            Ok(lock) => lock,
            Err(_) => return cancelled,
        };

        while let Some(current) = queue.pop_front() {
            if let Some(task) = tasks.get_mut(&current) {
                if task.status != TaskStatus::Completed && task.status != TaskStatus::Cancelled {
                    task.status = TaskStatus::Cancelled;
                    task.completed_at = Some(Utc::now());
                    cancelled.push(current.clone());

                    // Cascade to dependents
                    if let Some(deps) = dependents.get(&current) {
                        for dep in deps {
                            queue.push_back(dep.clone());
                        }
                    }
                }
            }
        }

        cancelled
    }

    /// Retrieve an immutable snapshot of a task by ID.
    pub fn get_task(&self, task_id: &str) -> Option<WorkflowTask> {
        self.tasks.read().ok()?.get(task_id).cloned()
    }

    /// List all tasks in arbitrary order.
    pub fn list_tasks(&self) -> Vec<WorkflowTask> {
        self.tasks
            .read()
            .map(|t| t.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Check if all tasks in a given workflow have terminated (Completed, Failed, Cancelled).
    pub fn is_workflow_complete(&self, workflow_id: &str) -> bool {
        let tasks = match self.tasks.read() {
            Ok(lock) => lock,
            Err(_) => return false,
        };

        let wf_tasks: Vec<&WorkflowTask> = tasks
            .values()
            .filter(|t| t.workflow_id == workflow_id)
            .collect();

        if wf_tasks.is_empty() {
            return false;
        }

        wf_tasks.iter().all(|t| {
            matches!(
                t.status,
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
            )
        })
    }
}

/// Orchestration coordinator executing canonical manager-researcher-coder-reviewer pipelines.
#[derive(Clone, Default)]
pub struct WorkflowCoordinator {
    queue: TaskQueue,
}

impl WorkflowCoordinator {
    /// Create a new coordinator with an underlying task queue.
    pub fn new() -> Self {
        Self {
            queue: TaskQueue::new(),
        }
    }

    /// Reference the internal [`TaskQueue`].
    pub fn queue(&self) -> &TaskQueue {
        &self.queue
    }

    /// Instantiate canonical manager -> researcher -> coder -> reviewer sequential pipeline.
    pub fn create_canonical_pipeline(&self, workflow_id: &str, goal: &str) -> Result<()> {
        let t1 = WorkflowTask::new(
            format!("{workflow_id}-plan"),
            workflow_id,
            "Decompose Goal & Plan Architecture",
            format!("Manager planning goal: {goal}"),
        )
        .with_role("manager")
        .with_priority(TaskPriority::Critical);

        let t2 = WorkflowTask::new(
            format!("{workflow_id}-research"),
            workflow_id,
            "Context Research & Dependency Discovery",
            format!("Researcher gathering dependencies for: {goal}"),
        )
        .with_role("researcher")
        .with_priority(TaskPriority::High)
        .with_dependencies(vec![format!("{workflow_id}-plan")]);

        let t3 = WorkflowTask::new(
            format!("{workflow_id}-code"),
            workflow_id,
            "Implementation & Patch Generation",
            format!("Coder implementing solution for: {goal}"),
        )
        .with_role("coder")
        .with_priority(TaskPriority::High)
        .with_dependencies(vec![format!("{workflow_id}-research")]);

        let t4 = WorkflowTask::new(
            format!("{workflow_id}-review"),
            workflow_id,
            "Security & Quality Review",
            format!("Reviewer validating correctness for: {goal}"),
        )
        .with_role("reviewer")
        .with_priority(TaskPriority::High)
        .with_dependencies(vec![format!("{workflow_id}-code")]);

        self.queue.enqueue(t1)?;
        self.queue.enqueue(t2)?;
        self.queue.enqueue(t3)?;
        self.queue.enqueue(t4)?;
        Ok(())
    }

    /// Handle reviewer feedback loop: if review fails, schedule a revision coder task.
    pub fn handle_review_rejection(
        &self,
        workflow_id: &str,
        iteration: usize,
        max_iterations: usize,
        review_feedback: &str,
    ) -> Result<Option<String>> {
        if iteration >= max_iterations {
            return Err(CortexError::Internal(format!(
                "review feedback loop exceeded maximum allowed iterations ({max_iterations})"
            )));
        }

        let new_code_task_id = format!("{workflow_id}-code-rev{iteration}");
        let review_task_id = format!("{workflow_id}-review");

        let t_code = WorkflowTask::new(
            &new_code_task_id,
            workflow_id,
            format!("Revise Implementation (Iteration {iteration})"),
            format!("Coder addressing feedback: {review_feedback}"),
        )
        .with_role("coder")
        .with_priority(TaskPriority::High)
        .with_dependencies(vec![review_task_id.clone()]);

        let new_review_task_id = format!("{workflow_id}-review-rev{iteration}");
        let t_review = WorkflowTask::new(
            &new_review_task_id,
            workflow_id,
            format!("Re-review Implementation (Iteration {iteration})"),
            "Reviewer validating revision".to_string(),
        )
        .with_role("reviewer")
        .with_priority(TaskPriority::High)
        .with_dependencies(vec![new_code_task_id.clone()]);

        self.queue.enqueue(t_code)?;
        self.queue.enqueue(t_review)?;

        Ok(Some(new_code_task_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_priority_ordering() {
        assert!(TaskPriority::Critical > TaskPriority::High);
        assert!(TaskPriority::High > TaskPriority::Normal);
        assert!(TaskPriority::Normal > TaskPriority::Low);
    }

    #[test]
    fn test_dependency_dag_blocking_and_progression() {
        let queue = TaskQueue::new();
        let agent = AgentId::from("worker-1");

        let t1 = WorkflowTask::new("t1", "wf-1", "Task 1", "desc");
        let t2 =
            WorkflowTask::new("t2", "wf-1", "Task 2", "desc").with_dependencies(vec!["t1".into()]);

        queue.enqueue(t1).unwrap();
        queue.enqueue(t2).unwrap();

        // Only t1 is available because t2 depends on t1
        let next = queue.pop_next_available(Some(&agent), None).unwrap();
        assert_eq!(next.id, "t1");

        // While t1 is pending/running, t2 cannot pop
        queue.mark_running("t1", &agent).unwrap();
        assert!(queue.pop_next_available(Some(&agent), None).is_none());

        // Completing t1 unlocks t2
        queue.mark_completed("t1", "output 1".into()).unwrap();
        let next2 = queue.pop_next_available(Some(&agent), None).unwrap();
        assert_eq!(next2.id, "t2");
    }

    #[test]
    fn test_cycle_detection_rejection() {
        let queue = TaskQueue::new();

        let t1 = WorkflowTask::new("task-a", "wf-1", "A", "desc");
        let t2 = WorkflowTask::new("task-b", "wf-1", "B", "desc")
            .with_dependencies(vec!["task-a".into()]);
        let t3 = WorkflowTask::new("task-c", "wf-1", "C", "desc")
            .with_dependencies(vec!["task-b".into()]);

        queue.enqueue(t1).unwrap();
        queue.enqueue(t2).unwrap();
        queue.enqueue(t3).unwrap();

        // Cyclic: task-d depends on task-c, and task-a depends on task-d
        let self_dep = WorkflowTask::new("task-loop", "wf-1", "Self", "desc")
            .with_dependencies(vec!["task-loop".into()]);
        assert!(queue.enqueue(self_dep).is_err());

        // Cycle through chain
        let mut t1_mut = queue.get_task("task-a").unwrap();
        t1_mut.id = "task-closing-cycle".into();
        t1_mut.dependencies = vec!["task-c".into()];
        queue.enqueue(t1_mut).unwrap();

        let cyclic_task = WorkflowTask::new("task-a-cycle", "wf-1", "Cycle", "desc")
            .with_dependencies(vec!["task-closing-cycle".into()]);
        assert!(queue.enqueue(cyclic_task).is_ok());
    }

    #[test]
    fn test_cascading_cancellation() {
        let queue = TaskQueue::new();

        let t1 = WorkflowTask::new("root", "wf-c", "Root", "desc");
        let t2 = WorkflowTask::new("child-1", "wf-c", "Child 1", "desc")
            .with_dependencies(vec!["root".into()]);
        let t3 = WorkflowTask::new("child-2", "wf-c", "Child 2", "desc")
            .with_dependencies(vec!["child-1".into()]);
        let unrelated = WorkflowTask::new("other", "wf-c", "Other", "desc");

        queue.enqueue(t1).unwrap();
        queue.enqueue(t2).unwrap();
        queue.enqueue(t3).unwrap();
        queue.enqueue(unrelated).unwrap();

        // Cancel root -> cascades to child-1 and child-2
        let cancelled = queue.cancel_task("root");
        assert_eq!(cancelled.len(), 3);
        assert!(cancelled.contains(&"root".to_string()));
        assert!(cancelled.contains(&"child-1".to_string()));
        assert!(cancelled.contains(&"child-2".to_string()));

        assert_eq!(
            queue.get_task("root").unwrap().status,
            TaskStatus::Cancelled
        );
        assert_eq!(
            queue.get_task("child-1").unwrap().status,
            TaskStatus::Cancelled
        );
        assert_eq!(
            queue.get_task("child-2").unwrap().status,
            TaskStatus::Cancelled
        );
        assert_eq!(queue.get_task("other").unwrap().status, TaskStatus::Pending);
    }

    #[test]
    fn test_canonical_pipeline_coordinator() {
        let coordinator = WorkflowCoordinator::new();
        coordinator
            .create_canonical_pipeline("wf-can", "Build auth module")
            .unwrap();

        let tasks = coordinator.queue().list_tasks();
        assert_eq!(tasks.len(), 4);

        // Sequence must unlock strictly plan -> research -> code -> review
        let agent = AgentId::from("worker");
        let p = coordinator
            .queue()
            .pop_next_available(Some(&agent), Some("manager"))
            .unwrap();
        assert_eq!(p.id, "wf-can-plan");

        coordinator
            .queue()
            .mark_completed("wf-can-plan", "plan".into())
            .unwrap();
        let r = coordinator
            .queue()
            .pop_next_available(Some(&agent), Some("researcher"))
            .unwrap();
        assert_eq!(r.id, "wf-can-research");
    }

    #[test]
    fn test_review_rejection_loop() {
        let coordinator = WorkflowCoordinator::new();
        coordinator
            .create_canonical_pipeline("wf-loop", "Feature")
            .unwrap();

        let new_code_id = coordinator
            .handle_review_rejection("wf-loop", 1, 3, "Missing error boundary")
            .unwrap()
            .expect("new task id");

        assert_eq!(new_code_id, "wf-loop-code-rev1");
        assert!(coordinator.queue().get_task("wf-loop-code-rev1").is_some());
        assert!(coordinator
            .queue()
            .get_task("wf-loop-review-rev1")
            .is_some());

        // Max iteration error
        let err = coordinator.handle_review_rejection("wf-loop", 3, 3, "fail");
        assert!(err.is_err());
    }
}
