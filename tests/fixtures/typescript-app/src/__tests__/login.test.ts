import { login, LoginParams } from '../auth/login';

describe('login', () => {
  it('should return true for valid credentials', () => {
    const params: LoginParams = { username: 'admin', password: 'secret' };
    expect(login(params)).toBe(true);
  });

  it('should handle empty username', () => {
    const params: LoginParams = { username: '', password: 'secret' };
    expect(login(params)).toBe(true);
  });
});
