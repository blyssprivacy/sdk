import React from 'react';

export default function Center({ children, space }) {
  space = space || 0;
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        width: '100%',
        paddingTop: space,
        paddingBottom: space,
        marginBottom: 'var(--ifm-paragraph-margin-bottom)'
      }}
    >
      {children}
    </div>
  );
}
