import ColorModeToggle from '@theme-original/Navbar/ColorModeToggle';
import React from 'react';

export default function ColorModeToggleWrapper(props) {
  return (
    <div style={{ display: 'none' }}>
      <ColorModeToggle {...props} />
    </div>
  );
}
