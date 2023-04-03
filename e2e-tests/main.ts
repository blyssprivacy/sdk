import { ChildProcess } from 'child_process';

const simple = require('./tests/simple').default;
const { spawn } = require('child_process');

process.removeAllListeners('warning');

function spawnChildProcess(): Promise<ChildProcess> {
  let seen = '';
  return new Promise(resolve => {
    const child = spawn(process.argv[2]);
    process.on('exit', function () {
      child.kill();
    });
    child.stdout.on('data', chunk => {
      seen += chunk;
      if (seen.includes('Listening')) {
        resolve(child);
      }
    });
    child.stderr.on('data', chunk => {
      console.log(chunk.toString());
    });
  });
}

async function main() {
  const child = await spawnChildProcess();

  await simple();

  console.log('Tests completed successfully.');

  child.kill();
}

main();
