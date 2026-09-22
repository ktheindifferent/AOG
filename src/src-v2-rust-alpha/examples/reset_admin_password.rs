//! Reset the existing admin password without initializing hardware.
//! Run: sudo target/debug/examples/reset_admin_password
fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !std::path::Path::new("/opt/aog/data.json").is_file() {
        return Err("No AOG installation found at /opt/aog/data.json".into());
    }
    let password = aog::aog::auth::reset_password()?;
    println!("Username: admin");
    println!("New password: {}", password);
    println!("Restart AOG to load the new password.");
    Ok(())
}
