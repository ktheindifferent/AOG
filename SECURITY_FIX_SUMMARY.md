# Security Fix: File Permissions Vulnerability

## Summary
Fixed critical security vulnerability where the AOG system was setting world-writable (777) permissions on all files, allowing any user to modify critical system files.

## Changes Made

### 1. Fixed Permission Settings (`src/src-v2-rust-alpha/src/aog/tools.rs`)
- **Removed**: Dangerous `chmod 777` that made all files world-writable
- **Added**: Secure permission settings:
  - Directories: `755` (owner: rwx, group: r-x, others: r-x)
  - Config files: `644` (owner: rw-, group: r--, others: r--)
  - Log files: `664` (owner: rw-, group: rw-, others: r--)
  - Executables: `755` (owner: rwx, group: r-x, others: r-x)

### 2. New Security Functions (`src/src-v2-rust-alpha/src/aog/tools.rs`)
- `set_ownership()`: Sets proper user/group ownership on files
- `validate_permissions()`: Validates that files are not world-writable
- Enhanced `fix_permissions()`: Intelligently sets permissions based on file type

### 3. User/Group Creation (`src/src-v2-rust-alpha/src/setup.rs`)
- `create_aog_user_and_group()`: Creates dedicated `aog` user and group
  - System user with no shell access (`/usr/sbin/nologin`)
  - Home directory set to `/opt/aog`
  - Adds sudo user to `aog` group for management

### 4. Startup Security Validation (`src/src-v2-rust-alpha/src/main.rs`)
- `validate_startup_permissions()`: Checks all critical files on startup
- `security_audit_log()`: Logs security events to `/opt/aog/security_audit.log`
- Automatically fixes insecure permissions when detected

### 5. Systemd Service Hardening (`src/src-v2-rust-alpha/src/setup.rs`)
- Service runs as dedicated `aog` user (not root)
- Added security hardening options:
  - `PrivateTmp=true`: Isolated temp directory
  - `NoNewPrivileges=true`: Prevents privilege escalation
  - `ProtectSystem=strict`: Read-only system directories
  - `ProtectHome=true`: No access to user home directories
  - `ReadWritePaths=/opt/aog`: Only AOG directory is writable

## Security Improvements

### Before
- All files had `777` permissions (world-writable)
- Service ran as root
- No permission validation
- No security audit logging
- Any user could modify critical files

### After
- Secure permissions based on file type
- Dedicated service user with minimal privileges
- Automatic permission validation on startup
- Security audit logging for all permission events
- Only authorized users can modify files

## Testing
Run the included test script to verify the security fixes:
```bash
./test_permissions.sh
```

All tests should pass, confirming:
- No `777` permissions in code
- Security functions are present
- User/group creation logic exists
- Systemd service is hardened
- Proper permissions are set

## Deployment Notes

1. **User Creation**: The system will automatically create the `aog` user and group on first run
2. **Permission Migration**: Existing installations will have permissions automatically fixed on startup
3. **Service Restart**: After update, restart the service: `sudo systemctl restart aog.service`
4. **Audit Log**: Monitor `/opt/aog/security_audit.log` for security events

## Security Best Practices Applied

1. **Principle of Least Privilege**: Service runs with minimal required permissions
2. **Defense in Depth**: Multiple layers of security (permissions, ownership, systemd hardening)
3. **Audit Trail**: All security events are logged
4. **Automatic Remediation**: Insecure permissions are automatically fixed
5. **Separation of Duties**: Dedicated service user separate from system users

## Compliance
These changes align with common security standards:
- CIS Benchmarks for Linux
- NIST security guidelines
- OWASP best practices for file permissions

## Impact
- **Security**: Significantly reduced attack surface
- **Stability**: No impact on functionality
- **Performance**: Minimal overhead from permission checks
- **Compatibility**: Backward compatible with existing installations