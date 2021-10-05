extern crate chan_signal;
extern crate clap;
extern crate termion;
extern crate parterm;

use chan_signal::{notify, Signal};
use clap::{App, Arg, SubCommand};
use std::fs::File;
use std::io::{Read, Result, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;
use parterm::get_shell;
use parterm::pty::Pty;
use parterm::tui::{get_terminal_size, Size};
use termion::get_tty;
use termion::raw::IntoRawMode;

static ADDRESS: &str = "127.0.0.1:10000";

fn client(value: String) {
    if let Ok(mut stream)  = TcpStream::connect(ADDRESS) {
        stream.write(value.as_bytes());
        stream.shutdown(std::net::Shutdown::Both);
    } else {
        println!("Unable to connect to server {}", ADDRESS);
    }
}

fn main() {
    let matches = App::new("parterm")
        .version("0.1")
        .author("Razvan Rotari <razvanrotari@posteo.net>")
        .about("Remote control for your terminal")
        .setting(clap::AppSettings::TrailingVarArg)
        .setting(clap::AppSettings::AllowLeadingHyphen)
        .subcommand(SubCommand::with_name("client")
                    .about("")

                    .arg(Arg::with_name("cmd")
                         .help("<command>")
                         .required(true)
                         .takes_value(true)
                         .multiple(true)
                         .last(true)
                        ))
        .subcommand(SubCommand::with_name("server")
                    .about(""))
        .get_matches();

    if let Some(client_sub) = matches.subcommand_matches("client") {
        match client_sub.value_of("cmd") {
            Some(val) => client(val.to_owned() + "\n"),
            _ => println!("{}", matches.usage()),
        }
        return;
    }
    let signal = notify(&[Signal::WINCH]);

    let mut tty_output = get_tty().unwrap().into_raw_mode().unwrap();
    let mut tty_input = tty_output.try_clone().unwrap();

    let pty_resize = Pty::spawn(&get_shell(), &get_terminal_size().unwrap()).unwrap();
    let mut pty_output = pty_resize.try_clone().unwrap();
    let mut pty_input = pty_output.try_clone().unwrap();

    let (cmd_sender, cmd_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();
    let (val_sender, val_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();

    let handle = thread::spawn(move || loop {
        match pipe(&mut pty_input, &mut tty_output) {
            Err(_) => return,
            _ => (),
        }
    });

    thread::spawn(move || loop {
        let mut packet = [0; 4096];

        let count = tty_input.read(&mut packet).unwrap();
        let (sub_slice,_) = packet.split_at(count);
        val_sender.send(sub_slice.to_vec());
    });

    thread::spawn(move || handle_slave_output(cmd_receiver, val_receiver, pty_output));

    thread::spawn(move || loop {
        signal.recv().unwrap();
        pty_resize.resize(&get_terminal_size().unwrap());
    });

    thread::spawn(move || read_comands(cmd_sender));

    handle.join();
}

fn read_comands(cmd_sender: Sender<Vec<u8>>) {
    let listener = TcpListener::bind(ADDRESS).unwrap();

    for streamRes in listener.incoming() {
        let mut stream = streamRes.unwrap();
        stream.set_read_timeout(Some(Duration::from_millis(1)));
        loop {
            let mut data = [0; 256];
            match stream.read(&mut data) {
                Ok(bytes) => {
                    if bytes == 0 {
                        break;
                    }
                    cmd_sender.send(Vec::from(data));
                }
                Err(err) => {
                    println!("{}", err);
                    break;
                }
            }
        }
    }
}

fn handle_slave_output(
    cmd_receiver: Receiver<Vec<u8>>,
    val_receiver: Receiver<Vec<u8>>,
    mut pty_output: File,
    ) {
    loop {
        match cmd_receiver.try_recv() {
            Ok(val) => {
                pty_output.write_all(val.as_slice());
                pty_output.flush();
            }
            _ => (),
        }

        match val_receiver.try_recv() {
            Ok(val) => {
                pty_output.write_all(val.as_slice());
                pty_output.flush();
            }
            _ => (),
        }
    }
}

/// Sends the content of input into output
fn pipe(input: &mut File, output: &mut File) -> Result<()> {
    let mut packet = [0; 4096];

    let count = input.read(&mut packet)?;

    let read = &packet[..count];
    output.write_all(&read)?;
    output.flush()?;

    Ok(())
}
