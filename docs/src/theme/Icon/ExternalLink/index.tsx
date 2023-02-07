import ExternalLink from '@theme-original/Icon/ExternalLink';
import React from 'react';

export default function ExternalLinkWrapper(props) {
  return (
    <div style={{ display: 'none' }}>
      <ExternalLink {...props} />
    </div>
  );
}
