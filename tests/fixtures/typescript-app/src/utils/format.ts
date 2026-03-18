export function formatDate(date: Date): string {
  return date.toISOString().split('T')[0];
}

export function formatName(first: string, last: string): string {
  return `${first} ${last}`.trim();
}
