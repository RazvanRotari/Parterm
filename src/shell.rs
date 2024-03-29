pub mod pty {
    //! pty low level handling

    use super::tui::Size;
    use super::util::FromLibcResult;
    use libc;
    use std::fs::File;
    use std::io::{self, Read, Write};
    use std::ops;
    use std::os::unix::io::{FromRawFd, RawFd};
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    use std::ptr;

    /// Master side of a pty master / slave pair.
    ///
    /// Allows reading the slave output, writing to the slave input and controlling the slave.
    pub struct Pty {
        /// File descriptor of the master side of the pty
        fd: RawFd,
        /// File built from fd to access Read and Write traits
        file: File,
    }

    /// Errors that might happen durring operations on pty.
    #[derive(Debug)]
    pub enum PtyError {
        /// Failed to open pty
        OpenPty,
        /// Failed spawn the shell
        SpawnShell,
        /// Failed to resize the pty
        Resize,
    }

    impl Pty {
        /// Spawns a child process running the given shell executable with the
        /// given size in a newly created pty.
        /// Returns a Pty representing the master side controlling the pty.
        pub fn spawn(shell: &str, size: &Size) -> Result<Pty, PtyError> {
            let (master, slave) = openpty(size)?;

            let mut cmd = Command::new(&shell);
            cmd.stdin(unsafe { Stdio::from_raw_fd(slave) })
                .stdout(unsafe { Stdio::from_raw_fd(slave) })
                .stderr(unsafe { Stdio::from_raw_fd(slave) });
            unsafe {
                cmd.pre_exec(before_exec);
            }
            cmd.spawn().map_err(|_| PtyError::SpawnShell).and_then(|_| {
                let pty = Pty {
                    fd: master,
                    file: unsafe { File::from_raw_fd(master) },
                };

                pty.resize(size)?;

                Ok(pty)
            })
        }

        /// Resizes the child pty.
        pub fn resize(&self, size: &Size) -> Result<(), PtyError> {
            unsafe {
                libc::ioctl(self.fd, libc::TIOCSWINSZ, &size.to_c_winsize())
                    .to_result()
                    .map(|_| ())
                    .map_err(|_| PtyError::Resize)
            }
        }
    }

    /// Creates a pty with the given size and returns the (master, slave)
    /// pair of file descriptors attached to it.
    fn openpty(size: &Size) -> Result<(RawFd, RawFd), PtyError> {
        let mut master = 0;
        let mut slave = 0;

        unsafe {
            // Create the pty master / slave pair
            libc::openpty(
                &mut master,
                &mut slave,
                ptr::null_mut(),
                ptr::null(),
                &size.to_c_winsize(),
            )
            .to_result()
            .map_err(|_| PtyError::OpenPty)?;

            // Configure master to be non blocking
            let current_config = libc::fcntl(master, libc::F_GETFL, 0)
                .to_result()
                .map_err(|_| PtyError::OpenPty)?;

            libc::fcntl(master, libc::F_SETFL, current_config)
                .to_result()
                .map_err(|_| PtyError::OpenPty)?;
        }

        Ok((master, slave))
    }

    /// Run between the fork and exec calls. So it runs in the cild process
    /// before the process is replaced by the program we want to run.
    fn before_exec() -> io::Result<()> {
        unsafe {
            // Create a new process group, this process being the master
            libc::setsid()
                .to_result()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, ""))?;

            // Set this process as the controling terminal
            libc::ioctl(0, libc::TIOCSCTTY, 1)
                .to_result()
                .map_err(|_| io::Error::new(io::ErrorKind::Other, ""))?;
        }

        Ok(())
    }

    impl Size {
        fn to_c_winsize(&self) -> libc::winsize {
            libc::winsize {
                ws_row: self.height,
                ws_col: self.width,

                // Unused fields in libc::winsize
                ws_xpixel: 0,
                ws_ypixel: 0,
            }
        }
    }

    impl Read for Pty {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            self.file.read(buf)
        }
    }

    impl Write for Pty {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.file.write(buf)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.file.flush()
        }
    }

    impl ops::Deref for Pty {
        type Target = File;

        fn deref(&self) -> &File {
            &self.file
        }
    }

    impl ops::DerefMut for Pty {
        fn deref_mut(&mut self) -> &mut File {
            &mut self.file
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::io::{Read, Write};

        #[test]
        #[ignore]
        fn can_open_a_shell_with_its_own_pty_and_can_read_and_write_to_its_master_side() {
            // Opening shell and its pty
            std::env::set_var("PS1", "+");
            let mut pty = Pty::spawn(
                "/bin/sh",
                &Size {
                    width: 100,
                    height: 100,
                },
            )
            .unwrap();

            let mut packet = [0; 4096];

            // Reading
            let count = pty.read(&mut packet).unwrap();
            let _output = String::from_utf8_lossy(&packet[..count]).to_string();

            // eprintln!("output: {}", output);
            // assert!(output.ends_with("$ "));

            // Writing and reading effect
            pty.write_all("exit\n".as_bytes()).unwrap();
            pty.flush().unwrap();

            let count = pty.read(&mut packet).unwrap();
            let output = String::from_utf8_lossy(&packet[..count]).to_string();
            eprintln!("output: {}", output);
            assert!(output.starts_with("exit"));
        }

        #[test]
        fn to_c_winsize_maps_width_to_col_height_to_row_and_sets_the_rest_to_0() {
            let expected = libc::winsize {
                ws_row: 42,
                ws_col: 314,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };

            let actual = Size {
                width: 314,
                height: 42,
            }
            .to_c_winsize();

            assert_eq!(expected.ws_row, actual.ws_row);
            assert_eq!(expected.ws_col, actual.ws_col);
            assert_eq!(expected.ws_xpixel, actual.ws_xpixel);
            assert_eq!(expected.ws_ypixel, actual.ws_ypixel);
        }
    }
}

