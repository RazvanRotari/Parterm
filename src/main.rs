extern crate anyhow;
extern crate clap;
extern crate errno;
extern crate libc;
extern crate nix;
extern crate parterm;
extern crate termion;


use anyhow::{Result};
use clap::{App, Arg, SubCommand};
use flexi_logger::*;
use log::{info};

static DEFAULT_NAME: &str = "default";





fn main() -> Result<()> {
    Logger::try_with_str("info")?
        .log_to_file(FileSpec::default()) // write logs to file
        // .duplicate_to_stderr(Duplicate::Info) // print warnings and errors also to the console
            .format(with_thread)
        .start()?;

    let matches = App::new("parterm")
        .version("0.1")
        .author("Razvan Rotari <razvanrotari@posteo.net>")
        .about("Remote control for your terminal")
        .setting(clap::AppSettings::TrailingVarArg)
        .setting(clap::AppSettings::AllowLeadingHyphen)
        .subcommand(
            SubCommand::with_name("client")
                .about("")
                .arg(
                    Arg::with_name("cmd")
                        .help("<command>")
                        .required(true)
                        .takes_value(true)
                        .last(true),
                )
                .arg(
                    Arg::with_name("name")
                        .help("Name of the connection")
                        .short("n")
                        .long("name")
                        .takes_value(true)
                        .default_value(DEFAULT_NAME),
                ),
        )
        .subcommand(
            SubCommand::with_name("server").about("").arg(
                Arg::with_name("name")
                    .help("Name of the connection")
                    .short("n")
                    .long("name")
                    .takes_value(true)
                    .default_value(DEFAULT_NAME),
            ),
        )
        .get_matches();

    if let Some(client_sub) = matches.subcommand_matches("client") {
        info!("Client");
        match client_sub.value_of("cmd") {
            Some(val) => {
                let path = format!(
                    "parterm_{}.pipe",
                    client_sub.value_of("name").unwrap_or(DEFAULT_NAME)
                );
                if let Err(err) = parterm::parterm::client(val.to_owned() + "\n", &path) {
                    info!("Error {}", err);
                }
            }
            _ => println!("{}", matches.usage()),
        }
        return Ok(());
    }
    if let Some(server_sub) = matches.subcommand_matches("server") {
        info!("server");
        let path = format!(
            "parterm_{}.pipe",
            server_sub.value_of("name").unwrap_or(DEFAULT_NAME)
        );
        if let Err(err) = parterm::parterm::server(path) {
            info!("Error {}", err);
        }
    }
    Ok(())
}
