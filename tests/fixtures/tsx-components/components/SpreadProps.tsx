import React from 'react';

interface SpreadProps {
  className: string;
  id: string;
}

// JSX spread attributes
export const SpreadProps = (props: SpreadProps) => <div {...props}>spread</div>;

// Spread with additional props
export const MixedSpread = (props: SpreadProps) => (
  <div {...props} data-testid="mixed" style={{ margin: 0 }}>mixed</div>
);
