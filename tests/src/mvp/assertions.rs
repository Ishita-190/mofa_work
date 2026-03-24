use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub args: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestContext {
    pub response_text: String,
    pub tool_calls: Vec<ToolCall>,
    pub memory: HashMap<String, Value>,
}

impl TestContext {
    pub fn new(response_text: impl Into<String>) -> Self {
        Self {
            response_text: response_text.into(),
            tool_calls: Vec::new(),
            memory: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssertionResult {
    pub name: String,
    pub passed: bool,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum Assertion {
    ResponseContains { needle: String },
    ResponseNotContains { needle: String },
    ResponseMatchesRegex { pattern: String },
    ResponseEquals { expected: String },
    ResponseLengthAtLeast { min_len: usize },
    ResponseLengthAtMost { max_len: usize },
    ToolCalled { tool_name: String },
    ToolNotCalled { tool_name: String },
    ToolCallCount { tool_name: String, expected: usize },
    ToolArgEquals {
        tool_name: String,
        arg_name: String,
        expected: String,
    },
    MemoryKeyExists { key: String },
    MemoryKeyNotExists { key: String },
    MemoryKeyEquals { key: String, expected: Value },
}

impl Assertion {
    pub fn evaluate(&self, context: &TestContext) -> AssertionResult {
        match self {
            Self::ResponseContains { needle } => {
                let passed = context.response_text.contains(needle);
                AssertionResult {
                    name: "response_contains".to_string(),
                    passed,
                    message: if passed {
                        format!("Response contains '{needle}'")
                    } else {
                        format!("Response did not contain '{needle}'")
                    },
                    expected: Some(needle.clone()),
                    actual: Some(context.response_text.clone()),
                }
            }
            Self::ResponseNotContains { needle } => {
                let passed = !context.response_text.contains(needle);
                AssertionResult {
                    name: "response_not_contains".to_string(),
                    passed,
                    message: if passed {
                        format!("Response does not contain '{needle}'")
                    } else {
                        format!("Response unexpectedly contained '{needle}'")
                    },
                    expected: Some(needle.clone()),
                    actual: Some(context.response_text.clone()),
                }
            }
            Self::ResponseMatchesRegex { pattern } => {
                let compiled = Regex::new(pattern);
                let (passed, message) = match compiled {
                    Ok(regex) => {
                        let matched = regex.is_match(&context.response_text);
                        (
                            matched,
                            if matched {
                                format!("Response matches regex '{pattern}'")
                            } else {
                                format!("Response did not match regex '{pattern}'")
                            },
                        )
                    }
                    Err(err) => (
                        false,
                        format!("Invalid regex pattern '{pattern}': {err}"),
                    ),
                };

                AssertionResult {
                    name: "response_matches_regex".to_string(),
                    passed,
                    message,
                    expected: Some(pattern.clone()),
                    actual: Some(context.response_text.clone()),
                }
            }
            Self::ResponseEquals { expected } => {
                let passed = context.response_text == *expected;
                AssertionResult {
                    name: "response_equals".to_string(),
                    passed,
                    message: if passed {
                        "Response exactly matches expected text".to_string()
                    } else {
                        "Response does not match expected text".to_string()
                    },
                    expected: Some(expected.clone()),
                    actual: Some(context.response_text.clone()),
                }
            }
            Self::ResponseLengthAtLeast { min_len } => {
                let actual_len = context.response_text.chars().count();
                let passed = actual_len >= *min_len;
                AssertionResult {
                    name: "response_length_at_least".to_string(),
                    passed,
                    message: if passed {
                        format!("Response length {actual_len} is >= {min_len}")
                    } else {
                        format!("Response length {actual_len} is < {min_len}")
                    },
                    expected: Some(min_len.to_string()),
                    actual: Some(actual_len.to_string()),
                }
            }
            Self::ResponseLengthAtMost { max_len } => {
                let actual_len = context.response_text.chars().count();
                let passed = actual_len <= *max_len;
                AssertionResult {
                    name: "response_length_at_most".to_string(),
                    passed,
                    message: if passed {
                        format!("Response length {actual_len} is <= {max_len}")
                    } else {
                        format!("Response length {actual_len} is > {max_len}")
                    },
                    expected: Some(max_len.to_string()),
                    actual: Some(actual_len.to_string()),
                }
            }
            Self::ToolCalled { tool_name } => {
                let passed = context.tool_calls.iter().any(|call| call.name == *tool_name);
                AssertionResult {
                    name: "tool_called".to_string(),
                    passed,
                    message: if passed {
                        format!("Tool '{tool_name}' was called")
                    } else {
                        format!("Tool '{tool_name}' was not called")
                    },
                    expected: Some(tool_name.clone()),
                    actual: Some(
                        context
                            .tool_calls
                            .iter()
                            .map(|call| call.name.clone())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                }
            }
            Self::ToolNotCalled { tool_name } => {
                let passed = !context.tool_calls.iter().any(|call| call.name == *tool_name);
                AssertionResult {
                    name: "tool_not_called".to_string(),
                    passed,
                    message: if passed {
                        format!("Tool '{tool_name}' was not called")
                    } else {
                        format!("Tool '{tool_name}' was unexpectedly called")
                    },
                    expected: Some(tool_name.clone()),
                    actual: Some(
                        context
                            .tool_calls
                            .iter()
                            .map(|call| call.name.clone())
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                }
            }
            Self::ToolCallCount {
                tool_name,
                expected,
            } => {
                let actual_count = context
                    .tool_calls
                    .iter()
                    .filter(|call| call.name == *tool_name)
                    .count();
                let passed = actual_count == *expected;
                AssertionResult {
                    name: "tool_call_count".to_string(),
                    passed,
                    message: if passed {
                        format!("Tool '{tool_name}' call count matched {expected}")
                    } else {
                        format!(
                            "Tool '{tool_name}' call count mismatch: expected {expected}, got {actual_count}"
                        )
                    },
                    expected: Some(expected.to_string()),
                    actual: Some(actual_count.to_string()),
                }
            }
            Self::ToolArgEquals {
                tool_name,
                arg_name,
                expected,
            } => {
                let actual = context
                    .tool_calls
                    .iter()
                    .find(|call| call.name == *tool_name)
                    .and_then(|call| call.args.get(arg_name))
                    .cloned();

                let passed = context
                    .tool_calls
                    .iter()
                    .filter(|call| call.name == *tool_name)
                    .any(|call| call.args.get(arg_name).is_some_and(|value| value == expected));
                AssertionResult {
                    name: "tool_arg_equals".to_string(),
                    passed,
                    message: if passed {
                        format!("Tool '{tool_name}' argument '{arg_name}' matches")
                    } else {
                        format!(
                            "Tool '{tool_name}' argument '{arg_name}' did not match expected value"
                        )
                    },
                    expected: Some(expected.clone()),
                    actual,
                }
            }
            Self::MemoryKeyExists { key } => {
                let passed = context.memory.contains_key(key);
                let actual = context
                    .memory
                    .get(key)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "<not present>".to_string());
                AssertionResult {
                    name: "memory_key_exists".to_string(),
                    passed,
                    message: if passed {
                        format!("Memory key '{key}' exists")
                    } else {
                        format!("Memory key '{key}' does not exist")
                    },
                    expected: Some(key.clone()),
                    actual: Some(actual),
                }
            }
            Self::MemoryKeyNotExists { key } => {
                let passed = !context.memory.contains_key(key);
                let actual = context
                    .memory
                    .get(key)
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "<not present>".to_string());
                AssertionResult {
                    name: "memory_key_not_exists".to_string(),
                    passed,
                    message: if passed {
                        format!("Memory key '{key}' does not exist")
                    } else {
                        format!("Memory key '{key}' unexpectedly exists")
                    },
                    expected: Some(key.clone()),
                    actual: Some(actual),
                }
            }
            Self::MemoryKeyEquals { key, expected } => {
                let actual = context.memory.get(key).cloned();
                let passed = actual.as_ref().is_some_and(|value| value == expected);
                AssertionResult {
                    name: "memory_key_equals".to_string(),
                    passed,
                    message: if passed {
                        format!("Memory key '{key}' matches expected value")
                    } else {
                        format!("Memory key '{key}' did not match expected value")
                    },
                    expected: Some(expected.to_string()),
                    actual: actual.map(|value| value.to_string()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_contains_passes() {
        let ctx = TestContext::new("hello world");
        let result = Assertion::ResponseContains {
            needle: "world".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn response_equals_fails_when_different() {
        let ctx = TestContext::new("hello world");
        let result = Assertion::ResponseEquals {
            expected: "HELLO WORLD".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn tool_called_passes() {
        let mut ctx = TestContext::new("ok");
        ctx.tool_calls.push(ToolCall {
            name: "weather".to_string(),
            args: HashMap::new(),
        });

        let result = Assertion::ToolCalled {
            tool_name: "weather".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn tool_arg_equals_passes() {
        let mut ctx = TestContext::new("ok");
        let mut args = HashMap::new();
        args.insert("city".to_string(), "Paris".to_string());
        ctx.tool_calls.push(ToolCall {
            name: "weather".to_string(),
            args,
        });

        let result = Assertion::ToolArgEquals {
            tool_name: "weather".to_string(),
            arg_name: "city".to_string(),
            expected: "Paris".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn memory_key_equals_passes() {
        let mut ctx = TestContext::new("ok");
        ctx.memory.insert("status".to_string(), Value::from("done"));

        let result = Assertion::MemoryKeyEquals {
            key: "status".to_string(),
            expected: Value::from("done"),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn response_not_contains_passes() {
        let ctx = TestContext::new("hello world");
        let result = Assertion::ResponseNotContains {
            needle: "forbidden".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn response_not_contains_fails_when_present() {
        let ctx = TestContext::new("hello world");
        let result = Assertion::ResponseNotContains {
            needle: "world".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn response_matches_regex_passes() {
        let ctx = TestContext::new("request id: abc-123");
        let result = Assertion::ResponseMatchesRegex {
            pattern: r"abc-\d+".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn response_matches_regex_fails_on_no_match() {
        let ctx = TestContext::new("hello world");
        let result = Assertion::ResponseMatchesRegex {
            pattern: r"\d+".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn response_matches_regex_fails_on_invalid_pattern() {
        let ctx = TestContext::new("hello");
        let result = Assertion::ResponseMatchesRegex {
            pattern: r"[invalid".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
        assert!(result.message.contains("Invalid regex"));
    }

    #[test]
    fn tool_not_called_passes() {
        let ctx = TestContext::new("ok");
        let result = Assertion::ToolNotCalled {
            tool_name: "weather".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn tool_not_called_fails_when_called() {
        let mut ctx = TestContext::new("ok");
        ctx.tool_calls.push(ToolCall {
            name: "weather".to_string(),
            args: HashMap::new(),
        });

        let result = Assertion::ToolNotCalled {
            tool_name: "weather".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn tool_call_count_passes() {
        let mut ctx = TestContext::new("ok");
        ctx.tool_calls.push(ToolCall {
            name: "weather".to_string(),
            args: HashMap::new(),
        });
        ctx.tool_calls.push(ToolCall {
            name: "weather".to_string(),
            args: HashMap::new(),
        });

        let result = Assertion::ToolCallCount {
            tool_name: "weather".to_string(),
            expected: 2,
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn tool_call_count_fails_on_mismatch() {
        let ctx = TestContext::new("ok");
        let result = Assertion::ToolCallCount {
            tool_name: "weather".to_string(),
            expected: 2,
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn memory_key_exists_passes() {
        let mut ctx = TestContext::new("ok");
        ctx.memory.insert("status".to_string(), Value::from("done"));

        let result = Assertion::MemoryKeyExists {
            key: "status".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn memory_key_exists_fails_when_missing() {
        let ctx = TestContext::new("ok");
        let result = Assertion::MemoryKeyExists {
            key: "status".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn memory_key_not_exists_passes() {
        let ctx = TestContext::new("ok");
        let result = Assertion::MemoryKeyNotExists {
            key: "status".to_string(),
        }
        .evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn memory_key_not_exists_fails_when_present() {
        let mut ctx = TestContext::new("ok");
        ctx.memory.insert("status".to_string(), Value::from("done"));

        let result = Assertion::MemoryKeyNotExists {
            key: "status".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn response_length_at_least_fails_when_shorter() {
        let ctx = TestContext::new("hello");
        let result = Assertion::ResponseLengthAtLeast { min_len: 10 }.evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn response_length_at_most_fails_when_longer() {
        let ctx = TestContext::new("hello world");
        let result = Assertion::ResponseLengthAtMost { max_len: 5 }.evaluate(&ctx);
        assert!(!result.passed);
    }

    #[test]
    fn response_length_uses_character_count_not_bytes() {
        let ctx = TestContext::new("hi\u{1F642}");
        let result = Assertion::ResponseLengthAtMost { max_len: 3 }.evaluate(&ctx);
        assert!(result.passed);
    }

    #[test]
    fn tool_arg_equals_fails_when_arg_value_mismatch() {
        let mut ctx = TestContext::new("ok");
        let mut args = HashMap::new();
        args.insert("city".to_string(), "Paris".to_string());
        ctx.tool_calls.push(ToolCall {
            name: "weather".to_string(),
            args,
        });

        let result = Assertion::ToolArgEquals {
            tool_name: "weather".to_string(),
            arg_name: "city".to_string(),
            expected: "London".to_string(),
        }
        .evaluate(&ctx);
        assert!(!result.passed);
    }
}
