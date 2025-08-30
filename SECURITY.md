# AOG Security Configuration Guide

## Network Security

### Default Configuration (Secure)
By default, the AOG system binds to localhost (127.0.0.1) only, preventing remote access. This is the most secure configuration and recommended for most deployments.

- **HTTPS Admin Interface**: `https://127.0.0.1:8443`
- **Command API**: `https://127.0.0.1:9443`

### Configuring Network Bindings

The network binding addresses can be configured in `/opt/aog/data.json`:

```json
{
  ...
  "https_bind_address": "127.0.0.1",
  "https_bind_port": 8443,
  "command_api_bind_address": "127.0.0.1", 
  "command_api_bind_port": 9443
}
```

⚠️ **WARNING**: Changing the bind address from `127.0.0.1` to `0.0.0.0` or a specific network interface IP will expose the admin interface to the network. Only do this if you understand the security implications.

## Secure Remote Access Setup

### Option 1: SSH Tunnel (Recommended)
The most secure way to access AOG remotely is through SSH tunneling:

```bash
# From your local machine, create an SSH tunnel
ssh -L 8443:localhost:8443 -L 9443:localhost:9443 user@raspberry-pi-ip

# Now access the interface locally
# https://localhost:8443
```

### Option 2: Reverse Proxy with Authentication
If you need permanent remote access, use a reverse proxy with authentication:

1. **Install nginx**:
```bash
sudo apt-get install nginx
```

2. **Configure nginx with authentication**:
```nginx
server {
    listen 443 ssl;
    server_name your-domain.com;
    
    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;
    
    # Basic authentication
    auth_basic "AOG Admin";
    auth_basic_user_file /etc/nginx/.htpasswd;
    
    location / {
        proxy_pass https://127.0.0.1:8443;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
```

3. **Create password file**:
```bash
sudo htpasswd -c /etc/nginx/.htpasswd admin
```

### Option 3: VPN Access
For the highest security, set up a VPN server on your network and access AOG through the VPN connection.

## Firewall Configuration

### UFW (Uncomplicated Firewall) Setup
```bash
# Enable firewall
sudo ufw enable

# Allow SSH (if needed)
sudo ufw allow 22/tcp

# By default, block all incoming connections
sudo ufw default deny incoming
sudo ufw default allow outgoing

# If you must expose AOG (not recommended), limit access
# sudo ufw allow from 192.168.1.0/24 to any port 8443
```

### iptables Rules
```bash
# Block external access to AOG ports (default)
sudo iptables -A INPUT -p tcp --dport 8443 -s 127.0.0.1 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 8443 -j DROP
sudo iptables -A INPUT -p tcp --dport 9443 -s 127.0.0.1 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 9443 -j DROP

# Save rules
sudo iptables-save > /etc/iptables/rules.v4
```

## Security Best Practices

1. **Never expose AOG directly to the internet** without proper authentication and encryption
2. **Use strong passwords** for the admin interface
3. **Keep the system updated** with security patches
4. **Monitor access logs** regularly:
   ```bash
   tail -f /opt/aog/output.log
   ```
5. **Use SSL certificates** from a trusted CA if exposing the service
6. **Implement rate limiting** to prevent brute force attacks
7. **Regular backups** of configuration and data

## API Authentication (Future Enhancement)

For programmatic access, consider implementing API key authentication:

```json
{
  "api_keys": [
    {
      "key": "generated-secure-key",
      "name": "monitoring-system",
      "permissions": ["read"],
      "created": "2024-01-01T00:00:00Z"
    }
  ]
}
```

## Incident Response

If you suspect unauthorized access:

1. **Immediately restrict access**:
   ```bash
   # Change binding to localhost only
   sudo systemctl stop aog
   # Edit /opt/aog/data.json to set bind addresses to 127.0.0.1
   sudo systemctl start aog
   ```

2. **Review logs**:
   ```bash
   grep -E "auth|login|access" /opt/aog/output.log
   sudo journalctl -u aog --since "1 day ago"
   ```

3. **Change all passwords**
4. **Update the system**:
   ```bash
   sudo apt update && sudo apt upgrade
   ```

## Security Contact

Report security vulnerabilities to: [Create an issue on GitHub with [SECURITY] tag]