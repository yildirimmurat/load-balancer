mod load_balancer;

use std::env;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use load_balancer::LoadBalancer;
const LOAD_BALANCER_ADDR_PORT: &str = "127.0.0.1:8081";
const BACKEND_SERVER_ADDR_PORT_8082: &str = "127.0.0.1:8082";
const BACKEND_SERVER_ADDR_PORT_8083: &str = "127.0.0.1:8083";
const BACKEND_SERVER_ADDR_PORT_8084: &str = "127.0.0.1:8084";

fn handle_client(mut client_stream: TcpStream, backend_addr: &str) {
    let mut buffer = [0;1024];

    client_stream.read(&mut buffer).expect("Failed to read from client stream");
    let request = String::from_utf8_lossy(&buffer[..]);
    println!("Received request from client:\n{}", request);

    // Forward the request to the backend server
    match TcpStream::connect(backend_addr) {
        Ok(mut backend_stream) => {
            // Write the client request to the backend server
            backend_stream.write_all(&buffer).expect("Failed to write to backend server");

            // Read the response from the backend server
            let mut backend_response = Vec::new();
            backend_stream.read_to_end(&mut backend_response).expect("Failed to read from the backend server");

            // @todo: should be a part of backends response
            // Prepare HTTP headers
            let response_header = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\n\r\n";
            client_stream.write_all(response_header).expect("Failed to write response headers to client stream");

            // Send the backend's response back to the client
            client_stream.write_all(&backend_response).expect("Failed to write response to client stream");
            println!("Response sent back to client");
        },
        Err(e) => {
            eprintln!("Failed to connect to backend server: {}", e);
            // In case of backend failure, send an error message to the client
            let error_message = b"Error connecting to backend server\n";
            client_stream.write_all(error_message).expect("Failed to write error to client stream");
        }
    }
}

fn main() {
    // Parse command line arguments
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

    let load_balancer = LoadBalancer::new(
        backend_addresses,
        health_check_interval,
        health_check_url,
    );
    load_balancer.start_health_check();

    let listener = TcpListener::bind(LOAD_BALANCER_ADDR_PORT)
        .expect("Failed to bind to address");
    println!("Server listening on {}", LOAD_BALANCER_ADDR_PORT);

    // Handle incoming client requests
    for stream in listener.incoming() {
        let backend_addr = load_balancer.get_backend().unwrap_or_else(|| {
            eprintln!("No healthy backend servers available");
            String::new()
        });

        if !backend_addr.is_empty() {
            match stream {
                Ok(stream) => {
                    std::thread::spawn(move || handle_client(stream, &backend_addr));
                }
                Err(e) => {
                    eprintln!("Failed to establish connection: {}", e);
                }
            }
        }
    }
}
