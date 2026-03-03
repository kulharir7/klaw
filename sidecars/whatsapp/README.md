# WhatsApp Sidecar

Real WhatsApp integration using Baileys library.

## Setup

```bash
cd sidecars/whatsapp
npm install
npm start
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| PORT | 3001 | Sidecar port |
| KLAW_URL | http://localhost:8080 | Klaw gateway URL |
| SESSION_PATH | ./session | Auth session path |

## API Endpoints

### GET /health
Health check endpoint.

### GET /status
Get connection status.

### GET /qr
Get QR code for pairing.

### POST /send
Send a text message.

```json
{
  "to": "+1234567890",
  "text": "Hello from Klaw!"
}
```

### POST /send-media
Send media message.

```json
{
  "to": "+1234567890",
  "mediaUrl": "https://example.com/image.jpg",
  "caption": "Check this out!"
}
```

### POST /read
Mark message as read.

```json
{
  "messageId": "abc123",
  "from": "+1234567890@s.whatsapp.net"
}
```

## Klaw Integration

Configure Klaw to use the sidecar:

```json
{
  "channels": {
    "whatsapp": {
      "enabled": true,
      "sidecar_url": "http://localhost:3001",
      "phone_number": "+1234567890"
    }
  }
}
```

## First Run

1. Start the sidecar: `npm start`
2. Open http://localhost:3001/qr in browser
3. Scan QR with WhatsApp on your phone
4. Session is saved for future runs

## Security

- Auth state saved locally in `./session/`
- Never commit session files
- Use HTTPS in production
- Set KLAW_URL to your gateway

## Notes

- Uses WhatsApp Web protocol (Baileys)
- May require occasional re-pairing
- WhatsApp Business API recommended for production