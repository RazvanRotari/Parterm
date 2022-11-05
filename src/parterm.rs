use crate::shell::pty::Pty;
use crate::shell::tui::get_terminal_size;
use crate::shell::util::get_shell;
use anyhow::{bail, Result};
use crossbeam_channel::select;
use libc::c_int;
use log::{debug, error};
use nix::sys::stat;
use nix::unistd;
use std::env::temp_dir;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::panic;
use std::path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use std::time::Duration;
use termion::get_tty;
use termion::raw::IntoRawMode;

fn spawn_with_name<F, T>(name: &str, f: F) -> thread::JoinHandle<T>
where
    F: FnOnce() -> T,
    F: Send + 'static,
    T: Send + 'static,
{
    thread::Builder::new()
        .name(name.into())
        .spawn(f)
        .expect("failed to spawn thread")
}

fn notify(signals: &[c_int]) -> Result<crossbeam_channel::Receiver<c_int>> {
    let (s, r) = crossbeam_channel::bounded(100);
    let mut signals = signal_hook::iterator::Signals::new(signals)?;
    thread::spawn(move || {
        for signal in signals.forever() {
            if s.send(signal).is_err() {
                break;
            }
        }
    });
    Ok(r)
}

fn create_pipe(path: &path::PathBuf) -> Result<()> {
    match unistd::mkfifo(path, stat::Mode::S_IRWXU) {
        Ok(_) => (),
        Err(err) => bail!("Error creating fifo: {}", err),
    }
    Ok(())
}

fn get_pipe(name: &str, write: bool) -> Result<File> {
    let dir = temp_dir();
    let pipe_file = dir.join(path::PathBuf::from(name));
    debug!("pipe_file {:?}", pipe_file);
    if !pipe_file.exists() {
        if write {
            bail!("No server open for {} ", name);
        }
        create_pipe(&pipe_file)?
    }
    let mut option = OpenOptions::new();
    let pipe = option
        .read(!write)
        .create(false) // It will crash if is's set to true
        .write(write)
        .open(pipe_file)?;

    debug!("Pipe open");
    Ok(pipe)
}

fn delete_pipe(name: &str) -> std::io::Result<()> {
    let dir = temp_dir();
    let pipe_file = dir.join(path::PathBuf::from(name));
    debug!("remove pipe_file {:?}", pipe_file);
    std::fs::remove_file(pipe_file)
}

pub fn client(value: String, name: &str) -> Result<()> {
    let mut pipe = get_pipe(name, true)?;
    let written = pipe.write(value.as_bytes())?;
    assert_eq!(written, value.as_bytes().len());
    pipe.sync_all()?;
    Ok(())
}

