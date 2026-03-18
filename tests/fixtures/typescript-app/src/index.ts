import { login } from './auth/login';
import { formatDate, formatName } from './utils/format';

const result = login({ username: 'admin', password: 'secret' });
console.log(formatDate(new Date()));
console.log(formatName('Jane', 'Doe'));
