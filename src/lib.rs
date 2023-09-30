use std::io;
extern crate rusoto_core;
extern crate rusoto_ec2;
extern crate ssh2;
extern crate rusoto_credential;
extern crate tokio;

use std::collections::HashMap;
use std::net::TcpStream;
use rusoto_core::RusotoError;
use ssh2::Session;
use rusoto_ec2::Ec2;

pub struct SshConnection;

mod ssh;

/*
 * Machine struct is used to store information about the spot instances which are running in AWS.
 * private_ip: priva te ip address of the ec2 machine
 * public_dns: dns of the ec2 machine
 */
pub struct Machine {
    pub ssh: Option<ssh::Session>,
    pub instance_type: String,
    pub private_ip: String,
    pub public_dns: String
}
 
/*
 * MachineSetup struct is used to stores description of the spot instances which will be launched in AWS.
 * it has following props: 
 * instance_type: possible type of ec2 machine available in aws
 * ami: possible machine images in aws
 * setup: A Box containing a trait object (Box<dyn Fn(&mut SshConnection) -> io::Result<()>>) that represents a function to set up the instance. This function takes a mutable reference to an SshConnection and returns an io::Result<()>.
 */
pub struct MachineSetup {
    instance_type: String,
    ami: String,
    setup: Box<dyn Fn(&mut ssh::Session) -> io::Result<()>>
}


 /* 
 * Follwing is the implementation of the new method for MachineSetup struct. 
 * It instantiates the MachineSetup with instance_type, ami and a setup method used to setup the machine when needed
 * The setup argument is a box containing a trait object which is a function to setup the instance.
 * THe trait bound for setup that is F, implies that the setup parameter must be a function or closure with a 'static lifetime
 * which means that the function/closure stored in the Box wil have lifetime of the program. 
 */
impl MachineSetup {
    pub fn new<F>(instance_type: &str, ami: &str, setup: F) -> Self
    where F: Fn(&mut ssh::Session) -> io::Result<()> + 'static,
    {
        MachineSetup {
            instance_type: instance_type.to_string(),
            ami: ami.to_string(),
            setup: Box::new(setup)
        }
    }
}

/***
 * Struct Builder is used for instantiating the burst library with the list of machine sets descibed in the descriptors.
 * Each "machine set" is identified with a unique name, and machine set has n number of machines in it.
 * A machine in the "machine set" is configured with MachineSetup 
 * The max_duration denotes the time till which ec2 spot instances will run before being terminated.
 */
pub struct BurstBuilder {
    descriptors: HashMap<String, (MachineSetup, u32)>,
    max_duration: i64,
}

/***
 * Default trait implementation of Burst Builder.
 * Helps in creating instances of BurstBuilder, instantiated with deafult values
 */
impl Default for BurstBuilder {
    fn default() -> Self {
        BurstBuilder {
            descriptors: Default::default(),
            max_duration: 60,
        }
    }
}

/*
 * Implementation block for Burst Builder.
 */
impl BurstBuilder {
    /*
     * The method "add_set" adds a new "machine set" to the burst builder struct by adding a entry to the 
     * descriptors field.
     */
    pub fn add_set(&mut self, name:&str, number: u32, description: MachineSetup) {
        // TODO : if name is already in use
        self.descriptors.insert(name.to_string(), (description, number));
    } 
    /*
     * The method "set_max_duration" modifies the max_duration attribute.
    */ 
    pub fn set_max_duration(&mut self, hours:u8) {
        self.max_duration = hours as i64 * 60;
    }

