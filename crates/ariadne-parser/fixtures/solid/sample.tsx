import { createSignal, createEffect } from "solid-js";

function Label(props: { text: string }) {
  return <span class="label">{props.text}</span>;
}

export function Timer() {
  const [seconds, setSeconds] = createSignal(0);
  createEffect(() => {
    console.log("tick", seconds());
  });
  return (
    <div class="timer">
      <Label text="elapsed" />
      <output>{seconds()}</output>
    </div>
  );
}
