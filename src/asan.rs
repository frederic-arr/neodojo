use crate::gunit::{TestCase, TestFailure, TestInfo};
use colored::Colorize;
use std::path::Path;

pub struct Asan;

impl Asan {
    pub fn try_from_file(path: &Path) -> Result<TestCase, TestCase> {
        let file = std::fs::read_to_string(path);
        let mut suite = TestCase {
            name: "sanitizer".to_string(),
            ..Default::default()
        };

        let file = match file {
            Ok(file) => file,
            Err(_) => return Ok(suite),
        };

        for (key, name) in [
            ("address_sanitizer", "AddressSanitizer"),
            ("undefined_behavior_sanitizer", "UndefinedBehaviorSanitizer"),
            ("leak_sanitizer", "LeakSanitizer"),
        ] {
            let search = format!("ERROR: {name}: ");
            let message: Option<String> = file
                .split_once(&search)
                .and_then(|(_, m)| Some(m.to_string().split_once('\n')?.0.to_string()));

            let info = TestInfo {
                name: key.to_string(),
                classname: "sanitizer".to_string(),
                failures: if let Some(message) = message {
                    vec![TestFailure {
                        failure: message,
                        ..Default::default()
                    }]
                } else {
                    vec![]
                },
                ..Default::default()
            };
            suite.testsuite.push(info);
        }

        suite.tests = suite.testsuite.len() as u32;
        suite.errors = (file.contains("DEADLYSIGNAL") || file.contains("ABORTING")).into();
        suite.failures = suite
            .testsuite
            .iter()
            .filter(|info| !info.failures.is_empty())
            .count() as u32;

        if suite.failures == 0 {
            Ok(suite)
        } else {
            print!("{}", file.bright_black());
            Err(suite)
        }
    }
}
