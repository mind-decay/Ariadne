import React from 'react';
import { Button } from './components/Button';

interface AppProps {
  title: string;
}

const App: React.FC<AppProps> = ({ title }) => {
  return (
    <div>
      <h1>{title}</h1>
      <Button label="Click me" onClick={() => console.log('clicked')} />
    </div>
  );
};

export default App;
