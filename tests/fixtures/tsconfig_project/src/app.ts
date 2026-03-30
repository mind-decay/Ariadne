import { Button } from '@/components/Button';
import { formatDate } from 'shared/lib/utils';

export function main(): void {
    Button({ label: 'Click me', onClick: () => {} });
    console.log(formatDate(new Date()));
}
