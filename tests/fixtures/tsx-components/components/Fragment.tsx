import React from 'react';

// JSX fragments: <>...</>
export const Fragment = () => (
  <>
    <span>one</span>
    <span>two</span>
  </>
);

// Inline fragment
export const InlineFragment = () => <><span>a</span><span>b</span></>;
