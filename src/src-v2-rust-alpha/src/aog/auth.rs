use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use rand::{distributions::Alphanumeric, Rng, thread_rng};
use std::error::Error;

const MIN_PASSWORD_LENGTH: usize = 12;
const MAX_PASSWORD_LENGTH: usize = 128;

#[derive(Debug)]
pub enum PasswordError {
    TooShort,
    TooLong,
    MissingUppercase,
    MissingLowercase,
    MissingDigit,
    MissingSpecialChar,
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PasswordError::TooShort => write!(f, "Password must be at least {} characters", MIN_PASSWORD_LENGTH),
            PasswordError::TooLong => write!(f, "Password must be no more than {} characters", MAX_PASSWORD_LENGTH),
            PasswordError::MissingUppercase => write!(f, "Password must contain at least one uppercase letter"),
            PasswordError::MissingLowercase => write!(f, "Password must contain at least one lowercase letter"),
            PasswordError::MissingDigit => write!(f, "Password must contain at least one digit"),
            PasswordError::MissingSpecialChar => write!(f, "Password must contain at least one special character"),
        }
    }
}

impl Error for PasswordError {}

pub fn hash_password(password: &str) -> Result<String, Box<dyn Error>> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    
    let password_hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Failed to hash password: {}", e))?;
    
    Ok(password_hash.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, Box<dyn Error>> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| format!("Invalid password hash format: {}", e))?;
    
    let argon2 = Argon2::default();
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub fn validate_password_strength(password: &str) -> Result<(), PasswordError> {
    if password.len() < MIN_PASSWORD_LENGTH {
        return Err(PasswordError::TooShort);
    }
    
    if password.len() > MAX_PASSWORD_LENGTH {
        return Err(PasswordError::TooLong);
    }
    
    let has_uppercase = password.chars().any(|c| c.is_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_lowercase());
    let has_digit = password.chars().any(|c| c.is_digit(10));
    let has_special = password.chars().any(|c| !c.is_alphanumeric());
    
    if !has_uppercase {
        return Err(PasswordError::MissingUppercase);
    }
    
    if !has_lowercase {
        return Err(PasswordError::MissingLowercase);
    }
    
    if !has_digit {
        return Err(PasswordError::MissingDigit);
    }
    
    if !has_special {
        return Err(PasswordError::MissingSpecialChar);
    }
    
    Ok(())
}

pub fn generate_secure_password() -> String {
    let mut rng = thread_rng();
    let length = rng.gen_range(16..=24);
    
    let uppercase: Vec<char> = "ABCDEFGHIJKLMNOPQRSTUVWXYZ".chars().collect();
    let lowercase: Vec<char> = "abcdefghijklmnopqrstuvwxyz".chars().collect();
    let digits: Vec<char> = "0123456789".chars().collect();
    let special: Vec<char> = "!@#$%^&*()_+-=[]{}|;:,.<>?".chars().collect();
    
    let mut password = String::new();
    
    password.push(uppercase[rng.gen_range(0..uppercase.len())]);
    password.push(lowercase[rng.gen_range(0..lowercase.len())]);
    password.push(digits[rng.gen_range(0..digits.len())]);
    password.push(special[rng.gen_range(0..special.len())]);
    
    let all_chars: Vec<char> = uppercase.iter()
        .chain(lowercase.iter())
        .chain(digits.iter())
        .chain(special.iter())
        .cloned()
        .collect();
    
    for _ in 4..length {
        password.push(all_chars[rng.gen_range(0..all_chars.len())]);
    }
    
    let mut chars: Vec<char> = password.chars().collect();
    use rand::seq::SliceRandom;
    chars.shuffle(&mut rng);
    
    chars.into_iter().collect()
}

pub fn get_initial_password() -> Result<String, Box<dyn Error>> {
    if let Ok(password) = std::env::var("AOG_INITIAL_PASSWORD") {
        validate_password_strength(&password)?;
        Ok(password)
    } else {
        Ok(generate_secure_password())
    }
}

pub fn reset_password() -> Result<String, Box<dyn Error>> {
    let new_password = generate_secure_password();
    
    // Load existing config
    let mut config = crate::Config::load(0)?;
    
    // Hash the new password
    config.encrypted_password = hash_password(&new_password)?;
    
    // Save the updated config
    config.save()?;
    
    Ok(new_password)
}

pub fn change_password(current_password: &str, new_password: &str) -> Result<(), Box<dyn Error>> {
    // Load existing config
    let mut config = crate::Config::load(0)?;
    
    // Verify current password
    if !verify_password(current_password, &config.encrypted_password)? {
        return Err("Current password is incorrect".into());
    }
    
    // Validate new password strength
    validate_password_strength(new_password)?;
    
    // Hash and save new password
    config.encrypted_password = hash_password(new_password)?;
    config.save()?;
    
    Ok(())
}

pub fn generate_api_token() -> String {
    let mut rng = thread_rng();
    let token: String = (0..32)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect();
    token
}

pub fn set_api_token() -> Result<String, Box<dyn Error>> {
    let token = generate_api_token();
    
    // Load existing config
    let mut config = crate::Config::load(0)?;
    
    // Set the new token
    config.command_api_token = Some(token.clone());
    
    // Save the updated config
    config.save()?;
    
    log::info!("New Command API token generated and saved");
    
    Ok(token)
}

pub fn remove_api_token() -> Result<(), Box<dyn Error>> {
    // Load existing config
    let mut config = crate::Config::load(0)?;
    
    // Remove the token
    config.command_api_token = None;
    
    // Save the updated config
    config.save()?;
    
    log::info!("Command API token removed");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing_and_verification() {
        let password = "Test@Password123!";
        let hash = hash_password(password).expect("Failed to hash password");
        
        assert!(verify_password(password, &hash).expect("Failed to verify password"));
        assert!(!verify_password("WrongPassword", &hash).expect("Failed to verify password"));
    }

    #[test]
    fn test_password_validation() {
        assert!(validate_password_strength("Test@Pass123").is_ok());
        
        assert!(matches!(
            validate_password_strength("short"),
            Err(PasswordError::TooShort)
        ));
        
        assert!(matches!(
            validate_password_strength("nouppercasepass123!"),
            Err(PasswordError::MissingUppercase)
        ));
        
        assert!(matches!(
            validate_password_strength("NOLOWERCASEPASS123!"),
            Err(PasswordError::MissingLowercase)
        ));
        
        assert!(matches!(
            validate_password_strength("NoDigitsPass!"),
            Err(PasswordError::MissingDigit)
        ));
        
        assert!(matches!(
            validate_password_strength("NoSpecialChar123"),
            Err(PasswordError::MissingSpecialChar)
        ));
    }

    #[test]
    fn test_secure_password_generation() {
        for _ in 0..10 {
            let password = generate_secure_password();
            assert!(password.len() >= 16 && password.len() <= 24);
            assert!(validate_password_strength(&password).is_ok());
        }
    }

    #[test]
    fn test_generated_passwords_are_unique() {
        let passwords: Vec<String> = (0..100)
            .map(|_| generate_secure_password())
            .collect();
        
        let unique_count = passwords.iter().collect::<std::collections::HashSet<_>>().len();
        assert_eq!(unique_count, passwords.len());
    }

    #[test]
    fn test_hash_format_is_valid() {
        let password = "Test@Password123!";
        let hash = hash_password(password).expect("Failed to hash password");
        
        assert!(hash.starts_with("$argon2"));
        assert!(hash.contains('$'));
    }
}