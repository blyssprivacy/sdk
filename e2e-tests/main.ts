import { ChildProcess } from 'child_process';

const simple = require('./tests/simple').default;
const { spawn } = require('child_process');

process.removeAllListeners('warning');

function spawnChildProcess(
  port: string,
  paramsFilename: string
): Promise<ChildProcess> {
  let seen = '';
  return new Promise(resolve => {
    console.log(process.argv[3] + '/' + paramsFilename);
    const child = spawn(process.argv[2], [
      port,
      process.argv[3] + '/' + paramsFilename
    ]);
    process.on('exit', function () {
      child.kill();
    });
    child.stdout.on('data', chunk => {
      // process.stdout.write(chunk);
      seen += chunk;
      if (seen.includes('Listening on ' + port)) {
        resolve(child);
      }
    });
    child.stderr.on('data', chunk => {
      console.log(chunk.toString());
    });
  });
}

const paramsFilenames = ['v0.json', 'v1.json'];

async function runTests(port: string, paramsFilename: string) {
  const child = await spawnChildProcess(port, paramsFilename);
  await new Promise(resolve => setTimeout(resolve, 1000));

  await simple(port);

  const promise: Promise<void> = new Promise(resolve => {
    child.on('exit', () => {
      resolve();
    });
  });

  child.kill();

  await promise;
}

async function main() {
  let portStart = 8008;
  for (const paramsFilename of paramsFilenames) {
    await runTests('' + portStart++, paramsFilename);
    console.log('Completed tests for ' + paramsFilename);
  }
  console.log('All tests completed successfully.');
}

main();
