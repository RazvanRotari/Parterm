use anyhow::Result;
use clap::{Arg, ArgAction, Command};
use log::info;

static DEFAULT_NAME: &str = "default";

fn main() -> Result<()> {
    flexi_logger::Logger::try_with_env()
        .unwrap()
        .start()
        .unwrap();

    let matches = Command::new("parterm")
        .version("0.1")
        .author("Razvan Rotari <razvanrotari@posteo.net>")
        .about("Remote control for your terminal")
        .subcommand_required(true)
        .subcommand(
            Command::new("client")
                .about("")
                .arg(
                    Arg::new("cmd")
                        .help("<command>")
                        .help("Command to run by the server")
                        .required(true)
                        .action(ArgAction::Set)
                        .last(true),
                )
                .arg(
                    Arg::new("name")
                        .help("Name of the connection")
                        .short('n')
                        .long("name")
                        .action(ArgAction::Set)
                        .default_value(DEFAULT_NAME),
                ),
        )
        .subcommand(
            Command::new("server")
                .about("")
                .arg(
                    Arg::new("name")
                        .help("Name of the connection")
                        .short('n')
                        .long("name")
                        .action(ArgAction::Set)
                        .default_value(DEFAULT_NAME),
                )
                .arg(
                    Arg::new("cmd")
                        .help("Command to be executed after the server starts")
                        .short('c')
                        .action(ArgAction::Set)
                        .long("command"),
                ),
        )
        .get_matches();

    if let Some(client_sub) = matches.subcommand_matches("client") {
        info!("Client");
        if let Some(val) = client_sub.get_one::<String>("cmd") {
            let path = format!(
                "parterm_{}.pipe",
                client_sub
                    .get_one::<String>("name")
                    .unwrap_or(&DEFAULT_NAME.to_string())
            );
            if let Err(err) = parterm::parterm::client(val.to_owned() + "\n", &path) {
                info!("Error {}", err);
            }
        }
        return Ok(());
    }
    if let Some(server_sub) = matches.subcommand_matches("server") {
        info!("server");
        let path = format!(
            "parterm_{}.pipe",
            server_sub
                .get_one::<String>("name")
                .unwrap_or(&DEFAULT_NAME.to_string())
        );
        if let Err(err) = parterm::parterm::server(
            path,
            server_sub.get_one::<String>("cmd").map(|x| x.as_str()),
        ) {
            info!("Error {}", err);
        }
        return Ok(());
    }

    Ok(())
}
