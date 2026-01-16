// Quanta Wallet Background Service Worker
// Handles communication between content scripts and popup

console.log('🔮 Quanta Wallet Background Service Worker Started');

// Store pending requests
const pendingRequests = new Map();
let requestIdCounter = 0;

// Listen for messages from content scripts
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
    console.log('Background received message:', message);

    switch (message.type) {
        case 'QUANTA_CONNECT_REQUEST':
            handleConnectRequest(message, sender, sendResponse);
            return true; // Keep channel open for async response

        case 'QUANTA_GET_BALANCE':
            handleBalanceRequest(message, sender, sendResponse);
            return true;

        case 'QUANTA_SEND_TRANSACTION':
            handleTransactionRequest(message, sender, sendResponse);
            return true;

        case 'QUANTA_SIGN_MESSAGE':
            handleSignRequest(message, sender, sendResponse);
            return true;

        case 'POPUP_RESPONSE':
            handlePopupResponse(message);
            break;

        default:
            console.warn('Unknown message type:', message.type);
    }
});

// Handle connection request from dApp
async function handleConnectRequest(message, sender, sendResponse) {
    const requestId = requestIdCounter++;
    const origin = new URL(sender.url).origin;

    // Store the request
    pendingRequests.set(requestId, {
        type: 'connect',
        origin,
        sender,
        sendResponse,
        timestamp: Date.now()
    });

    // Open popup to approve connection
    try {
        await chrome.action.openPopup();

        // Send request to popup
        chrome.runtime.sendMessage({
            type: 'CONNECTION_REQUEST',
            requestId,
            origin,
            favicon: sender.tab?.favIconUrl
        });
    } catch (error) {
        console.error('Failed to open popup:', error);
        sendResponse({ approved: false, error: 'Failed to open wallet' });
    }
}

// Handle balance request
async function handleBalanceRequest(message, sender, sendResponse) {
    try {
        // Forward to popup or fetch directly
        const response = await chrome.runtime.sendMessage({
            type: 'GET_BALANCE',
            address: message.address
        });

        sendResponse(response);
    } catch (error) {
        console.error('Balance request failed:', error);
        sendResponse({ balance: 0, error: error.message });
    }
}

// Handle transaction request
async function handleTransactionRequest(message, sender, sendResponse) {
    const requestId = requestIdCounter++;
    const origin = new URL(sender.url).origin;

    // Store the request
    pendingRequests.set(requestId, {
        type: 'transaction',
        origin,
        sender,
        sendResponse,
        transaction: message.transaction,
        timestamp: Date.now()
    });

    // Open popup for approval
    try {
        await chrome.action.openPopup();

        chrome.runtime.sendMessage({
            type: 'TRANSACTION_REQUEST',
            requestId,
            origin,
            transaction: message.transaction,
            favicon: sender.tab?.favIconUrl
        });
    } catch (error) {
        console.error('Failed to open popup:', error);
        sendResponse({ success: false, error: 'Failed to open wallet' });
    }
}

// Handle sign message request
async function handleSignRequest(message, sender, sendResponse) {
    const requestId = requestIdCounter++;
    const origin = new URL(sender.url).origin;

    pendingRequests.set(requestId, {
        type: 'sign',
        origin,
        sender,
        sendResponse,
        message: message.message,
        timestamp: Date.now()
    });

    try {
        await chrome.action.openPopup();

        chrome.runtime.sendMessage({
            type: 'SIGN_REQUEST',
            requestId,
            origin,
            message: message.message,
            favicon: sender.tab?.favIconUrl
        });
    } catch (error) {
        console.error('Failed to open popup:', error);
        sendResponse({ success: false, error: 'Failed to open wallet' });
    }
}

// Handle response from popup
function handlePopupResponse(message) {
    const request = pendingRequests.get(message.requestId);

    if (!request) {
        console.warn('No pending request found for ID:', message.requestId);
        return;
    }

    // Send response back to content script
    if (request.sendResponse) {
        request.sendResponse(message.response);
    }

    // Clean up
    pendingRequests.delete(message.requestId);
}

// Clean up old requests (timeout after 5 minutes)
setInterval(() => {
    const now = Date.now();
    const timeout = 5 * 60 * 1000; // 5 minutes

    for (const [requestId, request] of pendingRequests.entries()) {
        if (now - request.timestamp > timeout) {
            if (request.sendResponse) {
                request.sendResponse({
                    success: false,
                    error: 'Request timeout'
                });
            }
            pendingRequests.delete(requestId);
        }
    }
}, 60000); // Check every minute

// Handle extension installation
chrome.runtime.onInstalled.addListener((details) => {
    console.log('Quanta Wallet installed/updated:', details.reason);

    if (details.reason === 'install') {
        // Open welcome page on first install
        chrome.tabs.create({
            url: 'https://quantachain.org/wallet/welcome'
        });
    }
});

// Export for testing
if (typeof module !== 'undefined' && module.exports) {
    module.exports = { pendingRequests };
}
