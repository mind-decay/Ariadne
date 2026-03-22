import React from 'react';

// Bug #2a: {{ }} in JSX prop + text content on same line
export const StyledBox = () => <div style={{ color: 'red' }}>hello</div>;

// Bug #2b: {{ }} in JSX prop + {expression} on same line
export const StyledExpr = () => <div style={{ color: 'red' }}>{42}</div>;
