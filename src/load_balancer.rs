use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct LoadBalancer {
    pub backend_addresses: Arc<Mutex<Vec<String>>>,
    pub healthy_backend_addresses: Arc<Mutex<Vec<String>>>,
    pub health_check_interval: Duration,
    pub health_check_url: String,
    pub current_index: Arc<Mutex<usize>>,
}

impl LoadBalancer {
    pub fn new(
        backend_addresses: Vec<String>,
        health_check_interval: u64,
        health_check_url: String,
    ) -> Self {
        LoadBalancer {
            backend_addresses: Arc::new(Mutex::new(backend_addresses)),
            healthy_backend_addresses: Arc::new(Mutex::new(vec![])),
            health_check_interval: Duration::from_secs(health_check_interval),
            health_check_url,
            current_index: Arc::new(Mutex::new(0)),
        }
    }

    pub fn start_health_check(&self) {
        let backend_addresses = self.backend_addresses.clone();
        let healthy_backend_addresses = self.healthy_backend_addresses.clone();
        let health_check_url = self.health_check_url.clone();
        let health_check_interval = self.health_check_interval;

        thread::spawn(move || loop {
            thread::sleep(health_check_interval);

            let mut healthy_servers = Vec::new();
            let backend_addresses = backend_addresses.lock().unwrap();

            for backend in backend_addresses.iter() {
                match Self::check_health(backend, &health_check_url) {
                    true => healthy_servers.push(backend.clone()),
                    false => eprintln!("Server {} is unhealthy", backend),
                }
            }

            // Update the backend list with only healthy servers
            *healthy_backend_addresses.lock().unwrap() = healthy_servers.clone();
            println!("Healthy backend servers: {:?}", healthy_servers);
        });
    }

    fn check_health(backend_addr: &str, health_check_url: &str) -> bool {
        let url = format!("{}{}", backend_addr, health_check_url);
        println!("Checking url: {}", url);
        let mut stream = match TcpStream::connect(backend_addr) {
            Ok(stream) => stream,
            Err(_) => {
                return false;
            },
        };

        // Send a basic request to the health check URL
        let request = format!(
            "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
            health_check_url,
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

        // Look for "HTTP/1.1 200 OK" in the response
        if response_str.contains("HTTP/1.1 200 OK") {
            println!("Healthy response from backend: {}", backend_addr);
            true
        } else {
            eprintln!("Unhealthy response from backend: {}", response_str);
            false
        }
    }

    // Get a round-robin backend address
    pub fn get_backend(&self) -> Option<String> {
        let healthy_backend_addresses = self.healthy_backend_addresses.lock().unwrap();
        if healthy_backend_addresses.is_empty() {
            return None;
        }

        let mut current_index = self.current_index.lock().unwrap();

        let backend = healthy_backend_addresses[*current_index].clone();

        *current_index = (*current_index + 1) % healthy_backend_addresses.len();

        Some(backend)
    }
}