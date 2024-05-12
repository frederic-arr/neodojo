use colored::Colorize;
use indent::indent_all_by;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum TestError {
    #[error("unable to read test file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("unable to parse json: {0}")]
    DeserializationError(#[from] serde_json::Error),

    #[error("program crashed during test")]
    ExecutionError(String),

    #[error("test failed")]
    TestFailed(UnitTest),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct UnitTest {
    pub name: String,
    pub tests: u32,
    pub failures: u32,
    pub disabled: u32,
    pub errors: u32,
    pub timestamp: String,
    pub time: String,
    pub testsuites: Vec<TestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct TestCase {
    pub name: String,
    pub tests: u32,
    pub failures: u32,
    pub disabled: u32,
    pub errors: u32,
    pub time: String,
    pub testsuite: Vec<TestInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct TestInfo {
    pub name: String,
    pub file: String,
    pub line: u32,
    pub status: TestStatus,
    pub result: String,
    pub timestamp: String,
    pub time: String,
    pub classname: String,

    #[serde(default)]
    pub failures: Vec<TestFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
pub struct TestFailure {
    pub failure: String,

    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, Default)]
pub enum TestStatus {
    #[default]
    #[serde(rename = "RUN")]
    Run,

    #[serde(rename = "NOTRUN")]
    NotRun,
}

impl TryFrom<TestCase> for UnitTest {
    type Error = TestError;

    fn try_from(test: TestCase) -> Result<UnitTest, Self::Error> {
        let test = UnitTest {
            testsuites: vec![test.clone()],
            name: test.name,
            tests: test.tests,
            failures: test.failures,
            disabled: test.disabled,
            errors: test.errors,
            timestamp: "".to_owned(),
            time: test.time,
        };

        if test.errors > 0 {
            Err(TestError::ExecutionError("".to_string()))
        } else if test.failures > 0 {
            Err(TestError::TestFailed(test))
        } else {
            Ok(test)
        }
    }
}

impl UnitTest {
    pub fn add_suite(&mut self, suite: TestCase) {
        self.tests += suite.tests;
        self.failures += suite.failures;
        self.disabled += suite.disabled;
        self.errors += suite.errors;
        self.testsuites.push(suite);
    }

    pub fn has_failed(&self) -> bool {
        self.failures > 0 || self.errors > 0
    }

    pub fn try_from_file(path: &Path) -> Result<UnitTest, TestError> {
        let file = std::fs::read_to_string(path).map_err(TestError::from)?;
        let test: UnitTest = serde_json::from_str(&file).map_err(TestError::from)?;
        if test.has_failed() {
            Err(TestError::TestFailed(test))
        } else {
            Ok(test)
        }
    }

    pub fn pretty_print(&self) {
        println!("ran {} tests", self.tests);
        for test_suite in &self.testsuites {
            for test in &test_suite.testsuite {
                if test.failures.is_empty() {
                    println!("test {}.{} ... {}", test.classname, test.name, "ok".green());
                } else {
                    println!(
                        "test {}.{} ... {}",
                        test.classname,
                        test.name,
                        "FAILED".red()
                    );
                }
            }
        }
        println!();

        if self.failures > 0 {
            println!("failures:");
            for test_suite in &self.testsuites {
                for test in &test_suite.testsuite {
                    if !test.failures.is_empty() {
                        test.print_details();
                        println!();
                    }
                }
            }

            println!();
        }

        println!(
            "test result: {}. {} passed; {} failed; {} ignored; finished in {}",
            if self.failures > 0 {
                "FAILED".red()
            } else {
                "ok".green()
            },
            self.tests - self.failures,
            self.failures,
            self.disabled,
            self.time
        );
    }
}

impl TestInfo {
    pub fn print_details(&self) {
        println!(
            "    {} {}",
            format!("{}.{}", self.classname, self.name).cyan(),
            format!(
                "({})",
                if self.file.is_empty() {
                    "see logs for details".to_string()
                } else {
                    format!("{}:{}", self.file, self.line)
                }
            )
            .bright_black()
            .italic()
        );

        let printed_failures = self.failures.iter().take(1);
        for failure in printed_failures {
            let (location, message) = failure.message_and_location();
            println!(
                "{} {}",
                indent_all_by(4, message),
                location
                    .map(|s| format!("({s})").bright_black().italic())
                    .unwrap_or_default(),
            );
        }

        if self.failures.len() > 1 {
            println!(
                "{}",
                format!("    ... and {} more", self.failures.len() - 1).magenta()
            );
        }
    }
}

impl TestFailure {
    pub fn message_and_location(&self) -> (Option<String>, String) {
        let (location, message) = self
            .failure
            .split_once('\n')
            .map(|(a, b)| (Some(a.to_string()), b.to_string()))
            .unwrap_or_else(|| (None, self.failure.clone()));

        // if self.failure.contains("Expected equality of these values:") {
        //     let mut data = message.splitn(4, '\n');
        //     data.next();
        //     let var = data.next().unwrap().trim();
        //     let val = data
        //         .next()
        //         .unwrap()
        //         .split_once("Which is: ")
        //         .unwrap()
        //         .1
        //         .trim();
        //     let expected = data.next().unwrap().trim();
        //     let message = format!(
        //         "Expected: {} == {}, actual: {} vs {}",
        //         var, expected, val, expected
        //     );

        //     (location.to_owned(), message)
        // } else {
        (location.to_owned(), message.to_owned())
        // }
    }
}
