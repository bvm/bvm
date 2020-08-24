use super::types::ErrBox;

pub struct CliArgs {
    pub sub_command: SubCommand,
}

#[derive(Debug, PartialEq)]
pub enum SubCommand {
    Run(RunCommand),
    Install,
    Version,
    Help(String),
}

#[derive(Debug, PartialEq)]
pub struct RunCommand {
    pub binary_name: String,
    pub args: Vec<String>,
}

pub fn parse_args(args: Vec<String>) -> Result<CliArgs, ErrBox> {
    if args.get(1).map(|a| a.as_str()) == Some("run") {
        return Ok(CliArgs{
            sub_command: SubCommand::Run(RunCommand {
                binary_name: args.get(2).expect("Expected run command to have binary name").clone(), // todo: error instead
                args: args[3..].to_vec(),
            })
        })
    }

    let mut cli_parser = create_cli_parser();
    let matches = match cli_parser.get_matches_from_safe_borrow(args) {
        Ok(result) => result,
        Err(err) => return err!("{}", err.to_string()),
    };

    let sub_command = if matches.is_present("version") {
        SubCommand::Version
    } else if matches.is_present("install") {
        SubCommand::Install
    } else {
        SubCommand::Help({
            let mut text = Vec::new();
            cli_parser.write_help(&mut text).unwrap();
            String::from_utf8(text).unwrap()
        })
    };

    Ok(CliArgs {
        sub_command,
    })
}

fn create_cli_parser<'a, 'b>() -> clap::App<'a, 'b> {
    use clap::{App, Arg, SubCommand, AppSettings};
    App::new("gvm")
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpFlags)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DeriveDisplayOrder)
        .bin_name("gvm")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Copyright 2020 by David Sherret")
        .about("Handles running versions of specific binaries based on the current working directory.")
        .usage("gvm <SUBCOMMAND> [OPTIONS]")
        .template(r#"{bin} {version}
{author}

{about}

USAGE:
    {usage}

SUBCOMMANDS:
{subcommands}

OPTIONS:
{unified}

ARGS:
{positionals}

{after-help}"#)
        .after_help(
            r#"TODO: Will fill in this info later..."#,
        )
        .subcommand(
            SubCommand::with_name("run")
                .about("Runs the command using the version according to the current working directory.")
                .arg(
                    Arg::with_name("command")
                        .help("The command to execute where the first argument is the binary name.")
                        .takes_value(true)
                        .min_values(1)
                )
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Installs the dependencies for the current configuration file.")
        )
        .arg(
            Arg::with_name("help")
                .long("help")
                .short("h")
                .hidden(true)
                .takes_value(false),
        )
        .arg(
            Arg::with_name("version")
                .short("v")
                .long("version")
                .help("Prints the version.")
                .takes_value(false),
        )
}

fn values_to_vec(values: Option<clap::Values>) -> Vec<String> {
    values.map(|x| x.map(std::string::ToString::to_string).collect()).unwrap_or(Vec::new())
}
