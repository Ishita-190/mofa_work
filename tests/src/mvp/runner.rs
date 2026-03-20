use std::collections::HashMap;
use std::time::Instant;

use serde_json::Value;

use crate::mvp::assertions::{Assertion, TestContext, ToolCall};
use crate::mvp::llm_mock::{ChatCompletionRequest, ChatCompletionResponse, ChatMessage};
use crate::mvp::report::{
    report_now_unix_ms, TestCaseReport, TestRunReport, TestStatus, TestSummary,
};

pub trait LlmBackend: Send + Sync {
    fn complete(&self, request: &ChatCompletionRequest) -> ChatCompletionResponse;
}

#[derive(Debug, Clone)]
pub struct AgentRunResult {
    pub response_text: String,
    pub tool_calls: Vec<ToolCall>,
    pub memory: HashMap<String, Value>,
}

pub trait Agent: Send + Sync {
    fn run(&self, input: &str, llm: &dyn LlmBackend, test_id: &str) -> AgentRunResult;
}

#[derive(Debug, Clone)]
pub struct AgentTestCase {
    pub name: String,
    pub input: String,
    pub assertions: Vec<Assertion>,
}

#[derive(Debug, Clone)]
pub struct TestRunner {
    pub suite_name: String,
}

impl TestRunner {
    pub fn new(suite_name: impl Into<String>) -> Self {
        Self {
            suite_name: suite_name.into(),
        }
    }

    pub fn run_suite<A: Agent>(
        &self,
        agent: &A,
        llm: &dyn LlmBackend,
        cases: &[AgentTestCase],
    ) -> TestRunReport {
        let started_at = report_now_unix_ms();
        let mut case_reports = Vec::with_capacity(cases.len());

        for case in cases {
            let start = Instant::now();
            let result = agent.run(&case.input, llm, &case.name);
            let context = TestContext {
                response_text: result.response_text,
                tool_calls: result.tool_calls,
                memory: result.memory,
            };

            let assertions = case
                .assertions
                .iter()
                .map(|assertion| assertion.evaluate(&context))
                .collect::<Vec<_>>();

            let passed = assertions.iter().all(|result| result.passed);
            let duration_ms = start.elapsed().as_millis();
            let error = if passed {
                None
            } else {
                let failed = assertions
                    .iter()
                    .filter(|r| !r.passed)
                    .map(|r| r.name.clone())
                    .collect::<Vec<_>>();
                Some(format!("Failed assertions: {}", failed.join(", ")))
            };

            case_reports.push(TestCaseReport {
                name: case.name.clone(),
                status: if passed {
                    TestStatus::Passed
                } else {
                    TestStatus::Failed
                },
                duration_ms,
                assertions,
                error,
            });
        }

        let passed = case_reports
            .iter()
            .filter(|case_report| case_report.status == TestStatus::Passed)
            .count();
        let failed = case_reports.len().saturating_sub(passed);

        TestRunReport {
            suite_name: self.suite_name.clone(),
            run_id: uuid::Uuid::now_v7().to_string(),
            started_at_unix_ms: started_at,
            finished_at_unix_ms: report_now_unix_ms(),
            summary: TestSummary {
                total: case_reports.len(),
                passed,
                failed,
            },
            cases: case_reports,
        }
    }
}

pub fn build_request(input: &str, test_id: &str, turn: u32) -> ChatCompletionRequest {
    ChatCompletionRequest {
        model: "mock-model".to_string(),
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: input.to_string(),
        }],
        test_id: Some(test_id.to_string()),
        turn: Some(turn),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvp::llm_mock::ChatChoice;

    struct EchoAgent;

    impl Agent for EchoAgent {
        fn run(&self, input: &str, llm: &dyn LlmBackend, test_id: &str) -> AgentRunResult {
            let response = llm.complete(&build_request(input, test_id, 0));
            AgentRunResult {
                response_text: response.choices[0].message.content.clone(),
                tool_calls: Vec::new(),
                memory: HashMap::new(),
            }
        }
    }

    struct InlineBackend;

    impl LlmBackend for InlineBackend {
        fn complete(&self, _request: &ChatCompletionRequest) -> ChatCompletionResponse {
            ChatCompletionResponse {
                id: "inline".to_string(),
                object: "chat.completion".to_string(),
                created: 1,
                model: "mock-model".to_string(),
                choices: vec![ChatChoice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: "ok".to_string(),
                    },
                    finish_reason: "stop".to_string(),
                }],
            }
        }
    }

    #[test]
    fn run_suite_empty_cases_returns_zero_summary() {
        let runner = TestRunner::new("empty-suite");
        let report = runner.run_suite(&EchoAgent, &InlineBackend, &[]);

        assert_eq!(report.summary.total, 0);
        assert_eq!(report.summary.passed, 0);
        assert_eq!(report.summary.failed, 0);
        assert!(report.cases.is_empty());
    }

    #[test]
    fn run_suite_case_with_no_assertions_is_passed() {
        let runner = TestRunner::new("no-assertions-suite");
        let cases = vec![AgentTestCase {
            name: "no_assertions".to_string(),
            input: "hello".to_string(),
            assertions: vec![],
        }];

        let report = runner.run_suite(&EchoAgent, &InlineBackend, &cases);

        assert_eq!(report.summary.total, 1);
        assert_eq!(report.summary.passed, 1);
        assert_eq!(report.summary.failed, 0);
        assert_eq!(report.cases[0].status, TestStatus::Passed);
    }

    #[test]
    fn run_suite_generates_valid_run_id() {
        let runner = TestRunner::new("run-id-suite");
        let report = runner.run_suite(&EchoAgent, &InlineBackend, &[]);

        assert!(!report.run_id.is_empty());
        assert!(uuid::Uuid::parse_str(&report.run_id).is_ok());
    }

    #[test]
    fn run_suite_records_monotonic_timestamps() {
        let runner = TestRunner::new("timing-suite");
        let cases = vec![AgentTestCase {
            name: "timed".to_string(),
            input: "hello".to_string(),
            assertions: vec![],
        }];

        let report = runner.run_suite(&EchoAgent, &InlineBackend, &cases);

        assert!(report.finished_at_unix_ms >= report.started_at_unix_ms);
        assert_eq!(report.cases.len(), 1);
    }

    #[test]
    fn build_request_populates_fields() {
        let request = build_request("hello", "test-id", 2);
        assert_eq!(request.model, "mock-model");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
        assert_eq!(request.messages[0].content, "hello");
        assert_eq!(request.test_id.as_deref(), Some("test-id"));
        assert_eq!(request.turn, Some(2));
    }
}
