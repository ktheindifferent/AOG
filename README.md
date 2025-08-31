# A.O.G. (Algae Oxygen Reactor)

CO2 is a dangerous indoor/outdoor air pollutant. A.O.G. uses blue-green algae to convert CO2 into Oxygen.

<img src="./media/IMG_20200917_201800.jpg" alt="drawing" width="200"/>

When I first activated the A.O.G....my average indoor CO2 levels were 1000-2000ppm+.  
After 6+ months of use my indoor CO2 level averages between 400-600ppm. 

This project is licensed under the MIT license so that individuals and corporations can make/modify/sell A.O.G. without any restrictions. 

Prototype MK1 build pictures and videos can be found in the media folder. 

A parts list and build instructions can be found in the BUILD.md file.

## Command API Security Configuration

The Command API server (port 9443) has been secured with the following features:

### Security Features
- **Localhost-only connections**: The API server only accepts connections from 127.0.0.1 (IPv4) or ::1 (IPv6)
- **Token authentication**: Optional API token authentication for enhanced security
- **Rate limiting**: Maximum 10 requests per minute per client to prevent abuse
- **Command whitelisting**: Only safe, pre-approved commands are allowed

### Managing API Authentication

Generate a new API token:
```bash
./aog api token generate
```

Remove API token (disables authentication):
```bash
./aog api token remove
```

Check token status:
```bash
./aog api token status
```

### Using the API with Authentication

When a token is configured, include it in the Authorization header:
```bash
curl -X POST https://localhost:9443/api/command \
  -H "Authorization: Bearer YOUR_TOKEN_HERE" \
  -H "Content-Type: application/json" \
  -d '{"input_command":"help"}'
```

### Security Best Practices
1. Always generate and use an API token in production environments
2. Store the API token securely and never commit it to version control
3. The API server is hardcoded to bind only to localhost for maximum security
4. Rejected connection attempts and authentication failures are logged
5. Rate limiting prevents brute force attacks (10 requests/minute)

### Configuration

The following security settings are stored in `/opt/aog/data.json`:
- `command_api_bind_port`: Port for the command API (default: 9443)
- `command_api_token`: API authentication token (optional)

Note: The bind address is always forced to 127.0.0.1 for security and cannot be changed.

## License

Released under MIT.

# Support and follow my work by:

#### Buying my dope NTFs:
 * https://opensea.io/accounts/PixelCoda

#### Checking out my Github:
 * https://github.com/PixelCoda

#### Following my facebook page:
 * https://www.facebook.com/pixelcoda/

#### Subscribing to my Patreon:
 * https://www.patreon.com/calebsmith_pixelcoda

#### Or donating crypto:
 * ADA:    addr1vyjsx8zthl5fks8xjsf6fkrqqsxr4f5tprfwux5zsnz862glwmyr3
 * BTC:    3BCj9kYsqyENKU5YgrtHgdQh5iA7zxeJJi
 * MANA:   0x10DFc66F881226f2B91D552e0Cf7231C1e409114
 * SHIB:   0xdE897d5b511A66276E9B91A8040F2592553e6c28
