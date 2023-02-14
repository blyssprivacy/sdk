import './styles.css';
import { Log, LogMessage } from './util';
import { Bucket, Client } from '@blyss/sdk';
import React, { useState } from 'react';

// This function gets called only on the first query
async function setup(apiKey: string): Promise<Bucket> {
  const client = new Client(apiKey);

  // Create the bucket, if it doesn't exist.
  // By default, only you can read and write from the buckets you create.
  // To make a bucket others can read, prefix the name with "global."
  const bucketName = 'global.private-valentines-13643';
  if (!(await client.exists(bucketName))) {
    console.log('creating bucket');
    await client.create(bucketName);
  }

  // Connect to your bucket
  const bucket = await client.connect(bucketName);

  return bucket;
}

async function deriveMessageKey(recipient: string): Promise<CryptoKey> {
  // 0.1. Create base key material from recipient handle
  const baseKey = await window.crypto.subtle.importKey(
    'raw',
    new TextEncoder().encode(recipient),
    { name: 'PBKDF2' },
    false,
    ['deriveKey']
  );

  const pkbdf2Params = {
    name: 'PBKDF2',
    salt: new TextEncoder().encode('valentine'),
    iterations: 100000,
    hash: 'SHA-256'
  };

  const aesGenKeyParams = {
    name: 'AES-GCM',
    length: 256
  };

  const key = await window.crypto.subtle.deriveKey(
    pkbdf2Params,
    baseKey,
    aesGenKeyParams,
    false,
    ['encrypt', 'decrypt']
  );

  return key;
}

async function computeServerKey(mailbox: string): Promise<string> {
  // 2.2. Hash recipient's handle to get destination key on server
  const hash = await window.crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(mailbox)
  );
  // 2.3. Convert hash to base64 string
  const targetKey = window.btoa(String.fromCharCode(...new Uint8Array(hash)));
  return targetKey;
}

