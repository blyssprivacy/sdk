import TOC from '@theme-original/TOC';
import React from 'react';

export default function TOCWrapper(props) {
  return (
    <div className="toc-wrapper">
      <TOC {...props} />
    </div>
  );
}
