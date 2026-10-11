use crate::compact::ContextCompactionReason;
use crate::compact::ContextCompactionRequest;
use crate::compact::normalize_compaction_instruction;
use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::compact_context_spec::COMPACT_CONTEXT_TOOL_NAME;
use crate::tools::handlers::compact_context_spec::create_compact_context_tool;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;

pub(crate) const COMPACT_CONTEXT_REQUESTED_MESSAGE: &str = "Context compaction request accepted.";

pub(crate) const COMPACT_CONTEXT_ALREADY_REQUESTED_MESSAGE: &str =
    "Context compaction has already been requested for this turn.";

#[derive(Debug, Deserialize)]
struct CompactContextArgs {
    #[serde(default)]
    reason: Option<CompactContextReasonArg>,
    #[serde(default)]
    instruction: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CompactContextReasonArg {
    TaskBoundary,
    ContextDrift,
    LargeUnrelatedHistory,
    BeforeNewPlan,
    ContextPressure,
    SkillRequested,
    Other,
}

impl From<CompactContextReasonArg> for ContextCompactionReason {
    fn from(value: CompactContextReasonArg) -> Self {
        match value {
            CompactContextReasonArg::TaskBoundary => ContextCompactionReason::TaskBoundary,
            CompactContextReasonArg::ContextDrift => ContextCompactionReason::ContextDrift,
            CompactContextReasonArg::LargeUnrelatedHistory => {
                ContextCompactionReason::LargeUnrelatedHistory
            }
            CompactContextReasonArg::BeforeNewPlan => ContextCompactionReason::BeforeNewPlan,
            CompactContextReasonArg::ContextPressure => ContextCompactionReason::ContextPressure,
            CompactContextReasonArg::SkillRequested => ContextCompactionReason::SkillRequested,
            CompactContextReasonArg::Other => ContextCompactionReason::Other,
        }
    }
}

pub struct CompactContextHandler;

impl ToolExecutor<ToolInvocation> for CompactContextHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(COMPACT_CONTEXT_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_compact_context_tool()
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(async move { Self::handle_call(invocation).await })
    }
}

impl CompactContextHandler {
    async fn handle_call(
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;
        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{COMPACT_CONTEXT_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        if !turn.config.compact_context_tool.enabled {
            return Err(FunctionCallError::RespondToModel(format!(
                "{COMPACT_CONTEXT_TOOL_NAME} is disabled; enable [tools.compact_context].enabled to use it"
            )));
        }

        let args: CompactContextArgs = parse_arguments(&arguments)?;
        let instruction = normalize_compaction_instruction(args.instruction);
        let request = ContextCompactionRequest {
            reason: args.reason.map(Into::into),
            instruction,
        };

        let queued = session.request_context_compaction(request).await;
        let message = if queued {
            COMPACT_CONTEXT_REQUESTED_MESSAGE
        } else {
            COMPACT_CONTEXT_ALREADY_REQUESTED_MESSAGE
        };

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            message.to_string(),
            Some(true),
        )))
    }
}

impl CoreToolRuntime for CompactContextHandler {}
