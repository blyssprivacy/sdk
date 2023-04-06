import { ChildProcess } from 'child_process';

const simple = require('./tests/simple').default;
const { spawn } = require('child_process');

process.removeAllListeners('warning');

function spawnChildProcess(paramsFilename: string): Promise<ChildProcess> {
  let seen = '';
  return new Promise(resolve => {
    const child = spawn(process.argv[2], [
      '8008',
      process.argv[3] + '/' + paramsFilename
    ]);
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

const paramsFilenames = ['v0.json', 'v1.json'];

async function runTests(paramsFilename: string) {
  const child = await spawnChildProcess(paramsFilename);

  await simple();

  child.kill();
}

async function main() {
  for (const paramsFilename of paramsFilenames) {
    await runTests(paramsFilename);
    console.log('Completed tests for ' + paramsFilename);
  }
  console.log('All tests completed successfully.');
}

main();
