export interface ButtonProps {
  label: string;
}

export function Button(props: ButtonProps) {
  return <button type="button">{props.label}</button>;
}
