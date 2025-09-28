#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { execSync } from 'node:child_process';
import { Command } from 'commander';
import readline from 'node:readline';
import chalk from 'chalk';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const program = new Command();

program
    .name(chalk.ansi256(214)('edge') + chalk.gray('search'))
    .description('CLI for edgesearch package')
    .version('0.1.2');

const envCmd = program.command('env')
    .description('Manage wrangler environments');

envCmd.command('set')
    .description('Set an active wrangler environment')
    .argument('[envName]', 'environment name, defaults to "default"', 'default')
    .action((envName) => {
        const fileName = envName === 'default' ? 'wrangler.jsonc' : `wrangler.${envName}.jsonc`;
        console.error(chalk.green(`ES_WRANGLER_CONFIG set to ${fileName}`));
        console.log(getExportCommand('ES_WRANGLER_CONFIG', fileName));
        process.env.ES_WRANGLER_CONFIG = fileName;
        process.exit(0);
    });

envCmd.command('init')
    .description('Generate a new wrangler environment configuration')
    .argument('[envName]', 'environment name, defaults to "default"', 'default')
    .action(async (envName) => {
        const rl = readline.createInterface({
            input: process.stdin,
            output: process.stdout
        });

        function ask(question) {
            return new Promise((resolve) => {
                rl.question(chalk.yellow(question), resolve);
            });
        }

        // Ask for KV ID
        const kvId = await ask('Enter your Cloudflare KV datastore ID: ');
        if (!kvId.trim()) {
            console.error(chalk.red('KV ID is required!'));
            rl.close();
            process.exit(1);
        }
        console.log("");

        const fileName = envName === 'default' ? 'wrangler.jsonc' : `wrangler.${envName}.jsonc`;
        const packages = ["indexer"];

        for (const pkg of packages) {
            const dir = path.join(__dirname, '..', 'workers', pkg);
            if (!fs.existsSync(dir)) {
                throw new Error(`workers/${pkg} directory not found. Please run this command from the edgesearch root directory.`);
            }

            const name = envName === 'default' ? `edgesearch-${pkg}` : `edgesearch-${envName}-${pkg}`;
            // Create the wrangler.jsonc content as string to preserve comments
            const wranglerContent = `{
	"name": "${name}",
	"compatibility_date": "2025-09-07",
	"compatibility_flags": ["nodejs_compat"],
	"main": "./build/index.js",
	"workers_dev": true,
	"preview_urls": false,
	"placement": {
		"mode": "smart"
	},
	"kv_namespaces": [
		{
			"binding": "INDEX",
			"id": "${kvId.trim()}"
		}
	],
	"observability": {
		"enabled": true,
		"head_sampling_rate": 1.0
	},
	"build": {
		"command": "npm run build"
	}
}`;
            // Write to file
            const filePath = path.join(__dirname, '..', 'workers', 'indexer', fileName);
            fs.writeFileSync(filePath, wranglerContent);

            console.log(chalk.green(`written ${chalk.yellow(path.relative(process.cwd(), filePath))} ${"OK".padStart(80 - filePath.length, ' ')}`));
        }

        console.error(chalk.green(`\nEnable your new environment with:\n\t${chalk.yellow(getSetEnvEvalCommand(envName))}`));
        rl.close();
    });

program.command('deploy')
    .description('Deploy using lerna')
    .action(() => {
        if (!process.env.ES_WRANGLER_CONFIG) {
            console.error(chalk.red(`ES_WRANGLER_CONFIG is not set. Use "${getSetEnvEvalCommand("<env>")}" to set it.`));
            process.exit(1);
        }
        console.log(chalk.blue('Running lerna run -r deploy...'));
        try {
            execSync('lerna run -r deploy', { stdio: 'inherit', cwd: path.join(__dirname, '..') });
        } catch (error) {
            console.error(chalk.red('Deploy failed:', error.message));
            process.exit(1);
        }
    });

function getExportCommand(varName, value) {
    const prefix = (process.platform === 'win32' ? (process.env.PSModulePath ? '$env:' : 'set ') : 'export ');
    return `${prefix}${varName}=${value}`;
}

function getSetEnvEvalCommand(env) {
    if (process.platform === 'win32') {
        if (process.env.PSModulePath) {
            // PowerShell
            return `Invoke-Expression (npx edgesearch env set ${env})`;
        } else {
            // cmd
            return `for /f "delims=" %i in ('npx edgesearch env set ${env}') do %i`;
        }
    } else {
        // Unix-like
        return `eval $(npx edgesearch env set ${env})`;
    }
}

program.parse();