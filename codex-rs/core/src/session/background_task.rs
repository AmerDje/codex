//! Opt-in resumption of an idle thread when a backgrounded terminal process finishes.
//!
//! Unified exec keeps long-running processes alive across turns. While a turn is
//! active the model observes terminal output through `write_stdin`, but once the
//! turn ends nothing wakes the model when the process later exits. With
//! [`Feature::BackgroundTaskResume`] enabled, the exit watcher calls
//! [`Session::notify_background_process_exited`], which starts an internal
//! continuation turn so the model can act on the finished command.

use std::sync::Arc;

use super::TurnInput;
use super::session::Session;
use crate::context::ContextualUserFragment;
use crate::context::InternalContextSource;
use crate::context::InternalModelContextFragment;
use crate::tasks::RegularTask;
use codex_features::Feature;
use codex_protocol::config_types::ModeKind;

const BACKGROUND_TASK_TURN_TRIGGER: &str = "background_task";
const MAX_NOTIFICATION_OUTPUT_CHARS: usize = 4_000;

/// Completion details for a unified-exec process that finished in the background.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BackgroundProcessExit {
    pub(crate) process_id: i32,
    pub(crate) command: Vec<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) failure: Option<String>,
    pub(crate) output: String,
}

impl Session {
    /// Returns whether a turn is currently active on this thread.
    pub(crate) async fn is_active_turn(&self) -> bool {
        self.active_turn.lock().await.is_some()
    }

    /// Wakes an idle thread when a backgrounded terminal process finishes.
    ///
    /// Opt-in via [`Feature::BackgroundTaskResume`]. While a turn is active the
    /// model already observes terminal output through `write_stdin`, so this only
    /// starts an internal continuation turn for the idle case.
    pub(crate) async fn notify_background_process_exited(
        self: &Arc<Self>,
        exit: BackgroundProcessExit,
    ) {
        if !self.enabled(Feature::BackgroundTaskResume) {
            return;
        }
        // A turn is already in control; do not inject a competing notification.
        if self.is_active_turn().await {
            return;
        }
        // Automatic continuations may not run from Plan mode.
        if matches!(self.collaboration_mode().await.mode, ModeKind::Plan) {
            return;
        }

        let item = ContextualUserFragment::into(InternalModelContextFragment::new(
            InternalContextSource::from_static("background_task"),
            format_background_exit(&exit),
        ));
        let turn_context = self.new_default_turn().await;
        turn_context
            .turn_metadata_state
            .set_turn_trigger(BACKGROUND_TASK_TURN_TRIGGER.to_string());
        self.spawn_task(
            turn_context,
            vec![TurnInput::ResponseItem(item.into())],
            RegularTask::new(),
        )
        .await;
    }
}

fn format_background_exit(exit: &BackgroundProcessExit) -> String {
    let status = if exit.timed_out {
        "timed out".to_string()
    } else if let Some(failure) = exit.failure.as_deref() {
        format!("failed: {failure}")
    } else if let Some(code) = exit.exit_code {
        format!("exited with code {code}")
    } else {
        "exited".to_string()
    };
    let command = exit.command.join(" ");
    let output = truncate_tail(&exit.output, MAX_NOTIFICATION_OUTPUT_CHARS);
    let process_id = exit.process_id;
    format!(
        "Background terminal process {process_id} finished.\n\
command: {command}\n\
status: {status}\n\
output:\n{output}\n\n\
Review the output and continue the task if more work remains. The process is no longer running."
    )
}

fn truncate_tail(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut tail = text.chars().rev().take(max_chars).collect::<Vec<_>>();
    tail.reverse();
    let tail = tail.into_iter().collect::<String>();
    format!("... (earlier output truncated) ...\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_tail_keeps_the_end() {
        assert_eq!(truncate_tail("abc", 3), "abc");
        assert_eq!(
            truncate_tail("abcdef", 3),
            "... (earlier output truncated) ...\ndef"
        );
    }

    #[test]
    fn format_reports_exit_status_and_output() {
        let body = format_background_exit(&BackgroundProcessExit {
            process_id: 7,
            command: vec!["echo".to_string(), "hi".to_string()],
            exit_code: Some(0),
            timed_out: false,
            failure: None,
            output: "hi\n".to_string(),
        });
        assert!(body.contains("process 7 finished"));
        assert!(body.contains("command: echo hi"));
        assert!(body.contains("exited with code 0"));
        assert!(body.ends_with("no longer running."));
    }
}
