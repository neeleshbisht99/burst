>> To run Program
1. ENABLE SSH AGENT FOR DOING SSH 
2. run command: cargo run --example test1     


>> To do ssh manually:
1. ENABLE SSH AGENT FOR DOING SSH 
2. run command: ssh ec2-user@<ip>

>> AWS ACCESS 
export AWS_SECRET_ACCESS_KEY=****
export AWS_ACCESS_KEY_ID=*****

**SSH without a PEM file but using ssh-agent,** follow these steps:
1. **Start ssh-agent.** If it is not already running, start ssh-agent with the following command:
    `ssh-agent -s`
2. **Add your PEM key to the ssh-agent.** Use the following command to add your PEM key to the ssh-agent:
    `ssh-add <pem-key>`
3. **SSH to your remote server.** You can now SSH to your remote server without specifying your PEM key. For example, to SSH to the user `ubuntu` on the server with the IP address `192.168.1.100`, you would use the following command:
    `ssh ubuntu@192.168.1.100`