function SendValentineCard({
  loading,
  handler
}: {
  loading: boolean;
  handler: (e: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <div className="actioncard">
      <h2>😘 Send</h2>
      <div>
        <form onSubmit={handler}>
          <div className="actioncard-field">
            <input
              type="text"
              id="to"
              placeholder="mailbox destination"
              title="up to 500 Unicode chars, enforced by truncation."
              required
            />
          </div>
          <div className="actioncard-field">
            <textarea
              id="msg"
              placeholder="Valentine message"
              title="UTF8 up to 1KiB, enforced by truncation. Message will be client-side encrypted, using a key derived from the recipient's name."
              required
            />
          </div>
          <div className="actioncard-buttons">
            <button disabled={loading}>
              {loading ? '  sending...  ' : 'send valentine'}
            </button>
            <div>{loading ? <div className="loader"></div> : null}</div>
          </div>
        </form>
      </div>
    </div>
  );
}

function PrivateReceiveValentineCard({
  loading,
  fetchedMessage,
  handler
}: {
  loading: boolean;
  fetchedMessage: string;
  handler: (e: React.FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <div className="actioncard">
      <h2>💌 Private Retrieve</h2>
      <form onSubmit={handler}>
        <div className="actioncard-field">
          <input
            type="text"
            id="to"
            placeholder="mailbox to check"
            title="up to 500 Unicode chars, enforced by truncation."
            required
          />
        </div>
        <div className="actioncard-field">
          <textarea
            id="msg"
            value={fetchedMessage}
            title="display for the fetched message."
            disabled
          />
        </div>
        <div className="actioncard-buttons">
          <button disabled={loading}>
            {loading ? 'fetching...' : 'fetch valentine'}
          </button>
          <div>{loading ? <div className="loader"></div> : null}</div>
        </div>
      </form>
    </div>
  );
}

function Explainer() {
  return (
    <div className="explainer">
      <p>
        Send and receive encrypted valentines, while keeping your amour’s
        identity fully private. Your browser will fetch Valentines via the{' '}
        <a href="https://blyss.dev">Blyss</a> protocol, which secures{' '}
        <i>metadata</i> - like “which Valentines are you looking at?" More info{' '}
        <a href="#faq">below</a>.
      </p>
    </div>
  );
}

function Faq() {
  return (
    <div className="FAQ" id="faq">
      <h2>FAQ</h2>
      <h4>
        Is this really homomorphic encryption? I thought that was impossible /
        really slow.
      </h4>
      <p>
        Yup, this is real-deal fully homomorphic encryption, running as realtime
        multiplayer in your browser. Five years ago, this demo probably did seem
        impossible, but a lot of recent work has made FHE fast enough for some
        specific applications, like the private information retrieval we show
        here. Want to try using fast FHE in your own apps?{' '}
        <a href="https://github.com/blyssprivacy/sdk">
          Here's our open-source SDK!
        </a>
      </p>

      <h4>How are my message contents secured?</h4>
      <p>
        To <b>send</b> a message addressed to mailbox <strong>M</strong>, the
        browser client first derives a key{' '}
        <code>
          <strong>K</strong> = PKBDF2(
          <strong>M</strong>)
        </code>
        , using a fixed salt. <strong>K</strong> is used by the client to AES
        encrypt the message; <strong>K</strong> never leaves your device. Of
        course, the server can't know <strong>M</strong>, so the client writes
        the encrypted message to server location{' '}
        <code>
          <strong>L</strong> = SHA256(<strong>M</strong>)
        </code>
        .<br></br>
        <br></br>
        To <b>retrieve</b> a message sent to mailbox <strong>M</strong>, we do
        the same steps in reverse: the client first computes{' '}
        <code>
          <strong>L</strong> = SHA256(<strong>M</strong>)
        </code>
        , then performs a metadata-private read for <strong>L</strong>; the
        result is the encrypted message data of mailbox <strong>M</strong>,
        which the client finally decrypts with key{' '}
        <code>
          <strong>K</strong> = PKBDF2(<strong>M</strong>)
        </code>
        .
      </p>

      <h4>How is my message metadata secured?</h4>
      <p>
        Fully homomorphic encryption (FHE) is what makes this special. It lets
        the server retrieve any data the client requests, while the server
        remains completely oblivious to the client's selection. Here are some
        more detailed explainers on FHE, in increasing levels of technicality: a{' '}
        <a href="https://blintzbase.com/posts/pir-and-fhe-from-scratch/">
          blog post
        </a>{' '}
        we wrote, our{' '}
        <a href="https://github.com/blyssprivacy/sdk">source code</a>, and a{' '}
        <a href="https://eprint.iacr.org/2022/368">paper we published</a>.
      </p>
      <h4>Is this like end-to-end encryption?</h4>
      <p>
        No, this is a different kind of privacy. In this toy demo, your message
        contents are encrypted, but under a weak key that is merely derived from
        the recipient's name - not something we'd ever call E2E. But the
        metadata of message retrievals is actually protected, so the server
        cannot know whom is messaging whom. Caveat: regardless of encryption
        strategy, patterns in client activity can always hint at client
        relationships, unless communicating parties take care to decorrelate
        their actions.
      </p>
      <h4>Could this be used as a metadata-private messenger?</h4>
      <p>
        With a couple more steps (starting with E2EE), maybe! If you're
        interested in this sort of thing, we should{' '}
        <a href="mailto:founders@blyss.dev">definitely talk</a>.
      </p>
    </div>
  );
}

// UI
function App() {
  const [bucketHandle, setBucketHandle] = useState<Bucket | undefined>();
  const [loading, setLoading] = useState(false);
  const [posting, setPosting] = useState(false);
  const [apiKey, setApiKey] = useState(
    'CSdK3rKXvb4zb43AgycQn6KAmJDILMXU8IWUHrn7'
  );
  const [numMessages, setNumMessages] = useState(
    Math.floor(Math.random() * 100) + 950
  );
  const [fetchedMessage, setfetchedMessage] = useState(
    '(waiting for retrieval)'
  );

  const [trace, setTrace] = useState<Log[]>([]);
  const logMessage = (t: Log) => setTrace([t, ...trace]);

  async function animatePost(to: string, message: string): Promise<void> {
    setPosting(true);

    // enforce size limits
    if (to.length > 500) {
      to = to.slice(0, 500);
    }
    if (message.length > 1000) {
      message = message.slice(0, 500);
    }

    // 0. Derive an encryption key from the recipient's handle
    const key = await deriveMessageKey(to);

    // 1. Get a handle to the bucket
    let bucket = bucketHandle;
    if (!bucket) {
      console.log('setup!');
      bucket = await setup(apiKey);
      setBucketHandle(bucket);
    }

    // 2.1. Encrypt the message
    const iv = window.crypto.getRandomValues(new Uint8Array(12));
    const encryptedMessage = await window.crypto.subtle.encrypt(
      {
        name: 'AES-GCM',
        iv
      },
      key,
      new TextEncoder().encode(message)
    );

    // 2.2. Prepend iv to encrypted message
    const encryptedMessageWithIv = new Uint8Array([
      ...iv,
      ...new Uint8Array(encryptedMessage)
    ]);

    // 2.3. Write encrypted message to KV server
    const serverKey = await computeServerKey(to);
    const start = performance.now();
    const _ = await bucket.write({
      [serverKey]: encryptedMessageWithIv
    });
    const tookMs = performance.now() - start;
    const isRetrieval = false;

    // 3. Log the result to the UI
    logMessage({
      to,
      isRetrieval,
      tookMs
    });

    setPosting(false);
  }

  async function animateFetch(to: string): Promise<void> {
    setLoading(true);

    // enforce size limits
    if (to.length > 500) {
      to = to.slice(0, 500);
    }

    // 1. Get a handle to the bucket
    let bucket = bucketHandle;
    if (!bucket) {
      console.log('setup!');
      bucket = await setup(apiKey);
      setBucketHandle(bucket);
    }

    // 2. Retrieve the specified mailbox
    const serverKey = await computeServerKey(to);
    const start = performance.now();
    const fetchedResult = await bucket.privateRead(serverKey);
    const tookMs = performance.now() - start;
    const isRetrieval = true;

    // 3. Log the result to the UI
    logMessage({
      to,
      isRetrieval,
      tookMs
    });

    if (fetchedResult === null) {
      // 4.1 If the mailbox is empty, we're done
      console.log('no messages yet :(');
      setfetchedMessage('no messages yet :(');
    } else {
      // 4.2 Decrypt the message
      console.log('decrypting message...');
      const key = await deriveMessageKey(to);
      const decryptedMessage = await window.crypto.subtle.decrypt(
        {
          name: 'AES-GCM',
          iv: fetchedResult.slice(0, 12)
        },
        key,
        fetchedResult.slice(12)
      );
      const decodedMessage = new TextDecoder().decode(decryptedMessage);
      setfetchedMessage(decodedMessage);
    }

    setLoading(false);
  }

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    const to = (event.target as any).to.value;
    const msg = (event.target as any).msg.value;
    console.log('Sending message ' + msg + ' to mailbox ' + to);
    return animatePost(to, msg);
  };

  const handleFetch = (event: React.FormEvent) => {
    event.preventDefault();
    const to = (event.target as any).to.value;
    console.log('Checking mailbox: ' + to + '...');
    return animateFetch(to);
  };

  return (
    <div className="App">
      <div className="App-main">
        <div className="title">
          <h2 style={{ margin: 0 }}>Private Valentine Retrieval</h2>
          <h3 style={{ margin: 0, color: '#F68E9D' }}>
            (using homomorphic encryption!)
          </h3>
        </div>
        <Explainer></Explainer>
        <div className="deck">
          <PrivateReceiveValentineCard
            loading={loading}
            fetchedMessage={fetchedMessage}
            handler={handleFetch}
          ></PrivateReceiveValentineCard>
          <SendValentineCard
            loading={posting}
            handler={handleSubmit}
          ></SendValentineCard>
        </div>
      </div>

      <div className="footer">
        <Faq></Faq>
        <div className="trace">
          <div>
            <h2>Trace</h2>
          </div>
          <div>
            {trace.length > 0
              ? trace.map((t, i) => (
                  <div key={i}>
                    <LogMessage {...t} />
                  </div>
                ))
              : null}
          </div>
        </div>
      </div>
    </div>
  );
}

export default App;
