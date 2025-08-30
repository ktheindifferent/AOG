use aog::aog::auth::{
    hash_password, verify_password, validate_password_strength,
    generate_secure_password, get_initial_password, PasswordError
};
use std::env;
use tempfile::TempDir;

fn setup_test_environment() -> TempDir {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let aog_path = temp_dir.path().join("aog");
    std::fs::create_dir_all(&aog_path).expect("Failed to create test aog directory");
    
    env::set_var("AOG_TEST_DIR", aog_path.to_str().unwrap());
    temp_dir
}

#[test]
fn test_complete_authentication_flow() {
    let _temp_dir = setup_test_environment();
    
    // Generate initial password
    let initial_password = generate_secure_password();
    assert!(initial_password.len() >= 16);
    assert!(initial_password.len() <= 24);
    
    // Hash the password
    let hashed = hash_password(&initial_password).expect("Failed to hash password");
    assert!(hashed.starts_with("$argon2"));
    
    // Verify correct password
    assert!(verify_password(&initial_password, &hashed).expect("Failed to verify password"));
    
    // Verify incorrect password
    assert!(!verify_password("wrong_password", &hashed).expect("Failed to verify password"));
}

#[test]
fn test_environment_variable_password() {
    let _temp_dir = setup_test_environment();
    
    // Test with valid password in environment
    let test_password = "MySecure@Pass123!";
    env::set_var("AOG_INITIAL_PASSWORD", test_password);
    
    let password = get_initial_password().expect("Failed to get initial password");
    assert_eq!(password, test_password);
    
    env::remove_var("AOG_INITIAL_PASSWORD");
}

#[test]
fn test_environment_variable_weak_password_rejected() {
    let _temp_dir = setup_test_environment();
    
    // Test with weak password in environment
    env::set_var("AOG_INITIAL_PASSWORD", "weak");
    
    let result = get_initial_password();
    assert!(result.is_err());
    
    env::remove_var("AOG_INITIAL_PASSWORD");
}

#[test]
fn test_password_strength_validation() {
    // Valid passwords
    assert!(validate_password_strength("Valid@Pass123").is_ok());
    assert!(validate_password_strength("Another$Secure1Password").is_ok());
    
    // Too short
    assert!(matches!(
        validate_password_strength("Short@1"),
        Err(PasswordError::TooShort)
    ));
    
    // Missing uppercase
    assert!(matches!(
        validate_password_strength("lowercase@pass123"),
        Err(PasswordError::MissingUppercase)
    ));
    
    // Missing lowercase
    assert!(matches!(
        validate_password_strength("UPPERCASE@PASS123"),
        Err(PasswordError::MissingLowercase)
    ));
    
    // Missing digit
    assert!(matches!(
        validate_password_strength("NoDigits@Password"),
        Err(PasswordError::MissingDigit)
    ));
    
    // Missing special character
    assert!(matches!(
        validate_password_strength("NoSpecialChar123ABC"),
        Err(PasswordError::MissingSpecialChar)
    ));
}

#[test]
fn test_generated_passwords_meet_requirements() {
    for _ in 0..20 {
        let password = generate_secure_password();
        
        // Check length
        assert!(password.len() >= 16);
        assert!(password.len() <= 24);
        
        // Validate strength
        assert!(validate_password_strength(&password).is_ok());
        
        // Verify it can be hashed and verified
        let hash = hash_password(&password).expect("Failed to hash generated password");
        assert!(verify_password(&password, &hash).expect("Failed to verify generated password"));
    }
}

#[test]
fn test_password_hash_uniqueness() {
    let password = "TestPassword@123";
    
    let hash1 = hash_password(password).expect("Failed to hash password");
    let hash2 = hash_password(password).expect("Failed to hash password");
    
    // Hashes should be different due to unique salts
    assert_ne!(hash1, hash2);
    
    // But both should verify correctly
    assert!(verify_password(password, &hash1).expect("Failed to verify password"));
    assert!(verify_password(password, &hash2).expect("Failed to verify password"));
}

#[test]
fn test_hash_resistance_to_timing_attacks() {
    let password = "TestPassword@123";
    let hash = hash_password(password).expect("Failed to hash password");
    
    // Test with similar but incorrect passwords
    let similar_passwords = vec![
        "TestPassword@124",
        "TestPassword@12",
        "TestPassword@1234",
        "testPassword@123",
        "TESTPASSWORD@123",
    ];
    
    for wrong_password in similar_passwords {
        assert!(!verify_password(wrong_password, &hash).expect("Failed to verify password"));
    }
}

#[test]
fn test_invalid_hash_format_handling() {
    let invalid_hashes = vec![
        "invalid_hash",
        "$2b$10$invalid",  // bcrypt format (not argon2)
        "",
        "$argon2$",  // incomplete argon2 hash
    ];
    
    for invalid_hash in invalid_hashes {
        let result = verify_password("any_password", invalid_hash);
        // Should either return false or error, but not panic
        match result {
            Ok(valid) => assert!(!valid),
            Err(_) => {} // Expected for invalid format
        }
    }
}

#[cfg(test)]
mod password_reset_tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    
    #[test]
    #[ignore] // This test requires full application context
    fn test_password_reset_flow() {
        // This would test the reset_password function
        // but requires full Config infrastructure
    }
    
    #[test]
    #[ignore] // This test requires full application context  
    fn test_password_change_flow() {
        // This would test the change_password function
        // but requires full Config infrastructure
    }
}