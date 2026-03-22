import React from 'react';
import { Header } from './components/Header';
import { Card } from './components/Card';
import { StyledBox } from './components/StyledBox';
import { GenericBox } from './components/GenericBox';
import { Fragment } from './components/Fragment';
import { SpreadProps } from './components/SpreadProps';
import { Conditional, List } from './components/Conditional';
import DefaultAnon from './components/DefaultAnon';
import { Callback } from './components/Callback';
import { LegacyButton } from './components/LegacyButton';

const App = () => (
  <div>
    <Header title="Hello" />
    <Card label="click" />
    <StyledBox />
    <GenericBox value={42} render={(v) => <span>{v}</span>} />
    <Fragment />
    <SpreadProps className="x" id="y" />
    <Conditional show={true} items={[]} />
    <List show={false} items={['a', 'b']} />
    <DefaultAnon />
    <Callback />
    <LegacyButton label="legacy" />
  </div>
);

export default App;
