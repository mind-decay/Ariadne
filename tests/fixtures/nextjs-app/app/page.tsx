import { getData } from '../src/lib/data';

export default function Home() {
  return <div>{getData()}</div>;
}