pub mod tui {
    //! Terminal UI library

    use termion;

    /// A rectangular size in number of columns and rows
    pub struct Size {
        /// Number of columns
        pub width: u16,
        /// Number of rows
        pub height: u16,
    }

    /// Returns the terminal current size
    pub fn get_terminal_size() -> anyhow::Result<Size> {
        let (width, height) = termion::terminal_size()?;
        Ok(Size { width, height })
    }
}

pub mod util {
    //! Utilities

    use libc;
    use std::env;
    use std::ffi::CStr;

    /// The informations in /etc/passwd corresponding to the current user.
    struct Passwd {
        pub shell: String,
    }

    /// Return the informations in /etc/passwd corresponding to the current user.
    fn get_passwd() -> anyhow::Result<Passwd> {
        unsafe {
            let passwd = libc::getpwuid(libc::getuid()).to_result()?;

            let shell = CStr::from_ptr(passwd.pw_shell).to_str()?.to_string();

            Ok(Passwd { shell })
        }
    }

    /// Returns the path to the shell executable.
    ///
    /// Tries in order:
    ///
    /// * to read the SHELL env var
    /// * to read the user's passwd
    /// * defaults to /bin/sh
    ///
    /// # Example
    ///
    /// ```
    /// # use parterm::shell::util::get_shell;
    /// std::env::set_var("SHELL", "/bin/sh");
    /// let shell = get_shell();
    /// assert!(shell.contains("/bin/"));
    /// assert!(shell.contains("sh"));
    /// ```
    pub fn get_shell() -> String {
        env::var("SHELL")
            .or_else(|_| get_passwd().map(|passwd| passwd.shell))
            .unwrap_or_else(|_| "/bin/sh".to_string())
    }

    /// Converts a value returned by a libc function to a rust result.
    pub trait FromLibcResult: Sized {
        type Target;

        fn to_result(self) -> anyhow::Result<Self::Target>;
    }

    impl FromLibcResult for libc::c_int {
        type Target = libc::c_int;

        fn to_result(self) -> anyhow::Result<libc::c_int> {
            match self {
                -1 => anyhow::bail!("libc function failled"),
                res => Ok(res),
            }
        }
    }

    impl FromLibcResult for *mut libc::passwd {
        type Target = libc::passwd;

        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        fn to_result(self) -> anyhow::Result<libc::passwd> {
            if self.is_null() {
                anyhow::bail!("Fail to get the passwd")
            } else {
                unsafe {
                    let s = *self;
                    Ok(s)
                }
            }
        }
    }
}
