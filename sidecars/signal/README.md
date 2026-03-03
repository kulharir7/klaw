# Signal Sidecar

Real Signal integration using signal-cli REST API.

## Prerequisites

1. Install signal-cli:
```bash
# Debian/Ubuntu
sudo apt install signal-cli

# Arch Linux
yay -S signal-cli

# macOS
brew install signal-cli
```

2. Run signal-cli REST API:
```bash
docker run -d \
  --name signal-cli \
  -p 8080:8080 \
  -v $HOME/.signal-cli:/signal \
  bbernhard/signal-cli-rest-api
```

## Setup

```bash
cd sidecars/signal
npm install
npm start
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| PORT | 3002 | Sidecar port |
| KLAW_URL | http://localhost:8080 | Klaw gateway URL |
| SIGNAL_CLI_URL | http://localhost:8080 | signal-cli REST URL |
| PHONE_NUMBER | "" | Registered phone number |

## API Endpoints

### GET /health
Health check endpoint.

### GET /status
Get connection status.

### POST /register
Register phone number.

```json
{
  "number": "+1234567890",
  "captcha": "optional-captcha"
}
```

### POST /verify
Verify with SMS code.

```json
{
  "number": "+1234567890",
  "code": "123456"
}
```

### POST /send
Send a message.

```json
{
  "to": "+1234567890",
  "text": "Hello from Klaw!"
}
```

### POST /send-group
Send to a group.

```json
{
  "groupId": "group.id.123",
  "text": "Group message"
}
```

### GET /groups
List all groups.

### POST /link
Link new device (returns QR).

### GET /devices
List linked devices.

## Klaw Integration

Configure Klaw to use the sidecar:

```json
{
  "channels": {
    "signal": {
      "enabled": true,
      "sidecar_url": "http://localhost:3002",
      "phone_number": "+1234567890"
    }
  }
}
```

## First Run

1. Start signal-cli: `docker run -d bbernhard/signal-cli-rest-api`
2. Start sidecar: `npm start`
3. Register: `POST /register` with your phone
4. Verify: `POST /verify` with SMS code
5. Ready to send!

## Security

- Phone verification required
- session stored in signal-cli
- Use HTTPS in production
- Rate limits: ~60 messages/hour

## Notes

- Requires signal-cli running
- Can use Docker for signal-cli
- Alternative: Embedded signal-cli (Java)