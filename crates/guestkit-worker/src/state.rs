// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Job state machine

use serde::{Deserialize, Serialize};
use crate::error::{WorkerError, WorkerResult};

/// Job execution state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    /// Job received, awaiting validation
    Pending,

    /// Job validated and queued for execution
    Queued,

    /// Job assigned to worker
    Assigned,

    /// Job currently executing
    Running,

    /// Job completed successfully
    Completed,

    /// Job failed with error
    Failed,

    /// Job cancelled by user
    Cancelled,

    /// Job timed out
    Timeout,
}

impl std::fmt::Display for JobState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobState::Pending => write!(f, "pending"),
            JobState::Queued => write!(f, "queued"),
            JobState::Assigned => write!(f, "assigned"),
            JobState::Running => write!(f, "running"),
            JobState::Completed => write!(f, "completed"),
            JobState::Failed => write!(f, "failed"),
            JobState::Cancelled => write!(f, "cancelled"),
            JobState::Timeout => write!(f, "timeout"),
        }
    }
}

impl JobState {
    /// Check if state is terminal (no further transitions)
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            JobState::Completed | JobState::Failed | JobState::Cancelled | JobState::Timeout
        )
    }

    /// Check if state is active (job is being processed)
    pub fn is_active(&self) -> bool {
        matches!(self, JobState::Running)
    }
}

/// Job state machine
#[derive(Debug)]
pub struct JobStateMachine {
    current_state: JobState,
}

impl JobStateMachine {
    /// Create a new state machine
    pub fn new() -> Self {
        Self {
            current_state: JobState::Pending,
        }
    }

    /// Create from existing state
    pub fn from_state(state: JobState) -> Self {
        Self {
            current_state: state,
        }
    }

    /// Get current state
    pub fn current(&self) -> JobState {
        self.current_state
    }

    /// Attempt state transition
    pub fn transition(&mut self, target: JobState) -> WorkerResult<()> {
        if self.is_valid_transition(target) {
            log::debug!(
                "State transition: {} -> {}",
                self.current_state,
                target
            );
            self.current_state = target;
            Ok(())
        } else {
            Err(WorkerError::InvalidStateTransition {
                current: self.current_state.to_string(),
                target: target.to_string(),
            })
        }
    }

    /// Check if transition is valid
    fn is_valid_transition(&self, target: JobState) -> bool {
        use JobState::*;

        // Terminal states cannot transition
        if self.current_state.is_terminal() {
            return false;
        }

        match (self.current_state, target) {
            // From Pending
            (Pending, Queued) => true,
            (Pending, Failed) => true,

            // From Queued
            (Queued, Assigned) => true,
            (Queued, Cancelled) => true,
            (Queued, Failed) => true,

            // From Assigned
            (Assigned, Running) => true,
            (Assigned, Cancelled) => true,
            (Assigned, Failed) => true,

            // From Running
            (Running, Completed) => true,
            (Running, Failed) => true,
            (Running, Cancelled) => true,
            (Running, Timeout) => true,

            // Invalid transitions
            _ => false,
        }
    }
}

impl Default for JobStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        let mut sm = JobStateMachine::new();

        assert_eq!(sm.current(), JobState::Pending);
        assert!(sm.transition(JobState::Queued).is_ok());
        assert_eq!(sm.current(), JobState::Queued);
        assert!(sm.transition(JobState::Assigned).is_ok());
        assert!(sm.transition(JobState::Running).is_ok());
        assert!(sm.transition(JobState::Completed).is_ok());
    }

    #[test]
    fn test_invalid_transition() {
        let mut sm = JobStateMachine::new();

        // Cannot jump from Pending to Running
        assert!(sm.transition(JobState::Running).is_err());
    }

    #[test]
    fn test_terminal_state() {
        let mut sm = JobStateMachine::from_state(JobState::Completed);

        // Cannot transition from terminal state
        assert!(sm.transition(JobState::Running).is_err());
    }

    #[test]
    fn test_cancellation() {
        let mut sm = JobStateMachine::new();

        sm.transition(JobState::Queued).unwrap();
        sm.transition(JobState::Assigned).unwrap();

        // Can cancel from Assigned
        assert!(sm.transition(JobState::Cancelled).is_ok());
    }
}
