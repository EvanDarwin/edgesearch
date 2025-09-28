#!/usr/bin/env node

function getEvalCommand() {
    if (process.platform === 'win32') {
        if (process.env.PSModulePath) {
            // PowerShell
            return 'Invoke-Expression (npx edgesearch env set default)';
        } else {
            // cmd
            return 'for /f "delims=" %i in (\'npx edgesearch env set default\') do %i';
        }
    } else {
        // Unix-like
        return 'eval $(npx edgesearch env set default)';
    }
}

const command = getEvalCommand();
console.log(`To set WRANGLER_CONFIG, run: ${command}`);