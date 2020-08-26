use super::types::ErrBox;

pub struct CliArgs {
    pub sub_command: SubCommand,
}

#[derive(Debug, PartialEq)]
pub enum SubCommand {
    Resolve(ResolveCommand),
    Use(UseCommand),
    Install,
    InstallUrl(String),
    Uninstall(UninstallCommand),
    Version,
    Help(String),
}

#[derive(Debug, PartialEq)]
pub struct ResolveCommand {
    pub binary_name: String,
}

#[derive(Debug, PartialEq)]
pub struct UseCommand {
    pub binary_name: String,
    pub version: String,
}

#[derive(Debug, PartialEq)]
pub struct UninstallCommand {
    pub binary_name: String,
    pub version: String,
}

pub fn parse_args(args: Vec<String>) -> Result<CliArgs, ErrBox> {
    let mut cli_parser = create_cli_parser();
    let matches = match cli_parser.get_matches_from_safe_borrow(args) {
        Ok(result) => result,
        Err(err) => return err!("{}", err.to_string()),
    };

    let sub_command = if matches.is_present("resolve") {
        let resolve_matches = matches.subcommand_matches("resolve").unwrap();
        SubCommand::Resolve(ResolveCommand {
            binary_name: resolve_matches.value_of("binary_name").map(String::from).unwrap(),
        })
    } else if matches.is_present("version") {
        SubCommand::Version
    } else if matches.is_present("install") {
        let install_matches = matches.subcommand_matches("install").unwrap();
        if let Some(url) = install_matches.value_of("url").map(String::from) {
            SubCommand::InstallUrl(url)
        } else {
            SubCommand::Install
        }
    } else if matches.is_present("use") {
        let use_matches = matches.subcommand_matches("use").unwrap();
        SubCommand::Use(UseCommand {
            binary_name: use_matches.value_of("binary_name").map(String::from).unwrap(),
            version: use_matches.value_of("version").map(String::from).unwrap(),
        })
    } else if matches.is_present("uninstall") {
        let uninstall_matches = matches.subcommand_matches("uninstall").unwrap();
        SubCommand::Uninstall(UninstallCommand {
            binary_name: uninstall_matches.value_of("binary_name").map(String::from).unwrap(),
            version: uninstall_matches.value_of("version").map(String::from).unwrap(),
        })
    } else {
        SubCommand::Help({
            let mut text = Vec::new();
            cli_parser.write_help(&mut text).unwrap();
            String::from_utf8(text).unwrap()
        })
    };

    Ok(CliArgs { sub_command })
}

fn create_cli_parser<'a, 'b>() -> clap::App<'a, 'b> {
    use clap::{App, AppSettings, Arg, SubCommand};
    App::new("bvm")
        .setting(AppSettings::UnifiedHelpMessage)
        .setting(AppSettings::DisableHelpFlags)
        .setting(AppSettings::DisableHelpSubcommand)
        .setting(AppSettings::DeriveDisplayOrder)
        .bin_name("bvm")
        .version(env!("CARGO_PKG_VERSION"))
        .author("Copyright 2020 by David Sherret")
        .about("Runs versions of specific binaries based on the current working directory.")
        .usage("bvm <SUBCOMMAND> [OPTIONS]")
        .template(
            r#"{bin} {version}
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

{after-help}
"#,
        )
        .after_help(r#"TODO: Will fill in this info later..."#)
        .subcommand(
            SubCommand::with_name("resolve")
                .about("Outputs the binary path according to the current working directory.")
                .arg(
                    Arg::with_name("binary_name")
                        .help("The binary name to resolve.")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("install")
                .about("Installs the binaries for the current configuration file.")
                .arg(
                    Arg::with_name("url")
                        .help("The url of the binary manifest to install.")
                        .takes_value(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("uninstall")
                .about("Uninstalls the specified binary version.")
                .arg(
                    Arg::with_name("binary_name")
                        .help("The binary name.")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("version")
                        .help("The version of the binary to uninstall.")
                        .takes_value(true)
                        .required(true),
                ),
        )
        .subcommand(
            SubCommand::with_name("use")
                .about("Select a different version to use globally of a binary.")
                .arg(
                    Arg::with_name("binary_name")
                        .help("The binary name.")
                        .takes_value(true)
                        .required(true),
                )
                .arg(
                    Arg::with_name("version")
                        .help("The version of the binary to use or 'path' to use the binary on the path.")
                        .takes_value(true)
                        .required(true),
                ),
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
