import { login, LoginParams } from './login';

export interface RegisterParams extends LoginParams {
  email: string;
}

export function register(params: RegisterParams): boolean {
  console.log(`Registering ${params.email}`);
  return login({ username: params.username, password: params.password });
}
