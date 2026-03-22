import React from 'react';

interface BoxProps<T> {
  value: T;
  render: (item: T) => React.ReactNode;
}

// Generic arrow function with trailing comma (disambiguates from JSX)
export const GenericBox = <T,>(props: BoxProps<T>) => <div>{props.render(props.value)}</div>;

// Multiple generic params
export const Pair = <A, B>({ a, b }: { a: A; b: B }) => (
  <span>{String(a)}-{String(b)}</span>
);
