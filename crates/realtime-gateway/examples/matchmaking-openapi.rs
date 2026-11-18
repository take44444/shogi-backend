fn main() {
    println!("{}", realtime_gateway::openapi().to_pretty_json().unwrap());
}
