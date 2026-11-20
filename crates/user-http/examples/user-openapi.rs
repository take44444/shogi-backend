fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_http::openapi().to_pretty_json()?);
    Ok(())
}
