import React from 'react';

// Multiline callback prop with arrow function body
export const Callback = () => (
  <button
    onClick={() => {
      console.log('clicked');
    }}
    onMouseEnter={() => {
      const x = 1;
      return x;
    }}
  >
    click me
  </button>
);

// Inline callback
export const InlineCallback = () => <button onClick={() => console.log('hi')}>go</button>;
