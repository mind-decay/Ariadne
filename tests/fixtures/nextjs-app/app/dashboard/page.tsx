"use client";
import { useContext } from 'react';
import { ThemeContext } from '../../src/lib/theme';

export default function Dashboard() {
  const theme = useContext(ThemeContext);
  return <div className={theme}>Dashboard</div>;
}
