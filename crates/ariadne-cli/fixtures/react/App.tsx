import { useState } from "react";

function Label({ text }: { text: string }) {
  return <span className="label">{text}</span>;
}

export function App() {
  const [count, setCount] = useState(0);
  return (
    <main>
      <Label text="clicks" />
      <button type="button" onClick={() => setCount(count + 1)}>
        {count}
      </button>
    </main>
  );
}
