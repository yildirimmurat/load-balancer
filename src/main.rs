mod load_balancer;

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use load_balancer::LoadBalancer;

const LOAD_BALANCER_ADDR_PORT: &str = "127.0.0.1:8081";
const BACKEND_SERVER_ADDR_PORT_8082: &str = "127.0.0.1:8082";
const BACKEND_SERVER_ADDR_PORT_8083: &str = "127.0.0.1:8083";
const BACKEND_SERVER_ADDR_PORT_8084: &str = "127.0.0.1:8084";

fn handle_client(mut client_stream: TcpStream, load_balancer: Arc<LoadBalancer>) {
    let mut buffer = [0; 1024];

    match client_stream.read(&mut buffer) {
        Ok(n) if n > 0 => {
            let request = &buffer[..n];

            let load_balancer = load_balancer.clone();
            if !load_balancer.try_backend(&mut client_stream, request) {
                let error_message = b"HTTP/1.1 502 Bad Gateway\r\nAll backend servers are down";
                if let Err(e) = client_stream.write_all(error_message) {
                    eprintln!("Failed to send error to client: {}", e);
                }
            }
        }
        _ => eprintln!("Failed to read request or request was empty"),
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let health_check_interval = if args.len() > 1 {
        args[1].parse::<u64>().unwrap_or(10)
    } else {
        10
    };
    let health_check_url = if args.len() > 2 {
        args[2].clone()
    } else {
        "/health".to_string()
    };

    let backend_addresses = vec![
        BACKEND_SERVER_ADDR_PORT_8082.to_string(),
        BACKEND_SERVER_ADDR_PORT_8083.to_string(),
        BACKEND_SERVER_ADDR_PORT_8084.to_string(),
    ];

    let load_balancer = Arc::new(LoadBalancer::new(
        backend_addresses,
        health_check_interval,
        health_check_url,
    ));
    load_balancer.start_health_check();

    let listener = TcpListener::bind(LOAD_BALANCER_ADDR_PORT)
        .expect("Failed to bind to address");
    println!("Server listening on {}", LOAD_BALANCER_ADDR_PORT);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let load_balancer = load_balancer.clone();
                std::thread::spawn(move || handle_client(stream, load_balancer));
            }
            Err(e) => eprintln!("Failed to establish connection: {}", e),
        }
    }
}
