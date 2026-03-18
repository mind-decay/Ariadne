import { formatName } from '../utils/format';

export interface LoginParams {
  username: string;
  password: string;
}

export function login(params: LoginParams): boolean {
  const display = formatName(params.username, '');
  console.log(`Logging in as ${display}`);
  return true;
}
