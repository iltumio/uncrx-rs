use super::errors::UncrxCliError;
use crate::Cli;
use clap::CommandFactory;

pub fn exit_with_error(error: UncrxCliError) {
    let mut cmd = Cli::command();
    cmd.error(error.clone().into(), &error.to_string()).exit();
}
