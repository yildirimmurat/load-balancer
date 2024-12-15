use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct LoadBalancer {
    pub backend_addresses: Arc<Mutex<Vec<String>>>,
    pub health_check_interval: Duration,
}

impl LoadBalancer {
    pub fn new(backend_addresses: Vec<String>, health_check_interval: u64) -> Self {
        LoadBalancer {
            backend_addresses: Arc::new(Mutex::new(backend_addresses)),
            health_check_interval: Duration::from_secs(health_check_interval),
        }
    }

    pub fn start_health_check(&self) {
        let backend_addresses = self.backend_addresses.clone();
        let health_check_interval = self.health_check_interval;

        thread::spawn(move || loop {
            thread::sleep(health_check_interval);

            let mut healthy_servers = Vec::new();
            let mut backend_addresses = backend_addresses.lock().unwrap();

            for backend in backend_addresses.iter() {
                match Self::check_health(backend) {
                    true => healthy_servers.push(backend.clone()),
                    false => eprintln!("Server {} is unhealthy", backend),
                }
            }

            // Update the backend list with only healthy servers
            *backend_addresses = healthy_servers;
            println!("Healthy backend servers: {:?}", backend_addresses);
        });
    }

    fn check_health(backend_addr: &str) -> bool {
        // Try to connect to the backend address directly
        let mut stream = match TcpStream::connect(backend_addr) {
            Ok(stream) => stream,
            Err(_) => return false,
        };

        // Send a basic request
        let request = format!(
            "GET / HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            backend_addr,
        );

        if let Err(_) = stream.write_all(request.as_bytes()) {
            eprintln!("Error writing request to be stream");
            return false;
        }

        let mut response = Vec::new();
        if let Err(_) = stream.read_to_end(&mut response) {
            eprintln!("Error reading response from be stream");
            return false;
        }

        println!("Response from be: {:?}", response);


        // Check if we got a 200 OK response
        let response_str = match std::str::from_utf8(&response) {
            Ok(s) => s,
            Err(_) => return false,
        };

        println!("Response str from be: {}", response_str);
        response_str.contains("HTTP/1.1 200 OK")
    }

    // Get a round-robin backend address
    pub fn get_backend(&self) -> Option<String> {
        let backend_addresses = self.backend_addresses.lock().unwrap();
        if backend_addresses.is_empty() {
            return None;
        }

        // Simple round-robin logic, always return the first healthy backend
        Some(backend_addresses[0].clone())
    }
}