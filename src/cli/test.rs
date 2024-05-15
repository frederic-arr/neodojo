use crate::asan::Asan;
use crate::dojo::DojoAssignment;
use crate::gunit::{TestError, UnitTest};
use crate::sarif::BuildDiagnostic;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde_sarif::sarif::Sarif;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Output;
use std::time::Duration;

const DOCKER_COMPOSE: &str = "docker-compose.yml";
const TEST_RESULTS_FILE: &str = "test_detail.json";
const ASAN_FILE: &str = "memory.txt";
const DOJO_ASSIGNMENT_FILE: &str = "dojo_assignment.json";

#[derive(thiserror::Error, Debug)]
enum RunError {
    // #[error("unable to run docker-compose")]
    // DockerCompose,
    #[error("invalid dojo workspace: {0}")]
    DojoWorkspace(PathBuf),

    #[error("error building the project")]
    Build(#[from] BuildError),

    #[error("error running tests: {0}")]
    Test(#[from] TestError),
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("incorrect makefile format: {0}")]
    IncompatibleMakefile(String),

    #[error("build failed")]
    BuildFailed(BuildDiagnostic),
}

pub fn command(root: &Path, filter: &Vec<String>) {
    // dbg!(&filter);
    if let Err(err) = run(root) {
        match &err {
            RunError::Build(b) =>
            {
                #[allow(irrefutable_let_patterns)]
                if let BuildError::BuildFailed(diagnostics) = b {
                    diagnostics.pretty_print();
                }
            }
            RunError::Test(t) => {
                if let TestError::TestFailed(test) = t {
                    test.pretty_print();
                }
            }
            _ => {}
        }

        println!("{}{} {}", "error".red().bold(), ":".bold(), err);
    }
}

fn wrap_progress<F, T, E>(message: &str, f: F) -> Result<T, E>
where
    F: Fn() -> Result<T, E>,
{
    let style = ProgressStyle::with_template("{spinner:.bold.cyan} {wide_msg}")
        .unwrap()
        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏ ");

    let bar = ProgressBar::new_spinner()
        .with_style(style)
        .with_message(message.to_string());
    bar.enable_steady_tick(Duration::from_millis(100));

    let res = f();
    match res {
        Ok(_) => {
            bar.set_style(ProgressStyle::with_template("{msg}").unwrap());
            bar.set_message(format!("{} {message}", "✔".green().bold()));
            bar.finish()
        }
        Err(_) => {
            bar.set_style(ProgressStyle::with_template("{msg}").unwrap());
            bar.set_message(format!("{} {message}", "✖".red().bold()));
            bar.abandon()
        }
    }

    res
}

fn run(root: &Path) -> Result<(), RunError> {
    let assignment = DojoAssignment::try_from_file(&root.join(DOJO_ASSIGNMENT_FILE))
        .map_err(|_| RunError::DojoWorkspace(root.to_path_buf()))?;
    assert_ne!(assignment.result.volume, None);
    let container_name = assignment.result.container.as_str();

    let tempdir = tempfile::tempdir().unwrap().into_path();
    let overrides = create_docker_compose_file(tempdir.to_str().unwrap(), container_name);
    let args = vec![
        "compose",
        // "--project-name",
        // PROJECT_NAME,
        "--file",
        DOCKER_COMPOSE,
        "--file",
        &overrides,
    ];

    wrap_progress("Setting up environment", || {
        exec_run(root, container_name, &args)
    })
    .unwrap();

    wrap_progress("Cleaning up", || exec_clean(root, container_name, &args)).unwrap();

    let diagnostics = wrap_progress("Building project", || {
        exec_build(root, container_name, &args)
    })?;
    diagnostics.pretty_print();

    wrap_progress("Running tests", || {
        exec_test(
            root,
            container_name,
            &args,
            &tempdir.join(TEST_RESULTS_FILE),
        )
    })
    .map_err(RunError::from)?;

    Ok(())
}

fn create_docker_compose_file(dir: &str, container_name: &str) -> String {
    let compose = format!(
        r#"services:
    {container_name}:
        entrypoint:
            - sleep
            - infinity
        environment:
            - IN_DOCKER=true
        volumes: !override
            - {dir}:/results/
        "#,
        container_name = container_name,
        dir = dir
    );

    let path = format!("{dir}/docker-compose.yml");
    let mut compose_file = std::fs::File::create(&path).unwrap();
    compose_file.write_all(compose.as_bytes()).unwrap();
    path
}

fn exec_run(
    root: &Path,
    container_name: &str,
    common_args: &[&str],
) -> Result<Output, std::io::Error> {
    std::process::Command::new("docker")
        .args(common_args)
        .arg("run")
        .arg("-d")
        .arg("--rm")
        .arg("--build")
        .arg(container_name)
        .current_dir(root)
        .output()
}

fn exec_clean(
    root: &Path,
    container_name: &str,
    common_args: &[&str],
) -> Result<Output, std::io::Error> {
    std::process::Command::new("docker")
        .args(common_args)
        .arg("exec")
        .arg(container_name)
        .arg("make")
        .arg("-C")
        .arg("src")
        .arg("clean")
        .arg("-s")
        .current_dir(root)
        .output()
}

fn exec_build(
    root: &Path,
    container_name: &str,
    common_args: &[&str],
) -> Result<BuildDiagnostic, BuildError> {
    let makefile = std::fs::read_to_string(root.join("src/Makefile"))
        .map_err(|_| BuildError::IncompatibleMakefile("unable to read Makefile".to_string()))?;

    let (_, cflags) = makefile
        .lines()
        .find(|line| line.starts_with("CFLAGS:="))
        .ok_or(BuildError::IncompatibleMakefile("missing CFLAGS".to_string()))?
        .split_once('=')
        .ok_or(BuildError::IncompatibleMakefile("missing CFLAGS".to_string()))?;

    let build = std::process::Command::new("docker")
        .args(common_args)
        .arg("exec")
        .arg(container_name)
        .arg("make")
        .arg("-C")
        .arg("src")
        .arg("tests")
        .arg(format!("CFLAGS={cflags} -fdiagnostics-format=sarif-stderr"))
        .current_dir(root)
        .output()
        .unwrap();

    let mut diagnostics = BuildDiagnostic::default();
    for report in build.stderr.lines() {
        let sarif: Sarif = match serde_json::from_str(&report.unwrap()) {
            Ok(sarif) => sarif,
            Err(_) => continue,
        };

        diagnostics += BuildDiagnostic::from(sarif);
    }

    if diagnostics.has_errors() {
        Err(BuildError::BuildFailed(diagnostics))
    } else {
        Ok(diagnostics)
    }
}

fn exec_test(
    root: &Path,
    container_name: &str,
    common_args: &[&str],
    results: &Path,
) -> Result<UnitTest, TestError> {
    let _ = std::process::Command::new("docker")
        .args(common_args)
        .arg("exec")
        .arg(container_name)
        .arg("make")
        .arg("-C")
        .arg("src")
        .arg("run_tests")
        .current_dir(root)
        .output()
        .unwrap();

    let asan = Asan::try_from_file(&results.with_file_name(ASAN_FILE));
    let gunit = UnitTest::try_from_file(results);
    match (gunit, asan) {
        (Ok(mut gunit), Ok(asan)) => {
            gunit.add_suite(asan);
            Ok(gunit)
        },
        | (Ok(mut gunit), Err(asan))
        | (Err(TestError::TestFailed(mut gunit)), Ok(asan))
        | (Err(TestError::TestFailed(mut gunit)), Err(asan)) => {
            gunit.add_suite(asan);
            Err(TestError::TestFailed(gunit))
        }
        (_, Err(err)) => UnitTest::try_from(err),
        (Err(err), _) => Err(err),
    }
}
