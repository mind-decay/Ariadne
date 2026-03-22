import React from 'react';

interface CardProps {
  label: string;
}

// Bug #1b: implicit JSX return — arrow => (<JSX>) on one line
export const Card = ({ label }: CardProps) => (<button>{label}</button>);
