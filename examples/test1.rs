use std::collections::HashMap;
extern crate burst;
use futures::executor; 

use burst::{MachineSetup, BurstBuilder, Machine};


async fn test1() -> Result<(), Box<dyn std::error::Error>> {
    let mut b = BurstBuilder::default();
    b.use_term_logger();
    b.add_set(
        "server", 
        1, 
        MachineSetup::new("t3.small", "ami-e18aa89b", |sess| {
            sess.cmd("cat /etc/hostname").map(|out| {
                println!("{}", out);
            })
        })
    );
    
    b.add_set(
        "client",
        3,
        MachineSetup::new("t3.small", "ami-e18aa89b", |sess| {
            sess.cmd("date").map(|out| {
                println!("{}", out);
            })
        })
    );

    let future = b.run(|_vms: HashMap<String, Vec<Machine>>| {
        println!("==> {}",_vms["server"][0].private_ip);
        for c in &_vms["client"] {
            println!(" -> {}",c.private_ip);
        }
        Ok(())
    });

    Ok(future?)
}


fn main() {
    match executor::block_on(test1()) {
        Ok(()) => println!("test1() succeeded"),
        Err(e) => println!("test1() failed: {}", e),
    }

}


