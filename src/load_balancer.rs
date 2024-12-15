use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::{Arc, RwLock, atomic::{AtomicUsize, Ordering}};
use std::thread;
use std::time::Duration;

#[derive(Clone)]
pub struct LoadBalancer {
    pub backend_addresses: Arc<RwLock<Vec<String>>>,  // RwLock for better read performance
    pub healthy_backend_addresses: Arc<RwLock<Vec<String>>>,
    pub health_check_interval: Duration,
    pub health_check_url: String,
    pub current_index: Arc<AtomicUsize>,  // AtomicUsize for better performance on the round-robin index
}

impl LoadBalancer {
    pub fn new(
        backend_addresses: Vec<String>,
        health_check_interval: u64,
        health_check_url: String,
    ) -> Self {
        LoadBalancer {
            backend_addresses: Arc::new(RwLock::new(backend_addresses)),
            healthy_backend_addresses: Arc::new(RwLock::new(vec![])),
            health_check_interval: Duration::from_secs(health_check_interval),
            health_check_url,
            current_index: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn try_backend(&self, client_stream: &mut TcpStream, request: &[u8]) -> bool {
        let healthy_backends = self.healthy_backend_addresses.read().unwrap();

        if healthy_backends.is_empty() {
            return false;
        }

        let available_backends: Vec<String> = healthy_backends.clone();  // Clone the list to avoid holding the lock

        for _ in 0..available_backends.len() {
            if let Some(backend_addr) = self.get_backend() {
                match self.forward_request_to_backend(client_stream, request, &backend_addr) {
                    Ok(_) => return true,
                    Err(_) => println!("Backend {} failed, trying another one", backend_addr),
                }
            }
        }

        false
    }

    fn forward_request_to_backend(
        &self,
        client_stream: &mut TcpStream,
        request: &[u8],
        backend_addr: &str,
    ) -> Result<(), ()> {
        println!("Trying to forward request to backend: {}", backend_addr);
        match TcpStream::connect(backend_addr) {
            Ok(mut backend_stream) => {
                backend_stream.write_all(request).map_err(|_| ())?;

                let mut backend_response = Vec::new();
                backend_stream.read_to_end(&mut backend_response).map_err(|_| ())?;

                let response_header = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\n";
                client_stream.write_all(response_header).map_err(|_| ())?;
                client_stream.write_all(&backend_response).map_err(|_| ())?;

                println!("Successfully forwarded the response to the client.");
                Ok(())
            }
            Err(_) => {
                eprintln!("Failed to connect to backend: {}", backend_addr);
                Err(())
            }
        }
    }

    pub fn start_health_check(&self) {
        let backend_addresses = self.backend_addresses.clone();
        let healthy_backend_addresses = self.healthy_backend_addresses.clone();
        let health_check_url = self.health_check_url.clone();
        let health_check_interval = self.health_check_interval;

        thread::spawn(move || {
            // Perform the first immediate health check
            let mut healthy_servers = Vec::new();

            {
                let backend_addresses = backend_addresses.read().unwrap();
                for backend in backend_addresses.iter() {
                    if Self::check_health(backend, &health_check_url) {
                        healthy_servers.push(backend.clone());
                        println!("Backend {} is healthy", backend);
                    } else {
                        eprintln!("Backend {} is unhealthy", backend);
                    }
                }
            }

            // Update healthy backends immediately after the first check
            {
                let mut healthy_backend_addresses = healthy_backend_addresses.write().unwrap();
                *healthy_backend_addresses = healthy_servers;
            }

            // Then, continue with periodic health checks
            loop {
                thread::sleep(health_check_interval);

                let mut healthy_servers = Vec::new();

                {
                    let backend_addresses = backend_addresses.read().unwrap();
                    for backend in backend_addresses.iter() {
                        if Self::check_health(backend, &health_check_url) {
                            healthy_servers.push(backend.clone());
                            println!("Backend {} is healthy", backend);
                        } else {
                            eprintln!("Backend {} is unhealthy", backend);
                        }
                    }
                }

                {
                    let mut healthy_backend_addresses = healthy_backend_addresses.write().unwrap();
                    *healthy_backend_addresses = healthy_servers;
                }

                println!("Updated healthy backend servers: {:?}", healthy_backend_addresses);
            }
        });
    }


    fn check_health(backend_addr: &str, health_check_url: &str) -> bool {
        let url = format!("{}{}", backend_addr, health_check_url);
        println!("Checking health for: {}", url);

        match TcpStream::connect(backend_addr) {
            Ok(mut stream) => {
                let request = format!(
                    "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
                    health_check_url,
                    backend_addr,
                );
                if let Err(_) = stream.write_all(request.as_bytes()) {
                    eprintln!("Error sending health check request to {}", backend_addr);
                    return false;
                }

                let mut response = Vec::new();
                if let Err(_) = stream.read_to_end(&mut response) {
                    eprintln!("Error reading health check response from {}", backend_addr);
                    return false;
                }

                let response_str = match std::str::from_utf8(&response) {
                    Ok(s) => s,
                    Err(_) => return false,
                };

                if response_str.contains("HTTP/1.1 200 OK") {
                    println!("Backend {} is healthy", backend_addr);
                    true
                } else {
                    eprintln!("Unhealthy response from backend: {}", backend_addr);
                    false
                }
            }
            Err(_) => {
                eprintln!("Failed to connect to backend: {}", backend_addr);
                false
            }
        }
    }

    pub fn get_backend(&self) -> Option<String> {
        let healthy_backend_addresses = self.healthy_backend_addresses.read().unwrap();

        if healthy_backend_addresses.is_empty() {
            return None;
        }

        let current_index = self.current_index.load(Ordering::SeqCst);
        let backend = healthy_backend_addresses[current_index % healthy_backend_addresses.len()].clone();

        self.current_index
            .store((current_index + 1) % healthy_backend_addresses.len(), Ordering::SeqCst);

        Some(backend)
    }
}
