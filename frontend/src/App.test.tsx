import { render, screen } from '@testing-library/react';
import { beforeEach, describe, expect, it } from 'vitest';
import App from './App';

describe('App', () => {
  beforeEach(() => {
    sessionStorage.clear();
  });

  it('renders the HORUS authentication experience', () => {
    render(<App />);

    expect(screen.getByText('HORUS')).toBeInTheDocument();
    expect(screen.getByRole('heading', { name: /construire devient la dernière étape/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /se connecter/i })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /explorer le dashboard en aperçu/i })).toBeInTheDocument();
  });
});