pub fn server(name: String, program: Option<&str>) -> Result<()> {
    let mut tty_output = get_tty().unwrap().into_raw_mode().unwrap();
    let mut tty_input = tty_output.try_clone().unwrap();

    let pty_resize = Pty::spawn(&get_shell(), &get_terminal_size().unwrap()).unwrap();
    let pty_output = pty_resize.try_clone().unwrap();
    let mut pty_input = pty_output.try_clone().unwrap();

    let (cmd_sender, cmd_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();
    let (val_sender, val_receiver): (Sender<Vec<u8>>, Receiver<Vec<u8>>) = channel();

    let handle: thread::JoinHandle<Result<(), anyhow::Error>> = thread::spawn(move || loop {
        if let Err(err) = pipe(&mut pty_input, &mut tty_output) {
            bail!(err)
        }
    });

    spawn_with_name("ReadCmdTerm", move || loop {
        let mut packet = [0; 4096];

        let count = tty_input.read(&mut packet).unwrap();
        let (sub_slice, _) = packet.split_at(count);
        if let Err(err) = val_sender.send(sub_slice.to_vec()) {
            panic!("Fail to send: {}", err);
        }
    });

    spawn_with_name("HandleSlaveOutput", move || {
        handle_slave_output(cmd_receiver, val_receiver, pty_output)
    });

    let name_copy = name.clone();
    let name_copy2 = name.clone();
    //Resize thread
    spawn_with_name("SignalHandler", move || {
        use signal_hook::consts::signal;
        let signal = notify(&[signal::SIGWINCH, signal::SIGTERM]).unwrap();
        let handle_signal = |signal_value| {
            debug!("Handle signal {}", signal_value);
            match signal_value {
                signal::SIGWINCH => {
                    signal.recv().unwrap();
                    if let Err(e) = pty_resize.resize(&get_terminal_size().unwrap()) {
                        error!("Resize failed with {:?}", e);
                    }
                }
                signal::SIGTERM => {
                    if let Err(e) = delete_pipe(&name_copy) {
                        error!("Unable to delete pipe {:?}", e);
                    }
                    std::process::exit(0);
                }
                _ => {}
            }
        };
        loop {
            select! {recv(signal) -> signal_value => handle_signal(signal_value.unwrap()),
            }
        }
    });
    if let Some(program) = program {
        let cmd = format!("{}\n", program);
        cmd_sender.send(Vec::from(cmd))?;
    }
    //Read commands from pipe and push it to the channel
    spawn_with_name("ReadCmdsRemote", move || {
        read_comands_from_pipe(cmd_sender, &name)
    });

    if let Err(e) = handle.join() {
        panic::resume_unwind(e)
    }

    if let Err(e) = delete_pipe(&name_copy2) {
        error!("Unable to delete pipe {:?}", e);
    }
    Ok(())
}

fn read_comands_from_pipe(cmd_sender: Sender<Vec<u8>>, name: &str) -> Result<()> {
    debug!("read_comands");
    let mut input = get_pipe(name, false)?;
    let mut data = [0; 256];
    loop {
        match input.read(&mut data) {
            Ok(count) => {
                if count == 0 {
                    let ten_millis = Duration::from_millis(1);
                    thread::sleep(ten_millis);
                }
                debug!("count {}", count);
                let (sub_slice, _) = data.split_at(count);
                if let Err(err) = cmd_sender.send(Vec::from(sub_slice)) {
                    error!("{}", err);
                }
                data.fill(0);
            }
            Err(err) => error!("error {}", err),
        }
    }
}

//Pass all cmds to the terminal
fn handle_slave_output(
    cmd_receiver: Receiver<Vec<u8>>,
    val_receiver: Receiver<Vec<u8>>,
    mut pty_output: File,
) -> std::io::Result<()> {
    loop {
        if let Ok(val) = cmd_receiver.try_recv() {
            pty_output.write_all(val.as_slice())?;
            pty_output.flush()?;
        }

        if let Ok(val) = val_receiver.try_recv() {
            pty_output.write_all(val.as_slice())?;
            pty_output.flush()?;
        }
    }
}

/// Sends the content of input into output
fn pipe(input: &mut File, output: &mut File) -> Result<()> {
    let mut packet = [0; 4096];

    let count = input.read(&mut packet)?;

    let read = &packet[..count];
    output.write_all(read)?;
    output.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn read_write_pipe() {
        use flexi_logger::*;
        Logger::try_with_str("info")
            .unwrap()
            .log_to_file(FileSpec::default()) // write logs to file
            .duplicate_to_stderr(Duplicate::Info) // print warnings and errors also to the console
            .format(with_thread)
            .start()
            .unwrap();

        let t = spawn_with_name("Reader", move || {
            let mut output = get_pipe("test", false).unwrap();
            let mut data = [0; 256];
            match output.read(&mut data) {
                Ok(count) => {
                    assert_eq!(count, 5);
                    let mut str = String::from_utf8(data.into()).unwrap();
                    str.truncate(count);
                    assert_eq!(str, "12345");
                }
                Err(err) => error!("{}", err),
            }
        });
        let ten_millis = Duration::from_millis(10);
        thread::sleep(ten_millis);
        let mut pipe = get_pipe("test", true).unwrap();
        pipe.write("12345".as_bytes()).unwrap();
        t.join().unwrap();
    }
}
