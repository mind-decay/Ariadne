import { useState } from "react";

function Greeting({ name }) {
  return <h1 className="greeting">Hello, {name}</h1>;
}

export function Panel() {
  const [open, setOpen] = useState(false);
  const toggle = () => setOpen(!open);
  return (
    <section>
      <Greeting name="world" />
      <button type="button" onClick={toggle}>
        toggle
      </button>
    </section>
  );
}