    /*
     * The method "add_set" adds a new "machine set" to the burst builder struct by adding a entry to the 
     * descriptors field.
    */ 
    #[tokio::main]
    pub async fn run<F>(self, f: F) -> Result<(), Box<dyn std::error::Error>>
    where F: FnOnce(HashMap<String, Vec<Machine>>) -> io::Result<()> 
    {
        //let provider = rusoto::EnvironmentProvider;
        use rusoto_core::{Region};
        use rusoto_credential::{EnvironmentProvider};
       
        /*
        * Here we create a Ec2Client object with a credentials provider and region etc  
        */
        // use rusoto_sts::{StsAssumeRoleSessionCredentialsProvider, StsClient};
        // default_tls_client().unwrap(), EnvironmentProvider, 
        let credentials_provider = EnvironmentProvider::default();
        let ec2 = rusoto_ec2::Ec2Client::new_with(
            rusoto_core::HttpClient::new().unwrap(),
            credentials_provider,
            Region::UsEast1);

        let mut setup_fns = HashMap::new();
        /*
        * Here we are calling requesting spot instances for all the machine sets and storing the request ids in spot_req_ids.
        */
        let mut id_to_name = HashMap::new();
        let mut spot_req_ids = Vec::new();
        for (name, (setup, number)) in self.descriptors {
            let mut launch = rusoto_ec2::RequestSpotLaunchSpecification::default();
            launch.image_id = Some(setup.ami);
            launch.instance_type =Some(setup.instance_type);
            setup_fns.insert(name.clone(), setup.setup);

            launch.security_groups = Some(vec!["test".to_string()]);
            launch.key_name = Some("burst-key-pair".to_string());

            let mut req = rusoto_ec2::RequestSpotInstancesRequest::default();          
            req.instance_count = Some(i64::from(number));
            // TODO
            // req.block_duration_minutes = Some(self.max_duration);
            req.launch_specification = Some(launch);
            let res = ec2.request_spot_instances(req).await?;
            if let Some(spot_instance_requests) = res.spot_instance_requests {
                // Handle spot_requests.
                spot_req_ids.extend(
                    spot_instance_requests.into_iter()
                    .filter_map(
                        |sir| sir.spot_instance_request_id
                    )
                    .map(|sir| {
                        id_to_name.insert(sir.clone(), name.clone());
                        sir
                    })
                );
            } else {
                // Handle the case when spot_instance_requests is None.
            }
        }

        /*
         * Following code iterates over all the ec2 requests and checks whether if any one of the request is in open state.
         * If anyone of them is in "open state", it loops over again and again
         * If none of them is in "open state", then it collects the instance ids and breaks from the loop
         */
        let mut req = rusoto_ec2::DescribeSpotInstanceRequestsRequest::default();
        req.spot_instance_request_ids = Some(spot_req_ids);
        let instances: Vec<_>;
        loop {
            let res: rusoto_ec2::DescribeSpotInstanceRequestsResult = ec2.describe_spot_instance_requests(req.clone()).await?;
            
            if let Some(spot_instance_requests) = res.spot_instance_requests {
                // Handle spot_requests.
                let all_ready = spot_instance_requests
                                        .iter()
                                        .all(|sir| sir.state.as_ref().unwrap() == "active");
            
                if all_ready {
                    instances = spot_instance_requests
                                    .into_iter()
                                    .filter_map(|sir| {
                                        let name = id_to_name.remove(&sir.spot_instance_request_id.unwrap()).unwrap();
                                        id_to_name.insert(sir.instance_id.as_ref().unwrap().clone(), name);
                                        sir.instance_id
                                    })
                                    .collect();
                    break;
                }
            } 
            else {
                
            }
        }

        /*
        * Here once all the ec2 spot instance requests are satified, the instances are now starting or runing.
        * The spot instance requests are cancelled, to ensure that if anyone of the instances stops, the spot instance requests are not called again.
        * All the requests happen once and all the instances are requested/started only once.
        */
        let mut cancel = rusoto_ec2::CancelSpotInstanceRequestsRequest::default();
        cancel.spot_instance_request_ids = req.spot_instance_request_ids.unwrap();
        ec2.cancel_spot_instance_requests(cancel).await?;


        /****
         * Here all the ec2 instances which are requested are iterated and checked where 
         * if all the requested ec2 machines are ready or not
         * it all not ready, then status of all the instances are requested again and checked
         * if all ready, then Machine structs are are populated with the config of the ec2 machines and stored in machines vector. 
         */
        let mut machines = HashMap::new();
        let mut desc_req: rusoto_ec2::DescribeInstancesRequest = rusoto_ec2::DescribeInstancesRequest::default();
        let mut all_ready = false;
        while !all_ready {
            machines.clear();
            all_ready = true;
            desc_req.instance_ids = Some(instances.clone());
            let res: rusoto_ec2::DescribeInstancesResult = ec2.describe_instances(desc_req.clone()).await?;
            if let Some(res_reservations) = res.reservations {
                for reservations in res_reservations.into_iter() {
                    for instance in reservations.instances.unwrap() {
                        match instance {
                            rusoto_ec2::Instance {
                                instance_id: Some(instance_id),
                                instance_type: Some(instance_type),
                                private_ip_address: Some(private_ip),
                                public_dns_name: Some(public_dns),
                                ..
                            } => {
                                let machine = Machine{
                                    ssh:None,
                                    instance_type,
                                    private_ip,
                                    public_dns
                                };
                                let name = id_to_name[&instance_id].clone();
                                machines.entry(name).or_insert_with(Vec::new).push(machine);
                            }
                            _=> { 
                                all_ready = false;
                            }
                        }
                    }
                }
                
            } else {
                // Handle the case when spot_instance_requests is None.
            }
            
        }
        // req.spot_instance_request_ids = 
        // ec 2.describe_spot_instance_requests(req)
        /***
         * Here for all the machines which are up and running,
         * one by one ssh connection is established to each of the remote ec2 machines  and certain commands are executed to verify if they are running properly.
         * First a tcp stream to the ssh server in the remote ec2 machine is established
         * then a ssh session is created using ssh2 crate, the tcp stream is associated with the sssh session, which will enabled the ssh session to connect to remote machine using tcp stream
         * finally a ssh handshake happens to initialize ssh session and it negotiates encryptin and other settings
         * finally authentication happens with ssh user agent authentication method 
         * Lastly a ssh channel is created for executing commands in the remote server and the output is also recieved & printed on local machine
         */
        for (name,machines) in &mut machines {          
            let f: &Box<dyn Fn(&mut ssh::Session) -> Result<(), io::Error>> = &setup_fns[name];  
            for machine in machines {
                let mut sess = ssh::Session::connect(&format!("{}:22", machine.public_dns)).unwrap();
                f(&mut sess).unwrap(); 
            } 
         
        }
        
        f(machines).unwrap();


        /***
         * Lastly ec2 remote instance termination request is executed  to stop all the instances started.
         */
        let mut termination_req = rusoto_ec2::TerminateInstancesRequest::default();
        termination_req.instance_ids = desc_req.instance_ids.unwrap();
        ec2.terminate_instances(termination_req).await?;

        Ok(())
    }
}

