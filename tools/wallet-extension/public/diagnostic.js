// Quick diagnostic script to check if elements exist
console.log('=== QUANTA WALLET DIAGNOSTIC ===');
console.log('DOM Ready:', document.readyState);
console.log('startSetupBtn exists:', !!document.getElementById('startSetupBtn'));
console.log('walletView exists:', !!document.getElementById('walletView'));
console.log('All views:', document.querySelectorAll('.view').length);

// Wait for DOM
document.addEventListener('DOMContentLoaded', () => {
    console.log('✅ DOM Loaded!');
    console.log('startSetupBtn NOW exists:', !!document.getElementById('startSetupBtn'));
    console.log('All buttons:', document.querySelectorAll('button').length);
});
