use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::mvp::assertions::AssertionResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseReport {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u128,
    pub assertions: Vec<AssertionResult>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRunReport {
    pub suite_name: String,
    pub run_id: String,
    pub started_at_unix_ms: u128,
    pub finished_at_unix_ms: u128,
    pub summary: TestSummary,
    pub cases: Vec<TestCaseReport>,
}

impl TestRunReport {
    pub fn write_json(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let report_json = serde_json::to_string_pretty(self)?;
        fs::write(path, report_json)?;
        Ok(())
    }
}

pub fn report_now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_now_unix_ms_is_nonzero() {
        assert!(report_now_unix_ms() > 0);
    }

    #[test]
    fn write_json_roundtrips() {
        let report = TestRunReport {
            suite_name: "test".to_string(),
            run_id: "abc".to_string(),
            started_at_unix_ms: 1000,
            finished_at_unix_ms: 2000,
            summary: TestSummary {
                total: 1,
                passed: 1,
                failed: 0,
            },
            cases: vec![],
        };

        let path = std::env::temp_dir().join(format!(
            "mofa_report_unit_{}.json",
            uuid::Uuid::now_v7()
        ));

        report.write_json(&path).expect("report JSON should be writable");
        let raw = std::fs::read_to_string(&path).expect("report file should be readable");
        let parsed: TestRunReport =
            serde_json::from_str(&raw).expect("report json should deserialize");

        assert_eq!(parsed.suite_name, "test");
        assert_eq!(parsed.summary.passed, 1);
    }
}
