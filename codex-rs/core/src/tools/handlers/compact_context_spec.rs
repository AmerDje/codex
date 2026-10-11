use codex_tools::JsonSchema;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) const COMPACT_CONTEXT_TOOL_NAME: &str = "compact_context";

const COMPACT_CONTEXT_REASON_DESCRIPTION: &str =
    "Optional reason for requesting compaction. Used for logging and debugging.";

const COMPACT_CONTEXT_INSTRUCTION_DESCRIPTION: &str = "Optional guidance for the compaction summary. Describe what existing context to preserve, omit, or de-emphasize; do not introduce a new task.";

pub fn create_compact_context_tool() -> ToolSpec {
    let properties = BTreeMap::from([
        (
            "reason".to_string(),
            JsonSchema::string_enum(
                vec![
                    json!("task_boundary"),
                    json!("context_drift"),
                    json!("large_unrelated_history"),
                    json!("before_new_plan"),
                    json!("context_pressure"),
                    json!("skill_requested"),
                    json!("other"),
                ],
                Some(COMPACT_CONTEXT_REASON_DESCRIPTION.to_string()),
            ),
        ),
        (
            "instruction".to_string(),
            JsonSchema::string(Some(COMPACT_CONTEXT_INSTRUCTION_DESCRIPTION.to_string()))
                .with_encrypted(),
        ),
    ]);

    ToolSpec::Function(ResponsesApiTool {
        name: COMPACT_CONTEXT_TOOL_NAME.to_string(),
        description: compact_context_tool_description(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(properties, /*required*/ None, Some(false.into())),
        output_schema: None,
    })
}

fn compact_context_tool_description() -> String {
    r#"Request context compaction before continuing.
Compaction creates a concise handoff summary of the current conversation and replaces older conversation detail with that summary.

Use this tool only when a cleaner context is likely to improve the next step:
- after a clear task boundary or major topic change
- before starting a new plan
- when old discussion is no longer relevant
- when active task instructions request a clean context transition

When providing `instruction`, describe how the summary should treat existing context: what to preserve, omit, or de-emphasize. Do not use `instruction` to introduce a new task.

Follow active user, project, goal, and skill instructions for workflow-specific compaction behavior. Never use compaction to remove or weaken system, developer, safety, permission, current user constraints, unresolved requirements, or relevant mistakes. Do not call repeatedly without a clear context-quality reason."#
        .to_string()
}
