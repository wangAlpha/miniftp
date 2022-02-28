// mod miniftp;
// use miniftp::ThreadPool;
use miniftp::*;
use std::io::prelude::*;
use std::net::TcpListener;
use std::net::TcpStream;
use std::thread;
use std::{fs, time::Duration};

fn main() {
    // if let Some(ref opt) = env::args().nth(1) {
    //     if opt == "-c" {
    //         // let client = miniftp::Client;
    //         println!("Starting miniftp shell.");
    //         // client.shell_loop();
    //     } else {
    //         // println!(help)
    //     }
    // } else {
    //     run_server();
    // }
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    // cpu num
    let pool = ThreadPool::new(4);
    for stream in listener.incoming().take(2) {
        let stream = stream.unwrap();
        pool.execute(|| {
            handle_connection(stream);
        });
    }
    println!("Shutting down.");
}

fn handle_connection(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).unwrap();

    let get = b"GET / HTTP/1.1\r\n";
    let sleep = b"GET /sleep HTTP/1.1\r\n";

    let (status_line, filename) = if buffer.starts_with(get) {
        ("HTTP/1.1 200 OK", "hello.html")
    } else if buffer.starts_with(sleep) {
        thread::sleep(Duration::from_secs(5));
        ("HTTP/1.1 200 OK", "hello.html")
    } else {
        ("HTTP/1.1 404 NOT FOUND", "404.html")
    };

    let contents = fs::read_to_string(filename).unwrap();

    let response = format!(
        "{}\r\nContent-Length: {}\r\n\r\n{}",
        status_line,
        contents.len(),
        contents
    );

    stream.write(response.as_bytes()).unwrap();
    stream.flush().unwrap();
}
