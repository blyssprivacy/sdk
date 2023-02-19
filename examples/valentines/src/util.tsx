export interface Log {
  to: string;
  isRetrieval: boolean;
  tookMs: number;
}

export function LogMessage({ to, isRetrieval, tookMs }: Log) {
  const tookMsg = (
    <span style={{ color: '#666', paddingLeft: 5 }}>
      ({Math.round((tookMs / 1000) * 10) / 10} s)
    </span>
  );
  return (
    <>
      <div className="logline">
        {!isRetrieval ? (
          <>
            Sent a valentine to {to}. {tookMsg}
          </>
        ) : (
          <>
            Privately checked mailbox for {to}. {tookMsg}.
          </>
        )}
      </div>
    </>
  );
}
