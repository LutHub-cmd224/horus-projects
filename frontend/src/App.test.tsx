import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import App from './App';

describe('App', () => {
  it('renders the HORUS product promise', () => {
    render(<App />);

    expect(screen.getByRole('heading', { name: /voir avant de construire/i })).toBeInTheDocument();
    expect(screen.getByText('Analyser')).toBeInTheDocument();
    expect(screen.getByText('Déployer')).toBeInTheDocument();
  });
});
