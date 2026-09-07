import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { AnalyzeWorkspace } from './AnalyzeWorkspace';
import { api, type OverviewPhase, type ValidationCriterion } from './api';

const phase: OverviewPhase = {
  id: 'analyze-phase',
  phase_type: 'ANALYZE',
  position: 1,
  status: 'AVAILABLE',
  required_criteria: 2,
  completed_required_criteria: 0,
};

const criteria: ValidationCriterion[] = [
  { id: 'problem', phase_id: phase.id, code: 'PROBLEM', label: 'Problème formulé', required: true, completed: false },
  { id: 'target', phase_id: phase.id, code: 'TARGET', label: 'Cible identifiée', required: true, completed: false },
];

describe('AnalyzeWorkspace', () => {
  afterEach(cleanup);

  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('starts Analyser, completes a criterion, and refreshes the overview', async () => {
    vi.spyOn(api, 'criteria').mockResolvedValue(criteria);
    const startPhase = vi.spyOn(api, 'startPhase').mockResolvedValue(undefined);
    const updateCriterion = vi.spyOn(api, 'updateCriterion').mockResolvedValue(undefined);
    const onChanged = vi.fn().mockResolvedValue(undefined);

    render(<AnalyzeWorkspace onChanged={onChanged} phase={phase} token="token" />);
    fireEvent.click(await screen.findByRole('button', { name: /problème formulé/i }));

    await waitFor(() => expect(updateCriterion).toHaveBeenCalledWith(phase.id, 'problem', true, 'token'));
    expect(startPhase).toHaveBeenCalledWith(phase.id, 'token');
    expect(onChanged).toHaveBeenCalledOnce();
    expect(screen.getByText('1 / 2 critères')).toBeInTheDocument();
  });

  it('validates Analyser when every required criterion is complete', async () => {
    vi.spyOn(api, 'criteria').mockResolvedValue(criteria.map((criterion) => ({ ...criterion, completed: true })));
    const validatePhase = vi.spyOn(api, 'validatePhase').mockResolvedValue(undefined);
    const onChanged = vi.fn().mockResolvedValue(undefined);

    render(<AnalyzeWorkspace onChanged={onChanged} phase={{ ...phase, status: 'IN_PROGRESS' }} token="token" />);
    const validateButton = await screen.findByRole('button', { name: /valider analyser/i });
    expect(validateButton).toBeEnabled();
    fireEvent.click(validateButton);

    await waitFor(() => expect(validatePhase).toHaveBeenCalledWith(phase.id, 'token', 'Validation depuis HORUS Projects'));
    expect(onChanged).toHaveBeenCalledOnce();
  });
});
