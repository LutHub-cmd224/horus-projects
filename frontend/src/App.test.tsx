import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import App from './App';

describe('App', () => {
  it('renders the HORUS dashboard and six phases', () => {
    render(<App />);

    expect(screen.getByRole('heading', { name: /horus projects/i })).toBeInTheDocument();
    expect(screen.getByText('Analyser')).toBeInTheDocument();
    expect(screen.getByText('Concevoir')).toBeInTheDocument();
    expect(screen.getByText('Déployer')).toBeInTheDocument();
    expect(screen.getByText(/finaliser l’architecture technique/i)).toBeInTheDocument();
  });
});
