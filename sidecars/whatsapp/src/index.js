/**
 * WhatsApp Sidecar for Klaw AI Gateway
 * 
 * Uses Baileys library for WhatsApp Web protocol
 * Provides HTTP API for Klaw gateway to send/receive messages
 */

const { makeWASocket, useMultiFileAuthState, DisconnectReason } = require('@whiskeysockets/baileys');
const express = require('express');
const pino = require('pino');
const qrcode = require('qrcode-terminal');

const PORT = process.env.PORT || 3001;
const KLAW_URL = process.env.KLAW_URL || 'http://localhost:8080';
const SESSION_PATH = process.env.SESSION_PATH || './session';

const logger = pino({ level: 'silent' });
const app = express();
app.use(express.json());

let sock = null;
let isConnected = false;
let qrCode = null;

/**
 * Initialize WhatsApp connection
 */
async function initWhatsApp() {
    const { state, saveCreds } = await useMultiFileAuthState(SESSION_PATH);
    
    sock = makeWASocket({
        auth: state,
        logger,
        printQRInTerminal: true,
        browser: ['Klaw AI', 'Chrome', '1.0.0'],
    });
    
    sock.ev.on('connection.update', async (update) => {
        const { connection, lastDisconnect, qr } = update;
        
        if (qr) {
            qrCode = qr;
            console.log('\n📱 QR Code:');
            qrcode.generate(qr, { small: true });
            console.log('\nScan this QR code with WhatsApp on your phone');
        }
        
        if (connection === 'close') {
            const shouldReconnect = lastDisconnect?.error?.output?.statusCode !== DisconnectReason.loggedOut;
            isConnected = false;
            qrCode = null;
            
            console.log('Connection closed. Reconnecting:', shouldReconnect);
            
            if (shouldReconnect) {
                setTimeout(initWhatsApp, 5000);
            }
        }
        
        if (connection === 'open') {
            isConnected = true;
            qrCode = null;
            console.log('✅ WhatsApp connected!');
            
            // Notify Klaw gateway
            await notifyKlaw('connected', { 
                phone: sock.user?.id,
                name: sock.user?.name 
            });
        }
    });
    
    sock.ev.on('creds.update', saveCreds);
    
    sock.ev.on('messages.upsert', async ({ messages, type }) => {
        for (const msg of messages) {
            if (msg.key.fromMe) continue;
            if (type !== 'notify') continue;
            
            const from = msg.key.remoteJid;
            const text = msg.message?.conversation || 
                         msg.message?.extendedTextMessage?.text || '';
            
            if (text) {
                console.log(`📩 Message from ${from}: ${text}`);
                
                // Forward to Klaw gateway
                await notifyKlaw('message', {
                    from: from,
                    text: text,
                    timestamp: msg.messageTimestamp,
                    messageId: msg.key.id,
                    isGroup: from.includes('@g.us'),
                });
            }
        }
    });
}

/**
 * Send message via WhatsApp
 */
async function sendMessage(to, text) {
    if (!sock || !isConnected) {
        throw new Error('WhatsApp not connected');
    }
    
    const jid = to.includes('@') ? to : `${to}@s.whatsapp.net`;
    
    await sock.sendMessage(jid, { text });
    console.log(`✅ Message sent to ${jid}`);
    
    return { success: true, to: jid };
}

/**
 * Send media via WhatsApp
 */
async function sendMedia(to, mediaUrl, caption) {
    if (!sock || !isConnected) {
        throw new Error('WhatsApp not connected');
    }
    
    const jid = to.includes('@') ? to : `${to}@s.whatsapp.net`;
    
    // Determine media type from URL
    const mediaMessage = {
        image: { url: mediaUrl },
        caption: caption || '',
    };
    
    await sock.sendMessage(jid, mediaMessage);
    console.log(`✅ Media sent to ${jid}`);
    
    return { success: true, to: jid };
}

/**
 * Mark message as read
 */
async function markRead(messageId, from) {
    if (!sock || !isConnected) {
        throw new Error('WhatsApp not connected');
    }
    
    await sock.readMessages([{ key: { id: messageId, remoteJid: from } }]);
    return { success: true };
}

/**
 * Get QR code for pairing
 */
function getQRCode() {
    return qrCode;
}

/**
 * Get connection status
 */
function getStatus() {
    return {
        connected: isConnected,
        phone: sock?.user?.id || null,
        name: sock?.user?.name || null,
        hasQR: qrCode !== null,
    };
}

/**
 * Notify Klaw gateway of events
 */
async function notifyKlaw(event, data) {
    try {
        await fetch(`${KLAW_URL}/api/webhook/whatsapp`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ event, data }),
        });
    } catch (err) {
        console.error('Failed to notify Klaw:', err.message);
    }
}

// HTTP API

app.get('/health', (req, res) => {
    res.json(getStatus());
});

app.get('/status', (req, res) => {
    res.json(getStatus());
});

app.get('/qr', (req, res) => {
    if (qrCode) {
        res.json({ qr: qrCode, connected: false });
    } else {
        res.json({ connected: isConnected, phone: sock?.user?.id });
    }
});

app.post('/send', async (req, res) => {
    try {
        const { to, text } = req.body;
        if (!to || !text) {
            return res.status(400).json({ error: 'to and text required' });
        }
        
        const result = await sendMessage(to, text);
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.post('/send-media', async (req, res) => {
    try {
        const { to, mediaUrl, caption } = req.body;
        if (!to || !mediaUrl) {
            return res.status(400).json({ error: 'to and mediaUrl required' });
        }
        
        const result = await sendMedia(to, mediaUrl, caption);
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.post('/read', async (req, res) => {
    try {
        const { messageId, from } = req.body;
        const result = await markRead(messageId, from);
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

// Start server
app.listen(PORT, () => {
    console.log(`🚀 WhatsApp Sidecar running on port ${PORT}`);
    console.log(`📡 Connected to Klaw at ${KLAW_URL}`);
    
    initWhatsApp();
});

// Handle shutdown
process.on('SIGINT', () => {
    console.log('Shutting down...');
    process.exit(0);
});

module.exports = {
    sendMessage,
    sendMedia,
    markRead,
    getStatus,
    getQRCode,
};