/**
 * Signal Sidecar for Klaw AI Gateway
 * 
 * Uses signal-cli REST API for Signal protocol
 * Provides HTTP API for Klaw gateway to send/receive messages
 */

const express = require('express');
const axios = require('axios');
const qrcode = require('qrcode-terminal');

const PORT = process.env.PORT || 3002;
const KLAW_URL = process.env.KLAW_URL || 'http://localhost:8080';
const SIGNAL_CLI_URL = process.env.SIGNAL_CLI_URL || 'http://localhost:8080';
const PHONE_NUMBER = process.env.PHONE_NUMBER || '';

const app = express();
app.use(express.json());

let isRegistered = false;
let phoneNumber = PHONE_NUMBER;

/**
 * Check if signal-cli is available
 */
async function checkSignalCLI() {
    try {
        const response = await axios.get(`${SIGNAL_CLI_URL}/v1/about`);
        return response.data;
    } catch (err) {
        console.error('Signal CLI not available:', err.message);
        return null;
    }
}

/**
 * Register phone number
 */
async function registerPhone(number, captcha = null) {
    try {
        const payload = captcha ? { captcha } : {};
        await axios.post(`${SIGNAL_CLI_URL}/v1/register/${number}`, payload);
        console.log(`✅ Registration initiated for ${number}`);
        console.log('📱 You will receive an SMS verification code');
        return { success: true, message: 'Registration initiated. Verify with code.' };
    } catch (err) {
        console.error('Registration failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * Verify phone number with code
 */
async function verifyPhone(number, code) {
    try {
        await axios.post(`${SIGNAL_CLI_URL}/v1/verify/${number}/${code}`);
        console.log(`✅ Phone number verified: ${number}`);
        isRegistered = true;
        phoneNumber = number;
        return { success: true };
    } catch (err) {
        console.error('Verification failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * Send message
 */
async function sendMessage(to, text) {
    if (!phoneNumber) {
        throw new Error('Phone number not configured');
    }
    
    try {
        await axios.post(`${SIGNAL_CLI_URL}/v2/send`, {
            number: phoneNumber,
            recipients: [to],
            message: text,
        });
        
        console.log(`✅ Message sent to ${to}`);
        return { success: true, to };
    } catch (err) {
        console.error('Send failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * Send to group
 */
async function sendGroupMessage(groupId, text) {
    if (!phoneNumber) {
        throw new Error('Phone number not configured');
    }
    
    try {
        await axios.post(`${SIGNAL_CLI_URL}/v2/groups/${groupId}/send`, {
            number: phoneNumber,
            message: text,
        });
        
        console.log(`✅ Group message sent to ${groupId}`);
        return { success: true, groupId };
    } catch (err) {
        console.error('Group send failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * Get groups
 */
async function getGroups() {
    if (!phoneNumber) {
        throw new Error('Phone number not configured');
    }
    
    try {
        const response = await axios.get(`${SIGNAL_CLI_URL}/v1/groups/${phoneNumber}`);
        return response.data;
    } catch (err) {
        console.error('Get groups failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * Link new device (returns QR code URI)
 */
async function linkDevice() {
    try {
        const response = await axios.post(`${SIGNAL_CLI_URL}/v1/devices/link`);
        const { uri } = response.data;
        
        console.log('\n📱 Link device:');
        console.log('Open Signal app → Settings → Linked devices → Link new device');
        qrcode.generate(uri, { small: true });
        
        return { success: true, uri };
    } catch (err) {
        console.error('Link failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * List linked devices
 */
async function listDevices() {
    try {
        const response = await axios.get(`${SIGNAL_CLI_URL}/v1/devices`);
        return response.data;
    } catch (err) {
        console.error('List devices failed:', err.response?.data || err.message);
        throw new Error(err.response?.data?.message || err.message);
    }
}

/**
 * Notify Klaw gateway of events
 */
async function notifyKlaw(event, data) {
    try {
        await axios.post(`${KLAW_URL}/api/webhook/signal`, {
            event,
            data,
        });
    } catch (err) {
        console.error('Failed to notify Klaw:', err.message);
    }
}

/**
 * Get connection status
 */
function getStatus() {
    return {
        connected: isRegistered,
        phoneNumber: phoneNumber,
        signalCli: SIGNAL_CLI_URL,
        klawGateway: KLAW_URL,
    };
}

// HTTP API

app.get('/health', (req, res) => {
    res.json({ 
        status: 'ok',
        signalCli: SIGNAL_CLI_URL,
    });
});

app.get('/status', (req, res) => {
    res.json(getStatus());
});

app.post('/register', async (req, res) => {
    try {
        const { number, captcha } = req.body;
        if (!number) {
            return res.status(400).json({ error: 'number required' });
        }
        
        const result = await registerPhone(number, captcha);
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.post('/verify', async (req, res) => {
    try {
        const { number, code } = req.body;
        if (!number || !code) {
            return res.status(400).json({ error: 'number and code required' });
        }
        
        const result = await verifyPhone(number, code);
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
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

app.post('/send-group', async (req, res) => {
    try {
        const { groupId, text } = req.body;
        if (!groupId || !text) {
            return res.status(400).json({ error: 'groupId and text required' });
        }
        
        const result = await sendGroupMessage(groupId, text);
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.get('/groups', async (req, res) => {
    try {
        const groups = await getGroups();
        res.json(groups);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.post('/link', async (req, res) => {
    try {
        const result = await linkDevice();
        res.json(result);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

app.get('/devices', async (req, res) => {
    try {
        const devices = await listDevices();
        res.json(devices);
    } catch (err) {
        res.status(500).json({ error: err.message });
    }
});

// Webhook receiver for signal-cli
app.post('/webhook', async (req, res) => {
    const { envelope } = req.body;
    
    if (envelope) {
        const source = envelope.source;
        const timestamp = envelope.timestamp;
        
        // Handle data message
        if (envelope.dataMessage) {
            const text = envelope.dataMessage.message;
            
            console.log(`📩 Message from ${source}: ${text}`);
            
            // Forward to Klaw
            await notifyKlaw('message', {
                from: source,
                text: text,
                timestamp: timestamp,
                isGroup: envelope.dataMessage.groupInfo !== undefined,
            });
        }
    }
    
    res.json({ received: true });
});

// Start server
app.listen(PORT, async () => {
    console.log(`🚀 Signal Sidecar running on port ${PORT}`);
    console.log(`📡 Connected to Klaw at ${KLAW_URL}`);
    console.log(`📱 Signal CLI at ${SIGNAL_CLI_URL}`);
    
    // Check signal-cli availability
    const status = await checkSignalCLI();
    if (status) {
        console.log('✅ Signal CLI is available');
    } else {
        console.log('⚠️ Signal CLI not available. Start signal-cli-rest-api first.');
    }
});

// Handle shutdown
process.on('SIGINT', () => {
    console.log('Shutting down...');
    process.exit(0);
});

module.exports = {
    sendMessage,
    sendGroupMessage,
    getGroups,
    linkDevice,
    listDevices,
    registerPhone,
    verifyPhone,
    getStatus,
};