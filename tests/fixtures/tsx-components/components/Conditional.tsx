import React from 'react';

interface ConditionalProps {
  show: boolean;
  items: string[];
}

// Conditional rendering with &&
export const Conditional = ({ show }: ConditionalProps) => (
  <div>{show && <span>visible</span>}</div>
);

// Ternary in JSX
export const Ternary = ({ show }: ConditionalProps) => (
  <div>{show ? <span>yes</span> : <span>no</span>}</div>
);

// .map with arrow returning JSX inside JSX expression
export const List = ({ items }: ConditionalProps) => (
  <ul>
    {items.map(item => <li key={item}>{item}</li>)}
  </ul>
);

// Nested map with index
export const IndexedList = ({ items }: ConditionalProps) => (
  <ol>
    {items.map((item, i) => (
      <li key={i} style={{ fontWeight: i === 0 ? 'bold' : 'normal' }}>{item}</li>
    ))}
  </ol>
);
