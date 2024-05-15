mod test;

use anstyle::{AnsiColor, Color::Ansi, Style};
use clap::{builder::Styles, Command, CommandFactory, Parser, Subcommand, ValueHint};
use clap_complete::{generate, Generator};
use itertools::Itertools;
use std::path::Path;

#[derive(Debug, Parser)]
#[command(version, name = "neodojo")]
#[command(about = "A better Dojo CLI")]
#[command(after_help = "This is an experimental CLI tool, use at your own risk.")]
#[command(styles=STYLES, help_template = help_template("help", false, true, true))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[command(flatten)]
    pub verbose: clap_verbosity_flag::Verbosity,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run the exercise test suite
    #[command(subcommand_help_heading = "Exercise")]
    #[command(styles=STYLES, help_template = help_template("command", false, true, false))]
    Test {
        /// Path to the exercise directory
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        #[clap(short, long, default_value = "./")]
        path: Box<Path>,

        /// Watch for changes and re-run tests automatically
        #[clap(short, long)]
        watch: bool,

        /// Show the raw output from the test runner
        #[clap(short, long)]
        raw: bool,

        #[clap(short, long, default_values_t = Vec::<String>::default())]
        filter: Vec<String>,
    },

    /// Upgrade neodojo to the latest version
    #[command(subcommand_help_heading = "Misc")]
    #[command(styles=STYLES, help_template = help_template("command", false, true, false))]
    Upgrade {
        /// Only check for updates
        #[arg(short, long)]
        check: bool,
    },

    /// Generate shell completions
    #[command(styles=STYLES, help_template = help_template("command", true, false, false))]
    Completion {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

impl Cli {
    pub fn exec(&self) {
        let mut cmd = Cli::command();
        match &self.command {
            Commands::Test { path, filter, .. } => {
                test::command(path, filter);
            }
            Commands::Completion { shell } => print_completions(*shell, &mut cmd),
            Commands::Upgrade { check } => {
                println!("Checking for updates...");
                if *check {
                }
            }
        }
    }
}


const HEADER_STYLE: Style = Style::new().bold().fg_color(Some(Ansi(AnsiColor::Green)));
const STYLES: Styles = Styles::styled()
    .literal(AnsiColor::BrightCyan.on_default().bold())
    .placeholder(AnsiColor::BrightCyan.on_default());

fn help_template(
    template: &str,
    has_arguments: bool,
    has_options: bool,
    has_commands: bool,
) -> String {
    let header = HEADER_STYLE.render();
    let rheader = HEADER_STYLE.render_reset();

    let sections = [(
            format!("{header}Arguments{rheader}:\n{{positionals}}"),
            has_arguments,
        ),
        (
            format!("{header}Options{rheader}:\n{{options}}"),
            has_options,
        ),
        (
            format!("{header}Commands{rheader}:\n{{subcommands}}"),
            has_commands,
        )]
    .iter()
    .filter(|(_, has)| *has)
    .map(|(s, _)| s)
    .cloned()
    .collect_vec()
    .join("\n\n");

    match template {
        "help" => format!(
            "\
neodojo v{{version}}
{{author-with-newline}}{{about-with-newline}}
{header}Usage{rheader}: {{usage}}

{sections}
{{after-help}}
"
        ),
        _ => format!(
            "\
{{about-with-newline}}
{header}Usage{rheader}: {{usage}}

{sections}
{{after-help}}
"
        ),
    }
}

fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
    generate(gen, cmd, cmd.get_name().to_string(), &mut std::io::stdout());
}
