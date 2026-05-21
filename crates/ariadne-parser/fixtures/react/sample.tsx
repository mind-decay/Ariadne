import { useState } from "react";

interface CounterProps {
  label: string;
  step: number;
}

function Display(props: { value: number }) {
  return <span className="counter-value">{props.value}</span>;
}

const Badge = () => <span className="badge">new</span>;

function Counter({ label, step }: CounterProps) {
  const [count, setCount] = useState(0);
  const onClick = () => setCount(count + step);
  return (
    <div className="counter">
      <Display value={count} />
      <button type="button" onClick={onClick}>
        {label}
      </button>
    </div>
  );
}

export function App() {
  return (
    <main>
      <Badge />
      <Counter label="Clicks" step={1} />
    </main>
  );
}
