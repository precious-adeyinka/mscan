use std::net::TcpListener;

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8000").expect("could not bind");
    println!("Listening on 127.0.0.1:8000");
    for stream in listener.incoming() {
        println!("{:?}", stream);
        let _ = stream;
    }
}
