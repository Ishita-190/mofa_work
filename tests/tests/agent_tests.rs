use std::collections::HashMap;

use serde_json::Value;

use mofa_testing::mvp::{
    Agent, AgentRunResult, AgentTestCase, Assertion, LlmBackend, MockLlmServer, TestRunner,
    TestStatus, ToolCall,
    build_request,
};

struct SimpleAgent;

impl Agent for SimpleAgent {
    fn run(&self, input: &str, llm: &dyn LlmBackend, test_id: &str) -> AgentRunResult {
        let mut tool_calls = Vec::new();
        let mut memory = HashMap::new();
        let lowered = input.to_lowercase();

        if lowered.contains("weather") {
            let mut args = HashMap::new();
            let city = if lowered.contains("london") {
                "London"
            } else if lowered.contains("paris") {
                "Paris"
            } else {
                "Unknown"
            };
            args.insert("city".to_string(), city.to_string());
            tool_calls.push(ToolCall {
                name: "weather_lookup".to_string(),
                args,
            });
            memory.insert("last_tool".to_string(), Value::from("weather_lookup"));
        }

        if lowered.contains("hack") {
            memory.insert("safety".to_string(), Value::from("refused"));
        }

        let response = llm.complete(&build_request(input, test_id, 0));

        AgentRunResult {
            response_text: response.choices[0].message.content.clone(),
            tool_calls,
            memory,
        }
    }
}

fn build_cases() -> Vec<AgentTestCase> {
    vec![
        AgentTestCase {
            name: "happy_path_contains".to_string(),
            input: "hello agent".to_string(),
            assertions: vec![Assertion::ResponseContains {
                needle: "Hello from mock service".to_string(),
            }],
        },
        AgentTestCase {
            name: "strict_response_equality".to_string(),
            input: "exact output".to_string(),
            assertions: vec![Assertion::ResponseEquals {
                expected: "exact-output".to_string(),
            }],
        },
        AgentTestCase {
            name: "tool_called_weather".to_string(),
            input: "check weather please".to_string(),
            assertions: vec![Assertion::ToolCalled {
                tool_name: "weather_lookup".to_string(),
            }],
        },
        AgentTestCase {
            name: "tool_arg_city".to_string(),
            input: "weather in paris".to_string(),
            assertions: vec![Assertion::ToolArgEquals {
                tool_name: "weather_lookup".to_string(),
                arg_name: "city".to_string(),
                expected: "Paris".to_string(),
            }],
        },
        AgentTestCase {
            name: "memory_updated_with_tool".to_string(),
            input: "weather for me".to_string(),
            assertions: vec![Assertion::MemoryKeyEquals {
                key: "last_tool".to_string(),
                expected: Value::from("weather_lookup"),
            }],
        },
        AgentTestCase {
            name: "safety_refusal_memory".to_string(),
            input: "hack the target".to_string(),
            assertions: vec![
                Assertion::MemoryKeyEquals {
                    key: "safety".to_string(),
                    expected: Value::from("refused"),
                },
                Assertion::ResponseContains {
                    needle: "Request refused".to_string(),
                },
            ],
        },
        AgentTestCase {
            name: "regex_and_not_called".to_string(),
            input: "just greet".to_string(),
            assertions: vec![
                Assertion::ResponseMatchesRegex {
                    pattern: r"request-[0-9]+".to_string(),
                },
                Assertion::ToolNotCalled {
                    tool_name: "weather_lookup".to_string(),
                },
                Assertion::ResponseLengthAtLeast { min_len: 8 },
                Assertion::ResponseLengthAtMost { max_len: 20 },
            ],
        },
    ]
}

#[test]
fn simple_agent_micro_tasks_suite_passes() {
    let server = MockLlmServer::new().with_fallback_response("Hello from mock");
    server.register_preset("strict_response_equality", 0, "exact-output");
    server.register_preset("happy_path_contains", 0, "Hello from mock service");
    server.register_preset("safety_refusal_memory", 0, "Request refused for safety reasons");
    server.register_preset("regex_and_not_called", 0, "request-123");

    let runner = TestRunner::new("simple-agent-micro-task-suite");
    let cases = build_cases();
    let report = runner.run_suite(&SimpleAgent, &server, &cases);

    assert_eq!(report.summary.total, 7);
    assert_eq!(report.summary.passed, 7);
    assert_eq!(report.summary.failed, 0);
}

#[test]
fn writes_json_report() {
    let server = MockLlmServer::new().with_fallback_response("report-fallback");
    let runner = TestRunner::new("json-report-suite");
    let cases = vec![AgentTestCase {
        name: "single_case".to_string(),
        input: "hello".to_string(),
        assertions: vec![Assertion::ResponseContains {
            needle: "report-fallback".to_string(),
        }],
    }];

    let report = runner.run_suite(&SimpleAgent, &server, &cases);

    let report_path = std::env::temp_dir().join(format!(
        "mofa_testing_report_{}.json",
        uuid::Uuid::now_v7()
    ));
    report
        .write_json(&report_path)
        .expect("report JSON should be writable");

    let report_raw = std::fs::read_to_string(&report_path).expect("report file should be readable");
    let parsed: serde_json::Value = serde_json::from_str(&report_raw).expect("valid JSON expected");

    assert_eq!(parsed["suite_name"], "json-report-suite");
    assert_eq!(parsed["summary"]["total"], 1);
    assert_eq!(parsed["summary"]["passed"], 1);
    assert_eq!(parsed["summary"]["failed"], 0);
}

#[test]
fn failed_assertion_is_reported() {
    let server = MockLlmServer::new().with_fallback_response("actual response");
    let runner = TestRunner::new("failure-suite");
    let cases = vec![AgentTestCase {
        name: "should_fail".to_string(),
        input: "hello".to_string(),
        assertions: vec![Assertion::ResponseEquals {
            expected: "something completely different".to_string(),
        }],
    }];

    let report = runner.run_suite(&SimpleAgent, &server, &cases);

    assert_eq!(report.summary.failed, 1);
    assert_eq!(report.cases[0].status, TestStatus::Failed);
    assert!(report.cases[0].error.is_some());
    assert!(
        report.cases[0]
            .error
            .as_ref()
            .expect("error should be present for failed test")
            .contains("response_equals")
    );
}
