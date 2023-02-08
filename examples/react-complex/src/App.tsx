import './App.css';
import { Bucket, Client } from '@blyss/sdk';
import React, { ReactNode } from 'react';

// This function gets called only on the first query
async function setup(): Promise<Bucket> {
  const client = new Client('<YOUR API KEY HERE>');

  // Create the bucket
  const bucketName = 'contact-demo';
  if (!(await client.exists(bucketName))) {
    await client.create(bucketName);
  }

  // Connect to your bucket
  const bucket = await client.connect(bucketName);

  return bucket;
}

function pickRandom(arr: string[]): string {
  return arr[Math.floor(Math.random() * arr.length)];
}

function pickRandomSubset(arr: string[]): string[] {
  let n = Math.floor(Math.random() * Math.min(arr.length, 5));
  if (n === 0) return [];

  let res = [];
  for (let i = 0; i < n; i++) res.push(pickRandom(arr));
  return Array.from(new Set(res));
}

function randomPhone(): string {
  const digit = (s: string) =>
    s === '-' ? '-' : Math.floor(Math.random() * 10);
  return 'XXX-XXX-XXXX'.split('').map(digit).join('');
}

function generateRandomUser(allPhones: string[]): User {
  const first = 'Joe,Ali,Alisa,Belen,Jakob,Cade,Brett,Trent,Silas'.split(',');
  const last = 'Brown,Jones,Miller,Davis,Garcia,Rodriguez'.split(',');
  const name = pickRandom(first) + ' ' + pickRandom(last);
  const handle =
    '@' + name.toLowerCase().replace(' ', '') + Math.floor(Math.random() * 100);
  const phone = randomPhone();
  const contactPhones = pickRandomSubset(allPhones);
  return { name, handle, phone, contactPhones };
}

interface User {
  name: string;
  phone: string;
  handle: string;
  contactPhones: string[];
}

function App() {
  const [bucketHandle, setBucketHandle] = React.useState<Bucket | undefined>();
  const [loading, setLoading] = React.useState(false);
  const [user, setUser] = React.useState<User>(generateRandomUser([]));
  const [resolveAll, setResolveAll] = React.useState(false);
  const [allPhones, setAllPhones] = React.useState<string[]>([]);

  const [trace, setTrace] = React.useState<ReactNode[]>([]);
  let logMessage = (t: ReactNode) => setTrace([t, ...trace]);

  async function performSignup(user: User): Promise<any[]> {
    // 1. Get the bucket
    let bucket = bucketHandle;
    if (!bucket) {
      bucket = await setup();
      setBucketHandle(bucket);
    }

    // 2. Add the user to the bucket, using their phone number as the key
    await bucket.write({
      [user.phone]: { name: user.name, phone: user.phone, handle: user.handle }
    });

    // 3. Check if any of the user's contacts are on the service
    let intersection: any[];
    if (resolveAll) {
      intersection = await bucket.privateIntersect(user.contactPhones);
    } else {
      intersection = await bucket.privateKeyIntersect(user.contactPhones);
    }

    return intersection;
  }

  async function animateSignup(user: User): Promise<void> {
    setLoading(true);

    // 1. Sign up the user
    let start = performance.now();
    let intersection = await performSignup(user);
    let took = (performance.now() - start) / 1000;
    let tookMsg = (
      <em style={{ color: '#666', paddingLeft: 20 }}>
        ({Math.round(took * 100) / 100} s)
      </em>
    );

    // 2. Log the result
    logMessage(
      <>
        <div>
          Wrote new user "{user.handle}".{' '}
          {intersection.length > 0 ? (
            <>
              Privately found {intersection.length} of their contacts using the
              service. {tookMsg}
              {resolveAll ? (
                <div>
                  Resolved the details on these contacts:
                  <pre>{JSON.stringify(intersection)}</pre>
                </div>
              ) : null}
            </>
          ) : (
            <>{tookMsg}</>
          )}
        </div>
      </>
    );

    // 3. Add phone number to a local store (for demo only)
    setAllPhones([...allPhones, user.phone]);

    setLoading(false);
  }

  let addOneRandomUser = () => {
    let user = generateRandomUser(allPhones);
    setUser(user);
    animateSignup(user);
  };

  let colGap: React.CSSProperties = {
    display: 'flex',
    flexDirection: 'column',
    gap: 10
  };

  return (
    <div className="App">
      <header className="App-header">Private Contact Lookup</header>
      <div className="App-main">
        <div>
          <div style={{ ...colGap, gap: 40 }}>
            <div>Number of users: {allPhones.length}</div>
            <div style={{ maxWidth: 300 }}>
              <label
                style={{ display: 'flex', alignItems: 'flex-start', gap: 10 }}
              >
                <input
                  type="checkbox"
                  checked={resolveAll}
                  onChange={() => setResolveAll(!resolveAll)}
                  style={{ margin: 4 }}
                />
                <div>
                  Checking this box will resolve the details (name, handle,
                  phone) of every contact that we discover.
                </div>
              </label>
            </div>
          </div>
        </div>
        <div>
          <p>
            <strong>Add a user</strong>
          </p>
          <div style={colGap}>
            <div>
              <input type="text" value={user.name} disabled />
            </div>
            <div>
              <input type="text" value={user.phone} disabled />
            </div>
            <div>
              <input type="text" value={user.handle} disabled />
            </div>
            <div>
              <button disabled={loading} onClick={addOneRandomUser}>
                Add 1 random user
              </button>
            </div>
          </div>
        </div>

        <div>
          <p>
            <strong>Trace</strong>
          </p>
          <div style={colGap}>
            {trace.length > 0
              ? trace.map((t, i) => <div key={i}>{t}</div>)
              : null}
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
