import React from 'react';

interface HeaderProps {
  title: string;
}

// Bug #1a: implicit JSX return — arrow => <JSX> on one line
export const Header = ({ title }: HeaderProps) => <h1>{title}</h1>;
