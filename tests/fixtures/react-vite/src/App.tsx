import { Button } from '@components/Button';
import { formatDate } from '@/utils/format';

export default function App() {
  return <div><Button label={formatDate(new Date())} /></div>;
}
