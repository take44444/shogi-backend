fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", player_http::openapi().to_pretty_json()?);
    Ok(())
}
