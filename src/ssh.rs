use ssh2;
use std::{net::{ TcpStream, SocketAddr}, thread};
use failure::{Error};
use std::path::Path;
use std::time::{Instant, Duration};
use failure::ResultExt;

pub struct Session {
    ssh: ssh2::Session,
    _stream: TcpStream
}

impl Session  {
    pub(crate) fn connect(addr: SocketAddr, key: &Path) -> Result<Self, Error> {
        
        let start = Instant::now();

        let tcp = loop { 
            match TcpStream::connect_timeout(&addr, Duration::from_secs(3)) {
                Ok(s) => break s,
                Err(_) if start.elapsed() <= Duration::from_secs(60)=> {
                    thread::sleep(Duration::from_secs(1));
                }, 
                Err(e) => Err(e).context("falied to connect to ssh port")?,
            }
        };
        
        let mut sess = ssh2::Session::new()
        .context("libssh2 not available")?;
    
        let cloned_tcp = tcp.try_clone().unwrap();
        sess.set_tcp_stream(cloned_tcp);
        sess.handshake()
            .context("failed to perform ssh handshake")?;

        // ssh using the private key saved in temporary file, generated programmatically
        sess.userauth_pubkey_file("ec2-user", None, key, None)
            .context("failed to authenticate ssh session")?;
         
        Ok(Session{
            ssh: sess,
            _stream: tcp
        })
    }

    pub fn cmd(&mut self, cmd: &str) -> Result<String, Error> {
        use std::io::Read;
        
        let mut channel = self.ssh
            .channel_session()
            .context(format!("failed to create ssh channel for command '{}'", cmd))?;
        
        channel.exec(cmd)
                .context(format!("failed to execute command '{}'", cmd))?;
        
        let mut s = String::new(); 
        
        channel.read_to_string(&mut s)
                .context(format!("failed to read results of command '{}'", cmd))?;

        
        channel.wait_close()
            .context(format!("command '{}' never compeleted", cmd))?;
    
        Ok(s) 
    }
}

use std::ops::{Deref, DerefMut};
impl Deref for Session {
    type Target = ssh2::Session;

    fn deref(&self) -> &Self::Target {
        &self.ssh
    }
}

impl DerefMut for Session {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ssh
    }
}

