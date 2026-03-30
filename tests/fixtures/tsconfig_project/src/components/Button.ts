export interface ButtonProps {
    label: string;
    onClick: () => void;
}

export function Button(props: ButtonProps): void {
    console.log('Button:', props.label);
}
