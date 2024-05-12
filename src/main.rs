mod asan;
mod cli;
mod dojo;
mod gunit;
mod sarif;

use clap::Parser;

fn main() {
    let cli = cli::Cli::parse();
    cli.exec();
}
